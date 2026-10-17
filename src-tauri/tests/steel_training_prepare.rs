//! Training snapshot consistency and enqueue guards (synthetic CSV only).
use rusqlite::Connection;
use suna::app::compute_commands::logic::{
    enqueue_prepared_steel_training, load_train_dataset_snapshot, prepare_steel_training_task,
    TrainSteelDatasetRequest,
};
use suna::steel::{hash_dataset_source, preview_dataset, DatasetPreviewRequest};
use suna::storage::migrations::migrate;
use suna::storage::repositories::{settings, steel};

const WORKSPACE: &str = "local";

fn workspace_db() -> (Connection, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("suna-train-prepare-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let db_path = root.join("workspace.sqlite");
    let mut connection = Connection::open(&db_path).expect("open database");
    migrate(&mut connection).expect("migrate");
    (connection, root)
}

#[test]
fn prepare_rejects_when_file_content_no_longer_matches_saved_sha() {
    let (mut connection, root) = workspace_db();
    let csv = root.join("train.csv");
    std::fs::write(&csv, "heat_id,yield_strength\nH-01,355\n").expect("write csv");
    let source_path = csv.to_string_lossy().into_owned();
    let hash = hash_dataset_source(&source_path).expect("hash");
    let preview = preview_dataset(&DatasetPreviewRequest {
        source_path: source_path.clone(),
        sheet: None,
    })
    .expect("preview");
    let mappings = vec![
        steel::DatasetColumnMapping {
            ordinal: 0,
            canonical_field: Some("feature_0".into()),
            unit: None,
        },
        steel::DatasetColumnMapping {
            ordinal: 1,
            canonical_field: Some("yield_strength".into()),
            unit: None,
        },
    ];
    let dataset = steel::save_preview(
        &mut connection,
        WORKSPACE,
        &source_path,
        &hash,
        &preview,
        &mappings,
    )
    .expect("save");
    steel::activate(&mut connection, WORKSPACE, &dataset.id).expect("activate");
    std::fs::write(&csv, "heat_id,yield_strength\nH-01,999\n").expect("mutate file");
    let snapshot =
        load_train_dataset_snapshot(&connection, WORKSPACE, &dataset.id).expect("snapshot");
    let request = TrainSteelDatasetRequest {
        dataset_id: dataset.id.clone(),
        target_column: 1,
        feature_columns: vec![0],
        split_policy: None,
        algorithm: Some("linear_regression".into()),
    };
    let result = prepare_steel_training_task(snapshot, &request);
    assert!(
        result.is_err(),
        "mutated source must not prepare against saved sha"
    );
    assert!(result.err().unwrap().contains("source changed"));
}

#[test]
fn enqueue_rejects_when_dataset_metadata_changed_after_prepare() {
    let (mut connection, root) = workspace_db();
    let csv = root.join("train.csv");
    std::fs::write(&csv, "feature_0,yield_strength\n1,355\n2,360\n").expect("write csv");
    let source_path = csv.to_string_lossy().into_owned();
    let hash = hash_dataset_source(&source_path).expect("hash");
    let preview = preview_dataset(&DatasetPreviewRequest {
        source_path: source_path.clone(),
        sheet: None,
    })
    .expect("preview");
    let mappings = vec![
        steel::DatasetColumnMapping {
            ordinal: 0,
            canonical_field: Some("feature_0".into()),
            unit: None,
        },
        steel::DatasetColumnMapping {
            ordinal: 1,
            canonical_field: Some("yield_strength".into()),
            unit: None,
        },
    ];
    let dataset = steel::save_preview(
        &mut connection,
        WORKSPACE,
        &source_path,
        &hash,
        &preview,
        &mappings,
    )
    .expect("save");
    steel::activate(&mut connection, WORKSPACE, &dataset.id).expect("activate");
    let snapshot =
        load_train_dataset_snapshot(&connection, WORKSPACE, &dataset.id).expect("snapshot");
    let request = TrainSteelDatasetRequest {
        dataset_id: dataset.id.clone(),
        target_column: 1,
        feature_columns: vec![0],
        split_policy: None,
        algorithm: Some("linear_regression".into()),
    };
    let prepared = prepare_steel_training_task(snapshot, &request).expect("prepare");
    std::thread::sleep(std::time::Duration::from_millis(5));
    connection
        .execute(
            "UPDATE steel_datasets SET updated_at = '2099-01-01T00:00:00Z' WHERE id = ?1",
            [&dataset.id],
        )
        .expect("touch dataset");
    let error = enqueue_prepared_steel_training(&connection, WORKSPACE, &prepared).unwrap_err();
    assert!(error.contains("changed during training preparation"));
}

#[test]
fn prepare_does_not_block_concurrent_setting_reads() {
    let (mut connection, root) = workspace_db();
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .expect("wal");
    settings::set(&mut connection, WORKSPACE, "ui.theme", "{\"version\":1}").expect("seed setting");
    let csv = root.join("large.csv");
    let mut body = String::from("feature_0,yield_strength\n");
    for index in 0..8_000 {
        body += &format!("{},{}\n", index % 100, 300 + (index % 50));
    }
    std::fs::write(&csv, body).expect("write large csv");
    let source_path = csv.to_string_lossy().into_owned();
    let hash = hash_dataset_source(&source_path).expect("hash");
    let preview = preview_dataset(&DatasetPreviewRequest {
        source_path: source_path.clone(),
        sheet: None,
    })
    .expect("preview");
    let mappings = vec![
        steel::DatasetColumnMapping {
            ordinal: 0,
            canonical_field: Some("feature_0".into()),
            unit: None,
        },
        steel::DatasetColumnMapping {
            ordinal: 1,
            canonical_field: Some("yield_strength".into()),
            unit: None,
        },
    ];
    let dataset = steel::save_preview(
        &mut connection,
        WORKSPACE,
        &source_path,
        &hash,
        &preview,
        &mappings,
    )
    .expect("save");
    steel::activate(&mut connection, WORKSPACE, &dataset.id).expect("activate");
    let db_path = root.join("workspace.sqlite");
    let dataset_id = dataset.id.clone();
    let db_path_for_prepare = db_path.clone();
    let prepare_thread = std::thread::spawn(move || {
        let connection = Connection::open(&db_path_for_prepare).expect("open reader db");
        let snapshot =
            load_train_dataset_snapshot(&connection, WORKSPACE, &dataset_id).expect("snapshot");
        let request = TrainSteelDatasetRequest {
            dataset_id,
            target_column: 1,
            feature_columns: vec![0],
            split_policy: None,
            algorithm: Some("linear_regression".into()),
        };
        let started = std::time::Instant::now();
        let prepared = prepare_steel_training_task(snapshot, &request).expect("prepare");
        (started.elapsed(), prepared)
    });
    let read_connection = Connection::open(&db_path).expect("open concurrent reader");
    let mut slow_reads = 0_u32;
    for _ in 0..40 {
        let started = std::time::Instant::now();
        let _ = settings::get(&read_connection, WORKSPACE, "ui.theme");
        if started.elapsed() > std::time::Duration::from_millis(250) {
            slow_reads += 1;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let (prepare_elapsed, prepared) = prepare_thread.join().expect("join prepare");
    assert!(
        slow_reads < 5,
        "too many slow concurrent reads during prepare: {slow_reads}"
    );
    enqueue_prepared_steel_training(&connection, WORKSPACE, &prepared).expect("enqueue");
    if std::env::var_os("SUNA_MEASURE_TRAINING").is_some() {
        let report = serde_json::json!({
            "prepare_ms": prepare_elapsed.as_millis(),
            "slow_reads": slow_reads,
            "path": "prepare_outside_write_transaction",
        });
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.suna-temp/training-prepare-measure.json");
        std::fs::write(out, report.to_string()).expect("write measure");
    }
}
