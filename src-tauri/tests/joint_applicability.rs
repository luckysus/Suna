//! Synthetic data exercises the actual Scheduler, framed Worker and SQLite.
#[path = "support/worker.rs"]
mod support_worker;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use suna::app::compute_commands::logic::{
    self, OptimizeSteelProcessRequest, TrainSteelDatasetRequest,
};
use suna::storage::repositories::{
    research_artifacts as artifacts, research_projects as projects, steel,
};
use suna::tasks::scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock};
use suna::tasks::{repository, TaskState};
struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: SchedulerEvent) {}
}
fn wait(scheduler: &mut Scheduler, connection: &Connection, id: uuid::Uuid) -> Value {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        scheduler.tick().unwrap();
        let task = repository::get(connection, "local", id).unwrap().unwrap();
        if matches!(
            task.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        ) {
            assert_eq!(
                task.state,
                TaskState::Completed,
                "worker outcome: {:?}",
                task.error_code
            );
            artifacts::reconcile_task(connection, "local", &id.to_string()).unwrap();
            return serde_json::from_str::<Value>(task.checkpoint_json.as_deref().unwrap())
                .unwrap()["result"]
                .clone();
        }
        assert!(Instant::now() < deadline, "worker test deadline exceeded");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn real_independent_models_keep_unit_risks_filtered_empty_results_and_frozen_lineage() {
    let root = std::env::temp_dir().join(format!("suna-联合风险-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("科研.sqlite3");
    let mut c = Connection::open(&database).unwrap();
    suna::storage::migrations::migrate(&mut c).unwrap();
    let project = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "合成联合模型适用域".into(),
            description: "非真实材料测量".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    c.execute(
        "INSERT INTO settings VALUES('local','suna.research.current_project',?1,'now')",
        [json!(project.id).to_string()],
    )
    .unwrap();
    let source = root.join("合成 物理量.csv");
    std::fs::write(&source,"C,temperature,HV,J,time\n0.1,700,100,50,1\n0.2,710,110,52,2\n0.3,720,120,54,3\n0.4,730,130,56,4\n0.5,740,140,58,5\n0.6,750,150,60,6\n").unwrap();
    let source_text = source.to_string_lossy().to_string();
    let hash = suna::steel::hash_dataset_source(&source_text).unwrap();
    let source_request = suna::steel::DatasetPreviewRequest {
        source_path: source_text.clone(),
        sheet: None,
    };
    let preview = suna::steel::preview_dataset(&source_request).unwrap();
    let mappings: Vec<_> = [
        ("c", "wt%"),
        ("temperature", "degC"),
        ("hv", "HV"),
        ("j", "J"),
        ("time", "s"),
    ]
    .iter()
    .enumerate()
    .map(|(ordinal, (name, unit))| steel::DatasetColumnMapping {
        ordinal,
        canonical_field: Some((*name).into()),
        unit: Some((*unit).into()),
    })
    .collect();
    let dataset =
        steel::save_preview(&mut c, "local", &source_text, &hash, &preview, &mappings).unwrap();
    steel::activate(&mut c, "local", &dataset.id).unwrap();
    let mut worker = suna::compute::worker::WorkerConfig::new(support_worker::python_executable());
    worker.args = vec!["-m".into(), "suna_worker".into()];
    worker.working_directory =
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker"));
    let mut scheduler = Scheduler::new(
        database.clone(),
        "local".into(),
        SchedulerConfig {
            max_workers: 1,
            max_attempts: 1,
            retry_base: Duration::from_millis(1),
            retry_max: Duration::from_millis(1),
            poll_interval: Duration::from_millis(10),
        },
        Arc::new(SystemClock),
        suna::compute::handler::compute_task_handlers(Some(worker)),
        Arc::new(Sink),
    )
    .unwrap();
    let first = logic::train_steel_dataset_on_connection(
        &c,
        "local",
        &TrainSteelDatasetRequest {
            dataset_id: dataset.id.clone(),
            target_column: 2,
            feature_columns: vec![0, 1],
            split_policy: Some(json!({"kind":"time","timeColumn":4,"validationFraction":0.33})),
            algorithm: Some("linear_regression".into()),
        },
    )
    .unwrap();
    let first_result = wait(&mut scheduler, &c, first.id);
    // A second synthetic frozen training request represents the SAME physical
    // source in K/fraction. Use the actual materials engine, retain the original
    // source SHA and record the exact conversions; no CSV or model is edited.
    let mut table = suna::steel::read_dataset_table(&source_request).unwrap();
    for row in &mut table.rows {
        row[0] = suna::steel::materials::convert(row[0].parse().unwrap(), "wt%", "fraction")
            .unwrap()
            .to_string();
        row[1] = suna::steel::materials::convert(row[1].parse().unwrap(), "degC", "K")
            .unwrap()
            .to_string();
    }
    let mut columns = dataset.columns.clone();
    columns[0].unit = Some("fraction".into());
    columns[1].unit = Some("K".into());
    let mut payload = logic::build_linear_regression_payload(
        &table,
        &columns,
        &TrainSteelDatasetRequest {
            dataset_id: dataset.id.clone(),
            target_column: 3,
            feature_columns: vec![1, 0],
            split_policy: Some(json!({"kind":"time","timeColumn":4,"validationFraction":0.33})),
            algorithm: Some("random_forest".into()),
        },
    )
    .unwrap();
    payload["data_version"] = json!(hash);
    payload["algorithm"] = json!("random_forest");
    payload["n_estimators"] = json!(10);
    payload["unit_conversion_snapshot"] = json!({"source_sha256":hash,"c":{"from":"wt%","to":"fraction","scale":0.01,"shift":0.},"temperature":{"from":"degC","to":"K","scale":1.,"shift":273.15}});
    let second = repository::create(
        &c,
        suna::tasks::NewTask {
            workspace_id: "local".into(),
            kind: "compute_train_sklearn_model".into(),
            payload_json: json!({"operation":"train_sklearn_model","payload":payload}).to_string(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .unwrap();
    let second_result = wait(&mut scheduler, &c, second.id);
    assert_eq!(
        second_result["artifact"]["feature_names"],
        json!(["temperature", "c"])
    );
    let mut request:OptimizeSteelProcessRequest=serde_json::from_value(json!({"datasetId":dataset.id,"variables":[{"name":"c","unit":"wt%","min":0.1,"max":0.6,"fixedValue":0.2},{"name":"temperature","unit":"degC","min":700,"max":750,"fixedValue":710}],
        "objectives":[{"name":"HV","source":"prediction_model","modelId":first.id,"direction":"maximize","unit":"HV"},{"name":"J","source":"prediction_model","modelId":second.id,"direction":"minimize","unit":"J"}],"trials":10,"seed":42,"algorithm":"nsga2"})).unwrap();
    let within = logic::submit_optimization_on_connection(&c, &request, uuid::Uuid::nil()).unwrap();
    let within_result = wait(&mut scheduler, &c, within.id);
    let candidate = &within_result["pareto_front"][0];
    assert!((candidate["objectives"][0].as_f64().unwrap() - 110.).abs() < 1e-6);
    assert_eq!(within_result["directions"], json!(["maximize", "minimize"]));
    assert_eq!(
        candidate["model_applicability"][0]["model_id"],
        first_result["artifact"]["model_id"]
    );
    assert_eq!(
        candidate["model_applicability"][1]["model_id"],
        second_result["artifact"]["model_id"]
    );
    for entry in candidate["model_applicability"].as_array().unwrap() {
        assert_eq!(entry["risk"]["state"], "within_domain");
        assert_eq!(entry["dataset_sha256"], hash);
        assert!(entry["probability"].is_null());
    }
    request.variables[0]["fixedValue"] = json!(0.6);
    request.variables[1]["fixedValue"] = json!(750.);
    let outside =
        logic::submit_optimization_on_connection(&c, &request, uuid::Uuid::nil()).unwrap();
    let outside_result = wait(&mut scheduler, &c, outside.id);
    let outside_candidate = &outside_result["pareto_front"][0];
    assert_eq!(outside_candidate["feasible"], true);
    for entry in outside_candidate["model_applicability"].as_array().unwrap() {
        assert_eq!(entry["risk"]["state"], "outside_domain");
    }
    assert_eq!(
        outside_candidate["model_applicability"][1]["risk"]["reasons"][0]["threshold"]["unit"],
        "K"
    );
    request.domain_policy = Some("within_domain_only".into());
    let filtered =
        logic::submit_optimization_on_connection(&c, &request, uuid::Uuid::nil()).unwrap();
    let filtered_result = wait(&mut scheduler, &c, filtered.id);
    assert_eq!(filtered_result["pareto_front"], json!([]));
    assert_eq!(filtered_result["recommendations"], json!([]));
    assert_eq!(filtered_result["domain_filter"]["before"], 1);
    assert_eq!(filtered_result["domain_filter"]["after"], 0);
    assert_eq!(
        filtered_result["domain_filter"]["status"],
        "insufficient_domain_candidates"
    );
    let manifest =
        suna::storage::repositories::research_delivery::manifest(&c, "local", &project.id).unwrap();
    let text = manifest.to_string();
    assert!(text.contains("model_applicability"));
    assert!(text.contains("outside_domain"));
    assert!(text.contains("insufficient_domain_candidates"));
    assert!(!text.contains("model_pickle_base64"));
    assert!(!text.contains("unit_conversion_snapshot"));
    assert_eq!(
        suna::steel::hash_dataset_source(&source_text).unwrap(),
        hash
    );
    drop(scheduler);
    drop(c);
    let reopened = Connection::open(&database).unwrap();
    let saved = repository::get(&reopened, "local", filtered.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.state, TaskState::Completed);
    assert_eq!(
        serde_json::from_str::<Value>(saved.checkpoint_json.as_deref().unwrap()).unwrap()["result"]
            ["domain_filter"],
        filtered_result["domain_filter"]
    );
    let frozen = repository::get(&reopened, "local", second.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(frozen.checkpoint_json.as_deref().unwrap()).unwrap()
            ["result"]["artifact"],
        second_result["artifact"]
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
