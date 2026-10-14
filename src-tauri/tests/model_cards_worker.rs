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
    self, RegisterSteelModelRequest, TrainSteelDatasetRequest,
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
fn real_worker_registration_freezes_cards_for_project_report_and_reopen() {
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
    std::fs::write(&source,"C,temperature,HV,time\n0.1,800,100,1\n0.2,810,105,2\n0.3,820,110,3\n0.4,830,115,4\n0.5,840,120,5\n0.6,850,125,6\n0.7,860,,7\n").unwrap();
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
    let request = RegisterSteelModelRequest {
        task_id: trained.id.to_string(),
        lineage_id: None,
    };
    let registered = logic::register_steel_model_on_connection(&mut c, "local", &request).unwrap();
    let cards =
        suna::storage::repositories::steel_models::scientific_cards(&c, "local", &registered.id)
            .unwrap();
    assert_eq!(cards["persisted"], true);
    assert_eq!(cards["cards"]["model_card"]["features"][0]["unit"], "wt%");
    assert_eq!(cards["cards"]["model_card"]["target"]["unit"], "HV");
    assert_eq!(cards["cards"]["model_card"]["split_kind"], "time");
    assert_eq!(cards["cards"]["data_card"]["source_row_count"], 7);
    assert_eq!(cards["cards"]["data_card"]["valid_row_count"], 6);
    assert_eq!(cards["cards"]["data_card"]["filtered_row_count"], 1);
    assert_eq!(cards["cards"]["data_card"]["time_field"], "time");
    assert!(cards["cards"]["model_card"]["interval"]["observed_test_coverage"].is_null());
    let second = logic::register_steel_model_on_connection(&mut c, "local", &request).unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(second.model_sha256, registered.model_sha256);
    let second_cards =
        suna::storage::repositories::steel_models::scientific_cards(&c, "local", &second.id)
            .unwrap();
    assert_ne!(
        cards["cards"]["cards_sha256"],
        second_cards["cards"]["cards_sha256"]
    );
    let manifest =
        suna::storage::repositories::research_delivery::manifest(&c, "local", &project.id).unwrap();
    let snapshots = manifest["artifacts"].as_array().unwrap();
    assert!(snapshots
        .iter()
        .any(|artifact| artifact["payload"]["scientific_cards"] == cards["cards"]));
    assert!(!manifest.to_string().contains("model_pickle_base64"));
    assert!(!manifest.to_string().contains("source_rows"));
    let report =
        suna::storage::repositories::research_delivery::report(&mut c, "local", &project.id)
            .unwrap();
    assert!(report.content_markdown.contains("model-card.v1"));
    assert_eq!(
        suna::steel::hash_dataset_source(&source_text).unwrap(),
        hash
    );
    let calibration_source = root.join("合成 校准.csv");
    let mut csv = "C,temperature,HV,time\n".to_string();
    for i in 0..80 {
        let carbon = 0.1 + i as f64 * 0.005;
        let temperature = 800.0 + (i % 8) as f64 * 5.0;
        let target = 100.0 + carbon * 20.0 + temperature * 0.1 + (i as f64).sin();
        csv += &format!("{carbon},{temperature},{target},{i}\n");
    }
    std::fs::write(&calibration_source, csv).unwrap();
    let calibration_path = calibration_source.to_string_lossy().to_string();
    let calibration_sha = suna::steel::hash_dataset_source(&calibration_path).unwrap();
    let preview = suna::steel::preview_dataset(&suna::steel::DatasetPreviewRequest {
        source_path: calibration_path.clone(),
        sheet: None,
    })
    .unwrap();
    let data = steel::save_preview(
        &mut c,
        "local",
        &calibration_path,
        &calibration_sha,
        &preview,
        &mappings,
    )
    .unwrap();
    steel::activate(&mut c, "local", &data.id).unwrap();
    let trained_calibration=logic::train_steel_dataset_on_connection(&c,"local",&TrainSteelDatasetRequest{
        dataset_id:data.id.clone(),target_column:2,feature_columns:vec![0,1],algorithm:Some("linear_regression".into()),
        split_policy:Some(json!({"kind":"time","timeColumn":3,"validationFraction":0.2,"testFraction":0.2,"calibrationFraction":0.25})),
    }).unwrap();
    let calibrated = wait(&mut scheduler, &c, trained_calibration.id);
    let model = logic::register_steel_model_on_connection(
        &mut c,
        "local",
        &RegisterSteelModelRequest {
            task_id: trained_calibration.id.to_string(),
            lineage_id: None,
        },
    )
    .unwrap();
    let calibrated_cards =
        suna::storage::repositories::steel_models::scientific_cards(&c, "local", &model.id)
            .unwrap();
    let card = &calibrated_cards["cards"]["model_card"];
    assert_eq!(card["metrics"]["test"]["sample_count"], 16);
    assert_eq!(card["interval"]["calibration_sample_count"], 20);
    assert_eq!(
        card["interval"]["observed_test_coverage"],
        calibrated["artifact"]["evaluation"]["test_interval_coverage"]
    );
    assert_eq!(card["interval"]["exchangeability_verified"], false);
    assert_eq!(
        suna::storage::repositories::steel_models::scientific_cards(&c, "local", &registered.id)
            .unwrap(),
        cards
    );
    drop(scheduler);
    drop(c);
    let reopened = Connection::open(&database).unwrap();
    assert_eq!(
        suna::storage::repositories::steel_models::scientific_cards(
            &reopened,
            "local",
            &registered.id
        )
        .unwrap(),
        cards
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
