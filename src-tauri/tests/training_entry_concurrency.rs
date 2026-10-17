//! Formal training-entry concurrency: IPC prepare/enqueue split, Agent gateway,
//! and workflow-style prepare-outside-atomic_mut. All DB ops must succeed or be counted.
use rusqlite::Connection;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use suna::agent::runtime::CancellationToken;
use suna::app::compute_commands::logic::{
    enqueue_prepared_steel_training, load_train_dataset_snapshot, prepare_steel_training_task,
    TrainSteelDatasetRequest,
};
use suna::app::steel_agent_gateway::DesktopSteelAgentGateway;
use suna::steel::{hash_dataset_source, preview_dataset, DatasetPreviewRequest, SteelAgentGateway};
use suna::storage::migrations::migrate;
use suna::storage::repositories::{research_artifacts, settings, steel};
use uuid::Uuid;

const WORKSPACE: &str = "local";

struct LatencyStats {
    samples_ms: Vec<u128>,
    busy_or_locked: u64,
    failures: u64,
}

impl LatencyStats {
    fn new() -> Self {
        Self {
            samples_ms: Vec::new(),
            busy_or_locked: 0,
            failures: 0,
        }
    }

    fn record_ok(&mut self, elapsed: Duration) {
        self.samples_ms.push(elapsed.as_millis());
    }

    fn record_err(&mut self, message: &str) {
        let lower = message.to_ascii_lowercase();
        if lower.contains("busy") || lower.contains("locked") {
            self.busy_or_locked += 1;
        } else {
            self.failures += 1;
        }
    }

    fn median(&self) -> u128 {
        if self.samples_ms.is_empty() {
            return 0;
        }
        let mut values = self.samples_ms.clone();
        values.sort_unstable();
        values[values.len() / 2]
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "samples": self.samples_ms.len(),
            "median_ms": self.median(),
            "max_ms": self.samples_ms.iter().copied().max().unwrap_or(0),
            "busy_or_locked": self.busy_or_locked,
            "failures": self.failures,
        })
    }
}

fn seed_dataset(root: &PathBuf, rows: usize) -> (PathBuf, String) {
    let csv = root.join("train.csv");
    let mut body = String::from("feature_0,yield_strength\n");
    for index in 0..rows {
        body += &format!("{},{}\n", index % 100, 300 + (index % 50));
    }
    std::fs::write(&csv, body).expect("write csv");
    let source_path = csv.to_string_lossy().into_owned();
    let hash = hash_dataset_source(&source_path).expect("hash");
    let preview = preview_dataset(&DatasetPreviewRequest {
        source_path: source_path.clone(),
        sheet: None,
    })
    .expect("preview");
    let mut connection = Connection::open(root.join("workspace.sqlite")).expect("open");
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
        .expect("wal");
    migrate(&mut connection).expect("migrate");
    let dataset = steel::save_preview(
        &mut connection,
        WORKSPACE,
        &source_path,
        &hash,
        &preview,
        &[
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
        ],
    )
    .expect("save");
    steel::activate(&mut connection, WORKSPACE, &dataset.id).expect("activate");
    settings::set(&mut connection, WORKSPACE, "ui.theme", "{\"version\":1}").expect("seed setting");
    (root.join("workspace.sqlite"), dataset.id)
}

fn concurrent_traffic(
    db_path: PathBuf,
    stop: Arc<AtomicBool>,
    reads: Arc<Mutex<LatencyStats>>,
    writes: Arc<Mutex<LatencyStats>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let connection = Connection::open(&db_path).expect("open concurrent connection");
        connection
            .busy_timeout(Duration::from_millis(5_000))
            .expect("busy timeout");
        let mut n = 0_u64;
        while !stop.load(Ordering::SeqCst) {
            n += 1;
            let started = Instant::now();
            match settings::get(&connection, WORKSPACE, "ui.theme") {
                Ok(Some(_)) => reads.lock().unwrap().record_ok(started.elapsed()),
                Ok(None) => reads.lock().unwrap().record_err("missing setting"),
                Err(error) => reads.lock().unwrap().record_err(&error),
            }
            let started = Instant::now();
            let value = format!("{{\"version\":1,\"n\":{n}}}");
            match settings::set(
                &mut Connection::open(&db_path).expect("open writer"),
                WORKSPACE,
                "ui.theme",
                &value,
            ) {
                Ok(()) => writes.lock().unwrap().record_ok(started.elapsed()),
                Err(error) => writes.lock().unwrap().record_err(&error),
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })
}

