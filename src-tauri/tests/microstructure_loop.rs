//! Real framed Worker and existing Scheduler; synthetic rasters only.
use rusqlite::Connection;
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use suna::storage::{
    migrations::migrate,
    repositories::{
        microstructure as images, research_delivery as delivery, research_projects as projects,
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
fn wait(s: &mut Scheduler, c: &Connection, id: uuid::Uuid) -> Value {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        s.tick().unwrap();
        let t = repository::get(c, "local", id).unwrap().unwrap();
        if matches!(
            t.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        ) {
            assert_eq!(t.state, TaskState::Completed, "{:?}", t.error_code);
            let raw: Value = serde_json::from_str(t.checkpoint_json.as_deref().unwrap()).unwrap();
            return raw["result"].clone();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn raster_to_calibrated_statistics_review_report_manifest_and_reopen() {
    let root = std::env::temp_dir().join(format!("suna-显微合成-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("科研.sqlite");
    let mut c = Connection::open(&db).unwrap();
    migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "合成图像闭环".into(),
            description: "不是材料相真值".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let source = root.join("合成.png");
    image::GrayImage::from_fn(20, 10, |x, _| image::Luma([if x < 10 { 255 } else { 0 }]))
        .save(&source)
        .unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut imported = vec![];
    for n in 0..2 {
        imported.push(
            images::import(
                &c,
                "local",
                &root,
                images::ImportImage {
                    source_path: source.to_string_lossy().into(),
                    project_id: p.id.clone(),
                    sample_id: format!("synthetic-{n}"),
                    modality: "OM".into(),
                    source_type: "synthetic".into(),
                    magnification: Some(100.),
                    acquired_at: None,
                    notes: "已知面积像素".into(),
                    gray16_mapping: None,
                },
            )
            .unwrap(),
        );
    }
    let worker_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker");
    let python = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or(worker_root.join(if cfg!(windows) {
            ".venv/Scripts/python.exe"
        } else {
            ".venv/bin/python"
        }));
    assert!(python.is_file(), "install worker environment");
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
    let mut annotations = vec![];
    for i in &imported {
        annotations.push(
            images::annotate(
                &c,
                "local",
                &i.id,
                images::Annotation {
                    calibration: Some(images::Calibration {
                        points: [[0., 0.], [10., 0.]],
                        distance: 20.,
                        unit: "um".into(),
                    }),
                    rois: vec![],
                    threshold: 128,
                    mask_confirmed: true,
                },
            )
            .unwrap()
            .id,
        );
    }
    let task = images::submit(&c, "local", &root, &p.id, annotations).unwrap();
    let r = wait(&mut scheduler, &c, task.id);
    assert_eq!(
        r["images"][0]["result"]["metrics"]["thresholdFraction"]["value"],
        50.
    );
    assert_eq!(
        r["images"][0]["result"]["regions"][0]["areaSquareMicrons"],
        400.
    );
    assert_eq!(r["images"][0]["result"]["scaleMicronsPerPixel"], 2.);
    let results = images::analyses(&c, "local", &imported[0].id, false).unwrap();
    assert!(images::analyses(&c, "local", &imported[0].id, true)
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    images::review(
        &c,
        "local",
        results[0]["id"].as_str().unwrap(),
        "accepted",
        "人工确认合成统计",
    )
    .unwrap();
    let report = delivery::report(&mut c, "local", &p.id).unwrap();
    assert!(report.content_markdown.contains("thresholdFraction"));
    assert!(report.content_markdown.contains("人工确认合成统计"));
    let manifest = delivery::manifest(&c, "local", &p.id).unwrap();
    assert!(!manifest.to_string().contains(root.to_str().unwrap()));
    assert!(!manifest["artifacts"].as_array().unwrap().is_empty());
    let backup = root.join("隔离科研备份.zip");
    let summary = suna::storage::backup::create_backup(&c, &db, &root, &backup).unwrap();
    assert_eq!(summary.content_file_count, 4);
    assert_eq!(
        suna::storage::backup::preview_backup(&backup)
            .unwrap()
            .content_file_count,
        4
    );
    let a = images::annotate(
        &c,
        "local",
        &imported[0].id,
        images::Annotation {
            calibration: None,
            rois: vec![],
            threshold: 128,
            mask_confirmed: true,
        },
    )
    .unwrap();
    let task = images::submit(&c, "local", &root, &p.id, vec![a.id]).unwrap();
    let r = wait(&mut scheduler, &c, task.id);
    assert!(r["images"][0]["result"]["scaleMicronsPerPixel"].is_null());
    assert!(r["images"][0]["result"]["regions"][0]
        .get("equivalentDiameterMicrons")
        .is_none());
    let broken_ref: String = c
        .query_row(
            "SELECT storage_ref FROM microstructure_images WHERE id=?1",
            [&imported[1].id],
            |r| r.get(0),
        )
        .unwrap();
    std::fs::write(root.join(broken_ref), b"changed").unwrap();
    let mut ids = vec![];
    for i in &imported {
        ids.push(
            images::latest_annotation(&c, "local", &i.id)
                .unwrap()
                .unwrap()
                .id,
        );
    }
    let task = images::submit(&c, "local", &root, &p.id, ids).unwrap();
    let r = wait(&mut scheduler, &c, task.id);
    assert!(r["images"][0]["result"].is_object());
    assert_eq!(r["images"][1]["error"], "source_hash_changed");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    drop(scheduler);
    drop(c);
    let mut c = Connection::open(&db).unwrap();
    assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
    assert_eq!(images::list(&c, "local", &p.id, 0).unwrap().len(), 2);
    assert_eq!(
        images::analyses(&c, "local", &imported[0].id, true)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn real_tiff16_input_contract_reaches_worker_and_frozen_scientific_results() {
    let root = std::env::temp_dir().join(format!("suna-16位闭环-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("科研.sqlite");
    let mut c = Connection::open(&db).unwrap();
    migrate(&mut c).unwrap();
    let project = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "16位合成像素".into(),
            description: String::new(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let original = include_bytes!("../../fixtures/microstructure/gray16-be.tif");
    let source = root.join("中文原图.tif");
    std::fs::write(&source, original).unwrap();
    let image = images::import(
        &c,
        "local",
        &root,
        images::ImportImage {
            source_path: source.to_string_lossy().into(),
            project_id: project.id.clone(),
            sample_id: "synthetic-16bit".into(),
            modality: "OM".into(),
            source_type: "synthetic".into(),
            magnification: Some(100.),
            acquired_at: None,
            notes: "不是材料相真值".into(),
            gray16_mapping: Some(images::Gray16Mapping::Window {
                lower: 1000,
                upper: 5000,
            }),
        },
    )
    .unwrap();
    let annotation = images::annotate(
        &c,
        "local",
        &image.id,
        images::Annotation {
            calibration: Some(images::Calibration {
                points: [[0., 0.], [2., 0.]],
                distance: 4.,
                unit: "um".into(),
            }),
            rois: vec![images::Roi {
                kind: "rectangle".into(),
                points: vec![[0., 0.], [2., 2.]],
            }],
            threshold: 128,
            mask_confirmed: true,
        },
    )
    .unwrap();
    let worker_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../compute-worker");
    let python = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or(worker_root.join(if cfg!(windows) {
            ".venv/Scripts/python.exe"
        } else {
            ".venv/bin/python"
        }));
    assert!(python.is_file());
    let mut config = suna::compute::worker::WorkerConfig::new(python);
    config.args = vec!["-m".into(), "suna_worker".into()];
    config.working_directory = Some(worker_root);
    let mut scheduler = Scheduler::new(
        db.clone(),
        "local".into(),
        SchedulerConfig::default(),
        Arc::new(SystemClock),
        suna::compute::handler::compute_task_handlers(Some(config)),
        Arc::new(Sink),
    )
    .unwrap();
    let task = images::submit(&c, "local", &root, &project.id, vec![annotation.id]).unwrap();
    let output = wait(&mut scheduler, &c, task.id);
    let stats = &output["images"][0]["result"];
    assert_eq!(stats["algorithm"], "microstructure.v2");
    assert_eq!(stats["rasterInput"], image.metadata["rasterInput"]);
    assert_eq!(stats["sourceSha256"], image.source_sha256);
    assert_eq!(stats["metrics"]["roiArea"]["value"], 4);
    assert_eq!(stats["metrics"]["roiPhysicalArea"]["value"], 16.);
    assert_eq!(stats["metrics"]["meanGray"]["value"], 111.5);
    assert_eq!(stats["metrics"]["thresholdFraction"]["value"], 50.);
    assert_eq!(stats["regions"][0]["areaSquareMicrons"], 8.);
    let rows = images::analyses(&c, "local", &image.id, false).unwrap();
    assert_eq!(rows[0]["result"], *stats);
    images::review(
        &c,
        "local",
        rows[0]["id"].as_str().unwrap(),
        "accepted",
        "仅验证合成图像统计",
    )
    .unwrap();
    let report = delivery::report(&mut c, "local", &project.id).unwrap();
    assert!(report.content_markdown.contains("111.5"));
    let backup = root.join("16位原图备份.zip");
    assert_eq!(
        suna::storage::backup::create_backup(&c, &db, &root, &backup)
            .unwrap()
            .content_file_count,
        2
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
    drop(scheduler);
    drop(c);
    let mut c = Connection::open(&db).unwrap();
    assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
    assert_eq!(
        images::get(&c, "local", &image.id).unwrap().metadata,
        image.metadata
    );
    assert_eq!(
        images::analyses(&c, "local", &image.id, true).unwrap()[0]["result"],
        *stats
    );
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
