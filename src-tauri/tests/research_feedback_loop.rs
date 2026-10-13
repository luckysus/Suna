//! Synthetic fixtures exercise the real SQLite scheduler and framed Python worker.
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use suna::app::compute_commands::logic::{
    self, OptimizeSteelProcessRequest, TrainSteelDatasetRequest,
};
use suna::storage::{
    migrations::migrate,
    repositories::{
        experiment_measurements as measurements, experiments, material_rules,
        research_artifacts as artifacts, research_feedback as feedback,
        research_projects as projects, steel,
    },
};
use suna::tasks::scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock};
use suna::tasks::{repository, TaskState};
struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: SchedulerEvent) {}
}
fn config() -> suna::compute::worker::WorkerConfig {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker");
    let python = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(if cfg!(windows) {
                ".venv/Scripts/python.exe"
            } else {
                ".venv/bin/python"
            })
        });
    assert!(
        python.is_file(),
        "install compute-worker test dependencies or set SUNA_COMPUTE_WORKER_PYTHON"
    );
    let mut cfg = suna::compute::worker::WorkerConfig::new(python);
    cfg.args = vec!["-m".into(), "suna_worker".into()];
    cfg.working_directory = Some(root);
    cfg
}
fn wait(scheduler: &mut Scheduler, c: &Connection, id: uuid::Uuid) -> Value {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        scheduler.tick().unwrap();
        let task = repository::get(c, "local", id).unwrap().unwrap();
        if matches!(
            task.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        ) {
            assert_eq!(
                task.state,
                TaskState::Completed,
                "real worker failed: {task:?}"
            );
            let cp: Value = serde_json::from_str(task.checkpoint_json.as_deref().unwrap()).unwrap();
            return cp["result"].clone();
        }
        assert!(Instant::now() < deadline, "worker test deadline exceeded");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn train(c: &Connection, dataset: &str, algorithm: &str, target: usize) -> uuid::Uuid {
    logic::train_steel_dataset_on_connection(
        c,
        "local",
        &TrainSteelDatasetRequest {
            dataset_id: dataset.into(),
            target_column: target,
            feature_columns: vec![0, 1],
            split_policy: Some(json!({"kind":"random","validationFraction":0.2,"seed":42})),
            algorithm: Some(algorithm.into()),
        },
    )
    .unwrap()
    .id
}
fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("suna-feedback-合成-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn prepare(root: &Path) -> (Connection, String, String) {
    let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
    migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "自动化合成闭环（非科研事实）".into(),
            description: "".into(),
            material_system: Some("steel".into()),
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    c.execute(
        "INSERT INTO settings VALUES('local','suna.research.current_project',?1,'now')",
        [json!(p.id).to_string()],
    )
    .unwrap();
    let path = root.join("原始合成.csv");
    let mut csv = "C,temperature,HV,J\n".to_string();
    for i in 0..40 {
        let carbon = 0.1 + (i % 10) as f64 * 0.07;
        let temperature = 820. + (i / 10) as f64 * 20.;
        csv += &format!(
            "{carbon},{temperature},{},{}\n",
            300. + 200. * carbon + 0.1 * temperature,
            90. - 50. * carbon + 0.01 * temperature
        );
    }
    std::fs::write(&path, csv).unwrap();
    let source = path.to_string_lossy().to_string();
    let preview = suna::steel::preview_dataset(&suna::steel::DatasetPreviewRequest {
        source_path: source.clone(),
        sheet: None,
    })
    .unwrap();
    let sha = suna::steel::hash_dataset_source(&source).unwrap();
    let mappings = vec![
        steel::DatasetColumnMapping {
            ordinal: 0,
            canonical_field: Some("c".into()),
            unit: Some("wt%".into()),
        },
        steel::DatasetColumnMapping {
            ordinal: 1,
            canonical_field: Some("temperature".into()),
            unit: Some("degC".into()),
        },
        steel::DatasetColumnMapping {
            ordinal: 2,
            canonical_field: Some("hv".into()),
            unit: Some("HV".into()),
        },
        steel::DatasetColumnMapping {
            ordinal: 3,
            canonical_field: Some("impact".into()),
            unit: Some("J".into()),
        },
    ];
    let d = steel::save_preview(&mut c, "local", &source, &sha, &preview, &mappings).unwrap();
    steel::activate(&mut c, "local", &d.id).unwrap();
    (c, p.id, d.id)
}
#[test]
fn two_models_five_suggestions_two_measurements_new_version_retrain_next_round() {
    let root = root();
    let (mut c, p, d) = prepare(&root);
    let mut scheduler = Scheduler::new(
        root.join("研究.sqlite"),
        "local".into(),
        SchedulerConfig {
            max_workers: 1,
            max_attempts: 1,
            retry_base: Duration::from_millis(1),
            retry_max: Duration::from_millis(1),
            poll_interval: Duration::from_millis(10),
        },
        Arc::new(SystemClock),
        suna::compute::handler::compute_task_handlers(Some(config())),
        Arc::new(Sink),
    )
    .unwrap();
    let m1 = train(&c, &d, "linear_regression", 2);
    let before = wait(&mut scheduler, &c, m1);
    let m2 = train(&c, &d, "random_forest", 3);
    wait(&mut scheduler, &c, m2);
    let rule:suna::steel::materials::Rule=serde_json::from_value(json!({"id":"test-equipment","name":"合成设备限制","severity":"hard","enabled":true,"penalty":1,"provenance":{"type":"equipment","sourceRef":"synthetic-fixture","sourceVersion":"1","reviewed":true},"expression":{"kind":"range","fieldId":"process.quench_temperature","unit":"K","min":1093.15,"max":1153.15}})).unwrap();
    material_rules::save(&c, "local", &p, rule, 0).unwrap();
    let request:OptimizeSteelProcessRequest=serde_json::from_value(json!({"datasetId":d,"variables":[{"name":"c","fieldId":"chem.c","unit":"wt%","min":0.1,"max":0.73},{"name":"temperature","fieldId":"process.quench_temperature","unit":"degC","min":800,"max":900}],"objectives":[{"name":"HV","source":"prediction_model","modelId":m1,"direction":"maximize","unit":"HV"},{"name":"J","source":"prediction_model","modelId":m2,"direction":"maximize","unit":"J"},{"name":"成本","source":"formula","direction":"minimize","unit":"CNY","coefficients":{"c":1}}],"trials":200,"seed":42,"algorithm":"nsga2"})).unwrap();
    let optimization =
        logic::submit_optimization_on_connection(&c, &request, uuid::Uuid::nil()).unwrap();
    let result = wait(&mut scheduler, &c, optimization.id);
    assert!(result["recommendations"].as_array().unwrap().len() >= 5);
    let _five_suggestions = &result["recommendations"].as_array().unwrap()[..5];
    assert!(result["recommendations"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| {
            let t = p["values"]["temperature"].as_f64().unwrap();
            (820.0 - 1e-7..=880.0 + 1e-7).contains(&t)
        }));
    let opt_artifact = artifacts::list(&c, "local", &p, 100, 0)
        .unwrap()
        .into_iter()
        .find(|a| a.kind == "optimization_result")
        .unwrap();
    let mut execution_ids = vec![];
    for index in 0..2 {
        let plan = measurements::candidate_plan(
            &c,
            "local",
            &p,
            &opt_artifact.id,
            index,
            &format!("合成候选{}", index + 1),
        )
        .unwrap();
        experiments::set_state(&mut c, "local", &plan.id, "accepted").unwrap();
        let values = result["recommendations"][index]["values"].clone();
        let vars: std::collections::BTreeMap<String, f64> = serde_json::from_value(values).unwrap();
        let input:measurements::RecordExecution=serde_json::from_value(json!({"projectId":p,"planId":plan.id,"sampleId":format!("合成样本{index}"),"heatId":"synthetic-heat","batchId":null,"actualVariables":vars,"variableUnits":{"c":"wt%","temperature":"degC"},"performedAt":"2026-01-01T00:00:00Z","method":"synthetic fixture only","sourceType":"measured","note":"自动化夹具，非真实实验","measurements":[{"property":"HV","value":result["recommendations"][index]["objectives"][0].as_f64().unwrap()+5.,"unit":"HV","method":"HV10 fixture","measuredAt":"2026-01-01T00:00:00Z"},{"property":"J","value":result["recommendations"][index]["objectives"][1].as_f64().unwrap()-2.,"unit":"J","method":"impact fixture","measuredAt":"2026-01-01T00:00:00Z"}]})).unwrap();
        let e = measurements::record(&c, "local", input).unwrap();
        measurements::review(&c, "local", &e.id, "submitted", "test", "draft").unwrap();
        measurements::review(
            &c,
            "local",
            &e.id,
            "accepted",
            "synthetic acceptance fixture",
            "submitted",
        )
        .unwrap();
        assert!(
            (measurements::compare(&c, "local", &e.id, &opt_artifact.id, index)
                .unwrap()
                .iter()
                .find(|v| v.property == "HV")
                .unwrap()
                .prediction_minus_actual
                + 5.)
                .abs()
                < 1e-8
        );
        execution_ids.push(e.id);
    }
    let version = measurements::create_dataset_version(
        &c,
        "local",
        &root,
        measurements::DatasetVersionRequest {
            project_id: p.clone(),
            execution_ids,
            features: vec!["c".into(), "temperature".into()],
            properties: vec!["HV".into(), "J".into()],
            parent_dataset_id: Some(d.clone()),
        },
    )
    .unwrap();
    assert_eq!(version.row_count, 42);
    steel::activate(&mut c, "local", &version.id).unwrap();
    let trained = train(&c, &version.id, "linear_regression", 2);
    let after = wait(&mut scheduler, &c, trained);
    assert_ne!(before["artifact"], after["artifact"]);
    let original = repository::get(&c, "local", m1).unwrap().unwrap();
    let cp: Value = serde_json::from_str(original.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(cp["result"], before);
    let request:feedback::RecommendRequest=serde_json::from_value(json!({"projectId":p,"datasetId":version.id,"property":"HV","variables":[{"name":"c","fieldId":"chem.c","unit":"wt%","min":0.1,"max":0.73},{"name":"temperature","fieldId":"process.quench_temperature","unit":"degC","min":820,"max":880}],"strategy":"balanced","budget":3,"direction":"maximize","seed":42})).unwrap();
    let next = feedback::submit(&c, "local", request).unwrap();
    let recommendations = wait(&mut scheduler, &c, next.id);
    assert_eq!(recommendations["points"].as_array().unwrap().len(), 3);
    assert!(recommendations["points"][0]["uncertainty_source"]
        .as_str()
        .unwrap()
        .contains("GP"));
    assert!(
        artifacts::edges(&c, "local", &p, 100, 0)
            .unwrap()
            .iter()
            .filter(|e| e.relation == "uses_model")
            .count()
            >= 2
    );
    drop(scheduler);
    drop(c);
    let mut reopened = Connection::open(root.join("研究.sqlite")).unwrap();
    migrate(&mut reopened).unwrap();
    assert!(
        artifacts::list(&reopened, "local", &p, 100, 0)
            .unwrap()
            .len()
            >= 9
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
