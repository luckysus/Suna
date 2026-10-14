use crate::research_workflow::{dry_run_on_connection, runtime, testing::fixture};
use crate::tasks::scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock};
use crate::tasks::{repository, TaskRecord, TaskState};
use rusqlite::Connection;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: SchedulerEvent) {}
}
pub(crate) fn new_scheduler(path: &std::path::Path) -> Scheduler {
    scheduler_with_override(path, None)
}
pub(crate) fn scheduler_with_override(
    path: &std::path::Path,
    replacement: Option<Arc<dyn crate::tasks::scheduler::TaskHandler>>,
) -> Scheduler {
    scheduler_with_override_and_clock(path, replacement, Arc::new(SystemClock))
}
pub(crate) fn scheduler_with_override_and_clock(
    path: &std::path::Path,
    replacement: Option<Arc<dyn crate::tasks::scheduler::TaskHandler>>,
    clock: Arc<dyn crate::tasks::scheduler::Clock>,
) -> Scheduler {
    let worker = worker_config();
    let mut handlers = crate::compute::handler::compute_task_handlers(Some(worker));
    if let Some(replacement) = replacement {
        handlers.retain(|h| h.kind() != replacement.kind());
        handlers.push(replacement);
    }
    handlers.push(Arc::new(runtime::ModelingWorkflowHandler::new(path.into())));
    Scheduler::new(
        path.into(),
        "local".into(),
        SchedulerConfig {
            max_workers: 1,
            max_attempts: 1,
            retry_base: Duration::from_millis(1),
            retry_max: Duration::from_millis(1),
            poll_interval: Duration::from_millis(10),
        },
        clock,
        handlers,
        Arc::new(Sink),
    )
    .unwrap()
}
pub(crate) fn record(c: &Connection, id: uuid::Uuid) -> TaskRecord {
    repository::get(c, "local", id).unwrap().unwrap()
}
pub(crate) fn wait_until(
    s: &mut Scheduler,
    c: &Connection,
    id: uuid::Uuid,
    condition: impl Fn(&TaskRecord) -> bool,
) -> TaskRecord {
    let until = Instant::now() + Duration::from_secs(90);
    loop {
        s.tick().unwrap();
        let r = record(c, id);
        if condition(&r) {
            return r;
        }
        assert!(Instant::now() < until, "workflow deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
}
pub(crate) fn completed(s: &mut Scheduler, c: &Connection, id: uuid::Uuid) -> TaskRecord {
    let r = wait_until(s, c, id, |r| {
        matches!(
            r.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    });
    assert_eq!(
        r.state,
        TaskState::Completed,
        "workflow error: {:?}",
        r.error_code
    );
    r
}
pub(crate) fn setup() -> (crate::research_workflow::testing::Fixture, PathBuf) {
    let mut f = fixture();
    f.template.variables = vec![
        json!({"name":"c","unit":"wt%","min":0.1,"max":0.6}),
        json!({"name":"temperature","unit":"degC","min":700,"max":750}),
    ];
    let database = f.root.join("工作流.sqlite3");
    f.connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL")
        .unwrap();

    crate::research_workflow::templates::save(&mut f.connection, "local", &f.template, 0).unwrap();
    (f, database)
}
pub(crate) fn submit(
    f: &mut crate::research_workflow::testing::Fixture,
    nonce: uuid::Uuid,
) -> TaskRecord {
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    runtime::submit(
        &mut f.connection,
        "local",
        &f.template,
        &f.input,
        preview["dryRunSha256"].as_str().unwrap(),
        nonce,
        true,
    )
    .unwrap()
}

pub(crate) fn worker_config() -> crate::compute::worker::WorkerConfig {
    let worker_root = std::env::var_os("SUNA_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))
        .join("compute-worker");
    let executable = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            worker_root.join(if cfg!(windows) {
                ".venv/Scripts/python.exe"
            } else {
                ".venv/bin/python"
            })
        });
    assert!(
        executable.is_file(),
        "locked Worker test environment required"
    );
    let mut worker = crate::compute::worker::WorkerConfig::new(executable);
    worker.args = vec!["-m".into(), "suna_worker".into()];
    worker.working_directory = Some(worker_root);
    worker
}

pub(crate) fn drain(s: &mut Scheduler) {
    s.control().request_shutdown();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        s.tick().unwrap();
        if s.pending_workers() == 0 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "workflow test workers must close SQLite before cleanup"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
