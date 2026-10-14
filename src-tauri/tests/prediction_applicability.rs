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
    self, PredictSteelModelRequest, TrainSteelDatasetRequest,
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
fn named_unit_inputs_and_each_risk_row_persist_with_real_worker_lineage() {
    let root = std::env::temp_dir().join(format!("suna-适用域-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("研究.sqlite3");
    let mut c = Connection::open(&database).unwrap();
    suna::storage::migrations::migrate(&mut c).unwrap();
    let project = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "合成适用域回归".into(),
            description: "非真实科研测量".into(),
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
    let source = root.join("合成 数据.csv");
    std::fs::write(&source,"C,temperature,HV,time\n0.1,800,100,1\n0.2,810,105,2\n0.3,820,110,3\n0.4,830,115,4\n0.5,840,120,5\n0.6,850,125,6\n").unwrap();
    let source_text = source.to_string_lossy().to_string();
    let hash = suna::steel::hash_dataset_source(&source_text).unwrap();
    let preview = suna::steel::preview_dataset(&suna::steel::DatasetPreviewRequest {
        source_path: source_text.clone(),
        sheet: None,
    })
    .unwrap();
    let mappings: Vec<_> = [
        ("c", "wt%"),
        ("temperature", "degC"),
        ("hv", "HV"),
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
    let trained = logic::train_steel_dataset_on_connection(
        &c,
        "local",
        &TrainSteelDatasetRequest {
            dataset_id: dataset.id.clone(),
            target_column: 2,
            feature_columns: vec![0, 1],
            split_policy: Some(json!({"kind":"time","validationFraction":0.33,"timeColumn":3})),
            algorithm: Some("linear_regression".into()),
        },
    )
    .unwrap();
    let training = wait(&mut scheduler, &c, trained.id);
    assert_eq!(training["artifact"]["data_version"], hash);
    assert_eq!(
        training["artifact"]["applicability_support"]["features"][1]["max"],
        830.0
    );
    let request:PredictSteelModelRequest=serde_json::from_value(json!({"datasetId":dataset.id,"trainingTaskId":trained.id,"inputSchema":[{"name":"temperature","unit":"K"},{"name":"c","unit":"fraction"}],"featureRows":[[1083.15,0.002],[1223.15,0.01]]})).unwrap();
    let predicted =
        logic::predict_steel_model_on_connection(&c, "local", &request, trained.id).unwrap();
    let prediction = wait(&mut scheduler, &c, predicted.id);
    assert!((prediction["input_rows"][0][0].as_f64().unwrap() - 0.2).abs() < 1e-12);
    assert!((prediction["input_rows"][0][1].as_f64().unwrap() - 810.0).abs() < 1e-9);
    assert_eq!(
        prediction["applicability"]["rows"][0]["state"],
        "within_domain"
    );
    assert_eq!(
        prediction["applicability"]["rows"][1]["state"],
        "outside_domain"
    );
    assert!(prediction["confidence"].is_null());
    let record = c
        .query_row(
            "SELECT request_json FROM prediction_tasks WHERE background_task_id=?1",
            [predicted.id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&record).unwrap()["input_schema"][0]["unit"],
        "K"
    );
    let manifest =
        suna::storage::repositories::research_delivery::manifest(&c, "local", &project.id).unwrap();
    assert!(manifest.to_string().contains("outside_domain"));
    assert!(!manifest.to_string().contains("input_rows"));
    assert_eq!(
        suna::steel::hash_dataset_source(&source_text).unwrap(),
        hash
    );
    drop(scheduler);
    drop(c);
    let reopened = Connection::open(&database).unwrap();
    let saved = repository::get(&reopened, "local", predicted.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(saved.checkpoint_json.as_deref().unwrap()).unwrap()["result"]
            ["applicability"],
        prediction["applicability"]
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
