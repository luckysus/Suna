//! File exchange -> new dataset -> real framed Worker -> project lineage.
//! Synthetic inputs only; this is not a live SQL Server connection test.
use rusqlite::Connection;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use suna::app::compute_commands::logic::{self, TrainSteelDatasetRequest};
use suna::storage::{
    migrations::migrate,
    repositories::{
        industrial_sources, research_artifacts, research_delivery, research_projects, steel,
    },
};
use suna::tasks::{
    repository,
    scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock},
    TaskState,
};
struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: SchedulerEvent) {}
}
#[test]
fn synthetic_software_file_trains_without_license_and_retains_source_lineage() {
    let root = std::env::temp_dir().join(format!("suna-专业文件闭环-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("项目.sqlite");
    let mut c = Connection::open(&db).unwrap();
    migrate(&mut c).unwrap();
    let p = research_projects::save(
        &mut c,
        "local",
        research_projects::SaveResearchProject {
            id: None,
            name: "专业软件合成文件".into(),
            description: "此数据并非商业软件计算或实测".into(),
            material_system: None,
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
    let source = root.join("合成导出.csv");
    let mut csv = "C,temperature,HV\n".to_string();
    for i in 0..40 {
        let carbon = 0.1 + (i % 10) as f64 * 0.07;
        let temperature = 820. + (i / 10) as f64 * 20.;
        csv += &format!(
            "{carbon},{temperature},{}\n",
            300. + carbon * 200. + temperature * 0.1
        );
    }
    std::fs::write(&source, &csv).unwrap();
    let d = industrial_sources::file_import(
        &c,
        "local",
        &root,
        industrial_sources::FileImport {
            project_id: p.id.clone(),
            source_path: source.to_string_lossy().into(),
            adapter_id: "custom_file".into(),
            software_version: "synthetic-v1".into(),
            rights_basis: "local synthetic fixture".into(),
            rights_confirmed: true,
            batch: "synthetic-1".into(),
            input_conditions: json!({"temperature":{"value":850,"unit":"degC"}}),
            output_units: std::collections::BTreeMap::from([("HV".into(), "HV".into())]),
            mappings: vec![
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
            ],
        },
    )
    .unwrap();
    steel::activate(&mut c, "local", &d.id).unwrap();
    let worker_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker");
    let python = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            worker_root.join(if cfg!(windows) {
                ".venv/Scripts/python.exe"
            } else {
                ".venv/bin/python"
            })
        });
    assert!(python.is_file(), "install compute-worker test environment");
    let mut cfg = suna::compute::worker::WorkerConfig::new(python);
    cfg.args = vec!["-m".into(), "suna_worker".into()];
    cfg.working_directory = Some(worker_root);
    let mut scheduler = Scheduler::new(
        db.clone(),
        "local".into(),
        SchedulerConfig::default(),
        Arc::new(SystemClock),
        suna::compute::handler::compute_task_handlers(Some(cfg)),
        Arc::new(Sink),
    )
    .unwrap();
    let t = logic::train_steel_dataset_on_connection(
        &c,
        "local",
        &TrainSteelDatasetRequest {
            dataset_id: d.id.clone(),
            target_column: 2,
            feature_columns: vec![0, 1],
            split_policy: Some(json!({"kind":"random","validationFraction":0.2,"seed":42})),
            algorithm: Some("linear_regression".into()),
        },
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        scheduler.tick().unwrap();
        let task = repository::get(&c, "local", t.id).unwrap().unwrap();
        if matches!(
            task.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        ) {
            assert_eq!(task.state, TaskState::Completed, "{:?}", task.error_code);
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(research_artifacts::list(&c, "local", &p.id, 100, 0)
        .unwrap()
        .iter()
        .any(|a| a.kind == "model_snapshot"));
    assert!(
        research_artifacts::edges(&c, "local", &p.id, 100, 0)
            .unwrap()
            .len()
            >= 2
    );
    let manifest = research_delivery::manifest(&c, "local", &p.id)
        .unwrap()
        .to_string();
    assert!(manifest.contains("external_simulation"));
    assert!(!manifest.contains(root.to_str().unwrap()));
    assert_eq!(std::fs::read_to_string(&source).unwrap(), csv);
    assert_eq!(std::fs::read_to_string(&d.source_path).unwrap(), csv);
    drop(scheduler);
    drop(c);
    let c = Connection::open(&db).unwrap();
    assert_eq!(
        steel::get(&c, "local", &d.id)
            .unwrap()
            .unwrap()
            .mapping_state,
        "ready"
    );
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