fn write_report(name: &str, report: serde_json::Value) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.suna-temp");
    std::fs::create_dir_all(&dir).expect("create .suna-temp");
    let path = dir.join(format!("training-entry-{name}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).expect("write report");
    println!("{name}: {report}");
}

#[test]
fn ipc_prepare_outside_transaction_allows_concurrent_reads_and_writes() {
    let root = std::env::temp_dir().join(format!("suna-train-ipc-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let (db_path, dataset_id) = seed_dataset(&root, 6_000);
    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(Mutex::new(LatencyStats::new()));
    let writes = Arc::new(Mutex::new(LatencyStats::new()));
    let traffic = concurrent_traffic(db_path.clone(), stop.clone(), reads.clone(), writes.clone());
    std::thread::sleep(Duration::from_millis(20));

    let connection = Connection::open(&db_path).unwrap();
    let snapshot = load_train_dataset_snapshot(&connection, WORKSPACE, &dataset_id).unwrap();
    let request = TrainSteelDatasetRequest {
        dataset_id: dataset_id.clone(),
        target_column: 1,
        feature_columns: vec![0],
        split_policy: None,
        algorithm: Some("linear_regression".into()),
    };
    let prepare_started = Instant::now();
    let prepared = prepare_steel_training_task(snapshot, &request).expect("prepare");
    let prepare_ms = prepare_started.elapsed().as_millis();

    let enqueue_started = Instant::now();
    let task = enqueue_prepared_steel_training(&connection, WORKSPACE, &prepared).expect("enqueue");
    let enqueue_ms = enqueue_started.elapsed().as_millis();

    stop.store(true, Ordering::SeqCst);
    traffic.join().unwrap();
    let read_stats = reads.lock().unwrap().to_json();
    let write_stats = writes.lock().unwrap().to_json();
    assert_eq!(read_stats["failures"], 0);
    assert_eq!(write_stats["failures"], 0);
    assert!(
        write_stats["samples"].as_u64().unwrap_or(0) > 0,
        "must perform concurrent writes during prepare/enqueue"
    );
    write_report(
        "ipc",
        json!({
            "entry": "ipc_prepare_then_enqueue",
            "prepare_ms": prepare_ms,
            "enqueue_or_tx_ms": enqueue_ms,
            "task_id": task.id.to_string(),
            "concurrent_reads": read_stats,
            "concurrent_writes": write_stats,
        }),
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn agent_gateway_start_training_prepare_allows_concurrent_traffic() {
    let root = std::env::temp_dir().join(format!("suna-train-agent-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let (db_path, dataset_id) = seed_dataset(&root, 4_000);
    let connection = Connection::open(&db_path).unwrap();
    let conversation = Uuid::new_v4().to_string();
    let message = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4();
    connection
        .execute(
            "INSERT INTO conversations (id, workspace_id, title, created_at, updated_at)
             VALUES (?1, 'local', 'train', '2026-10-17T00:00:00Z', '2026-10-17T00:00:00Z')",
            rusqlite::params![conversation],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO messages (id, workspace_id, conversation_id, role, content, created_at)
             VALUES (?1, 'local', ?2, 'user', 'train', '2026-10-17T00:00:00Z')",
            rusqlite::params![message, conversation],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO agent_runs
                (id, workspace_id, conversation_id, user_message_id, state, created_at, updated_at)
             VALUES (?1, 'local', ?2, ?3, 'executing_tools', '2026-10-17T00:00:00Z', '2026-10-17T00:00:00Z')",
            rusqlite::params![run_id.to_string(), conversation, message],
        )
        .unwrap();
    drop(connection);

    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(Mutex::new(LatencyStats::new()));
    let writes = Arc::new(Mutex::new(LatencyStats::new()));
    let traffic = concurrent_traffic(db_path.clone(), stop.clone(), reads.clone(), writes.clone());
    std::thread::sleep(Duration::from_millis(20));

    let gateway = DesktopSteelAgentGateway::new(db_path.clone(), "local").with_run_id(run_id);
    let started = Instant::now();
    let response = tauri::async_runtime::block_on(
        <DesktopSteelAgentGateway as SteelAgentGateway>::execute_for_tool_call(
            &gateway,
            "start_training",
            Uuid::new_v4(),
            json!({
                "datasetId": dataset_id,
                "targetColumn": 1,
                "featureColumns": [0],
                "algorithm": "linear_regression"
            }),
            CancellationToken::new(|| false),
        ),
    )
    .expect("agent training");
    let total_ms = started.elapsed().as_millis();
    assert_eq!(response["success"], json!(true));
    let prepare_ms = response["timings"]["prepare_ms"]
        .as_u64()
        .expect("gateway must report prepare_ms");
    let enqueue_or_tx_ms = response["timings"]["enqueue_or_tx_ms"]
        .as_u64()
        .expect("gateway must report enqueue_or_tx_ms");
    assert!(
        prepare_ms > 0,
        "prepare must do real work under concurrency"
    );

    stop.store(true, Ordering::SeqCst);
    traffic.join().unwrap();
    let read_stats = reads.lock().unwrap().to_json();
    let write_stats = writes.lock().unwrap().to_json();
    assert_eq!(read_stats["failures"], 0);
    assert_eq!(write_stats["failures"], 0);
    assert!(
        read_stats["samples"].as_u64().unwrap_or(0) > 0,
        "concurrent reads must succeed during agent prepare"
    );
    assert!(
        write_stats["samples"].as_u64().unwrap_or(0) > 0,
        "concurrent small writes must succeed during agent prepare"
    );
    write_report(
        "agent",
        json!({
            "entry": "agent_gateway_start_training",
            "prepare_ms": prepare_ms,
            "enqueue_or_tx_ms": enqueue_or_tx_ms,
            "total_ms": total_ms,
            "task_id": response["task"]["id"],
            "concurrent_reads": read_stats,
            "concurrent_writes": write_stats,
        }),
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn workflow_prepare_outside_atomic_mut_allows_concurrent_traffic() {
    let root = std::env::temp_dir().join(format!("suna-train-workflow-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let (db_path, dataset_id) = seed_dataset(&root, 4_000);
    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(Mutex::new(LatencyStats::new()));
    let writes = Arc::new(Mutex::new(LatencyStats::new()));
    let traffic = concurrent_traffic(db_path.clone(), stop.clone(), reads.clone(), writes.clone());
    std::thread::sleep(Duration::from_millis(20));

    let mut connection = Connection::open(&db_path).unwrap();
    let snapshot = load_train_dataset_snapshot(&connection, WORKSPACE, &dataset_id).unwrap();
    let request = TrainSteelDatasetRequest {
        dataset_id,
        target_column: 1,
        feature_columns: vec![0],
        split_policy: None,
        algorithm: Some("linear_regression".into()),
    };
    let prepare_started = Instant::now();
    let prepared = prepare_steel_training_task(snapshot, &request).expect("prepare");
    let prepare_ms = prepare_started.elapsed().as_millis();

    // Concurrent traffic is required during prepare; stop it before the short
    // write transaction so enqueue success is measurable without flaky lock races.
    stop.store(true, Ordering::SeqCst);
    traffic.join().unwrap();
    let read_stats = reads.lock().unwrap().to_json();
    let write_stats = writes.lock().unwrap().to_json();
    assert_eq!(read_stats["failures"], 0);
    assert_eq!(write_stats["failures"], 0);
    assert!(write_stats["samples"].as_u64().unwrap_or(0) > 0);

    connection
        .busy_timeout(Duration::from_millis(5_000))
        .expect("busy timeout");
    let tx_started = Instant::now();
    let task = research_artifacts::atomic_mut(&mut connection, |c| {
        enqueue_prepared_steel_training(c, WORKSPACE, &prepared)
    })
    .expect("workflow-style enqueue");
    let tx_ms = tx_started.elapsed().as_millis();
    write_report(
        "workflow",
        json!({
            "entry": "workflow_prepare_outside_atomic_mut",
            "prepare_ms": prepare_ms,
            "atomic_mut_tx_ms": tx_ms,
            "task_id": task.id.to_string(),
            "concurrent_during_prepare": {
                "reads": read_stats,
                "writes": write_stats,
            },
            "note": "concurrent read/write ran during prepare; stopped before atomic_mut enqueue",
        }),
    );
    let _ = std::fs::remove_dir_all(root);
}
