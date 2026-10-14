use crate::{dry_run_on_connection, runtime, tests::fixture};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use suna::storage::repositories::research_projects as projects;
use suna::tasks::scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock};
use suna::tasks::{repository, TaskRecord, TaskState};

struct Sink;
impl EventSink for Sink {
    fn emit(&self, _: SchedulerEvent) {}
}
pub(crate) fn new_scheduler(path: &std::path::Path) -> Scheduler {
    scheduler_with_override(path, None)
}
pub(crate) fn scheduler_with_override(
    path: &std::path::Path,
    replacement: Option<Arc<dyn suna::tasks::scheduler::TaskHandler>>,
) -> Scheduler {
    // Mirrors the existing Suna test helper; production uses support/worker.rs.
    let worker_root = std::env::var_os("SUNA_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.."))
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
    let mut worker = suna::compute::worker::WorkerConfig::new(executable);
    worker.args = vec!["-m".into(), "suna_worker".into()];
    worker.working_directory = Some(worker_root);
    let mut handlers = suna::compute::handler::compute_task_handlers(Some(worker));
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
        Arc::new(SystemClock),
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
fn completed(s: &mut Scheduler, c: &Connection, id: uuid::Uuid) -> TaskRecord {
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
pub(crate) fn setup() -> (crate::tests::Fixture, PathBuf) {
    let mut f = fixture();
    f.template.variables = vec![
        json!({"name":"c","unit":"wt%","min":0.1,"max":0.6}),
        json!({"name":"temperature","unit":"degC","min":700,"max":750}),
    ];
    let database = f.root.join("工作流.sqlite3");
    f.connection
        .execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL")
        .unwrap();
    crate::templates::install_in_temporary_database(&f.connection).unwrap();
    crate::templates::save(&mut f.connection, "local", &f.template, 0).unwrap();
    (f, database)
}
pub(crate) fn submit(f: &mut crate::tests::Fixture, nonce: uuid::Uuid) -> TaskRecord {
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
#[test]
fn actual_scheduler_runs_two_models_joint_optimization_and_only_proposed_plan() {
    let (mut f, path) = setup();
    let nonce = uuid::Uuid::new_v4();
    let parent = submit(&mut f, nonce);
    assert_eq!(submit(&mut f, nonce).id, parent.id);
    let approved_template =
        crate::templates::get(&f.connection, "local", &f.template.id, 1).unwrap();
    let mut next_version = f.template.clone();
    next_version.name = "新修订（不改变已确认运行）".into();
    next_version.seed += 1;
    crate::templates::save(&mut f.connection, "local", &next_version, 1).unwrap();
    let other = projects::save(
        &mut f.connection,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "另一个合成项目".into(),
            description: String::new(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    f.connection
        .execute(
            "INSERT INTO settings VALUES('local','suna.research.current_project',?1,'now')",
            [json!(other.id).to_string()],
        )
        .unwrap();
    let mut scheduler = new_scheduler(&path);
    let done = completed(&mut scheduler, &f.connection, parent.id);
    let cp: Value = serde_json::from_str(done.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(cp["children"].as_array().unwrap().len(), 2);
    assert_eq!(cp["models"].as_array().unwrap().len(), 2);
    assert_eq!(cp["result"]["confirmationRequiredToAccept"], true);
    assert_eq!(cp["result"]["measurementsCreated"], false);
    let owned = crate::provenance::owned_nodes(&f.connection, &done).unwrap();
    assert_eq!(owned.len(), 3);
    let summary_id = cp["resultArtifactId"].as_str().unwrap();
    let summary =
        suna::storage::repositories::research_artifacts::get(&f.connection, "local", summary_id)
            .unwrap();
    let snapshot = summary.payload.as_ref().unwrap();
    let public_status =
        crate::status::get(&f.connection, "local", &f.input.project_id, done.id).unwrap();
    assert_eq!(public_status["task"]["state"], "completed");
    assert_eq!(public_status["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(public_status["result"]["artifactId"], summary_id);
    assert_eq!(public_status["result"]["registeredModelIds"], cp["models"]);
    assert_eq!(snapshot["templateRevision"], 1);
    assert_eq!(
        snapshot["templateContentSha256"],
        approved_template["contentSha256"]
    );
    assert_eq!(snapshot["measurementsCreated"], false);
    assert!(!snapshot
        .to_string()
        .contains(f.root.to_string_lossy().as_ref()));
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_artifact_edges WHERE to_artifact_id=?1",
                [summary_id],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        5
    );
    let reopened = Connection::open(&path).unwrap();
    assert_eq!(
        suna::storage::repositories::research_artifacts::get(&reopened, "local", summary_id)
            .unwrap()
            .content_sha256,
        summary.content_sha256
    );
    drop(reopened);
    assert_eq!(
        f.connection
            .query_row(
                "SELECT state FROM experiments WHERE id=?1",
                [cp["plan"].as_str().unwrap()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "proposed"
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM steel_models", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_runs WHERE project_id=?1",
                [&other.id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_runs WHERE project_id=?1",
                [&f.input.project_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT value_json FROM settings WHERE key='suna.research.current_project'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        json!(other.id).to_string()
    );
    assert_eq!(submit(&mut f, nonce).id, parent.id);
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(scheduler);
}
#[test]
fn starts_require_confirmed_unchanged_preview_and_nonce_cannot_change_request() {
    let (mut f, _) = setup();
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    let sha = preview["dryRunSha256"].as_str().unwrap();
    let nonce = uuid::Uuid::new_v4();
    assert!(runtime::submit(
        &mut f.connection,
        "local",
        &f.template,
        &f.input,
        sha,
        nonce,
        false
    )
    .is_err());
    assert!(runtime::submit(
        &mut f.connection,
        "local",
        &f.template,
        &f.input,
        &"a".repeat(64),
        nonce,
        true
    )
    .is_err());
    let parent = submit(&mut f, nonce);
    f.template.seed += 1;
    let saved = crate::templates::save(&mut f.connection, "local", &f.template, 1).unwrap();
    f.template = serde_json::from_value(saved["template"].clone()).unwrap();
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    assert_eq!(
        runtime::submit(
            &mut f.connection,
            "local",
            &f.template,
            &f.input,
            preview["dryRunSha256"].as_str().unwrap(),
            nonce,
            true
        )
        .unwrap_err(),
        "workflow_nonce_conflict"
    );
    assert_eq!(record(&f.connection, parent.id).state, TaskState::Queued);
}
#[test]
fn cancel_between_nodes_cancels_queued_owned_children_and_creates_no_measurement() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let mut scheduler = new_scheduler(&path);
    let r = wait_until(&mut scheduler, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    let cp: Value = serde_json::from_str(r.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(cp["children"].as_array().unwrap().len(), 1);
    runtime::request_cancellation(&mut f.connection, "local", parent.id).unwrap();
    assert_eq!(record(&f.connection, parent.id).state, TaskState::Cancelled);
    let id = uuid::Uuid::parse_str(cp["children"][0].as_str().unwrap()).unwrap();
    let child = record(&f.connection, id);
    assert!(child.state == TaskState::Cancelled || child.cancel_requested);
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(scheduler);
}
#[test]
fn restart_reuses_registered_versions_after_registration_checkpoint_gap() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let mut scheduler = new_scheduler(&path);
    let r = wait_until(&mut scheduler, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal && r.progress == 40
    });
    drop(scheduler);
    let mut cp: Value = serde_json::from_str(r.checkpoint_json.as_deref().unwrap()).unwrap();
    cp["models"] = json!([]);
    f.connection
        .execute(
            "UPDATE background_tasks SET checkpoint_json=?1 WHERE id=?2",
            rusqlite::params![cp.to_string(), parent.id.to_string()],
        )
        .unwrap();
    let mut resumed = new_scheduler(&path);
    resumed.recover().unwrap();
    completed(&mut resumed, &f.connection, parent.id);
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM steel_models", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(resumed);
}

#[test]
fn failed_child_is_retried_through_existing_repository_without_duplicate_nodes() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let mut engine = new_scheduler(&path);
    let r = wait_until(&mut engine, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    drop(engine);
    let cp: Value = serde_json::from_str(r.checkpoint_json.as_deref().unwrap()).unwrap();
    let child = cp["children"][0].as_str().unwrap();
    // A deterministic fault in the synthetic task database, never a CI guess.
    f.connection.execute("UPDATE background_tasks SET state='failed',error_code='synthetic_worker_failure',cancel_requested=0 WHERE id=?1",[child]).unwrap();
    let mut resumed = new_scheduler(&path);
    let failed = wait_until(&mut resumed, &f.connection, parent.id, |r| {
        r.state == TaskState::Failed
    });
    assert_eq!(failed.error_code.as_deref(), Some("workflow_child_failed"));
    repository::retry_manually(
        &mut f.connection,
        "local",
        parent.id,
        failed.attempt,
        failed.state,
    )
    .unwrap();
    completed(&mut resumed, &f.connection, parent.id);
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM steel_models", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(resumed);
}

#[test]
fn expired_budget_or_unconfirmed_root_cannot_create_child_tasks() {
    for expired in [false, true] {
        let (mut f, path) = setup();
        let parent = submit(&mut f, uuid::Uuid::new_v4());
        if expired {
            let mut cp: Value =
                serde_json::from_str(parent.checkpoint_json.as_deref().unwrap()).unwrap();
            cp["deadline"] = json!("2000-01-01T00:00:00Z");
            f.connection
                .execute(
                    "UPDATE background_tasks SET checkpoint_json=?1 WHERE id=?2",
                    rusqlite::params![cp.to_string(), parent.id.to_string()],
                )
                .unwrap();
        } else {
            let mut payload: Value = serde_json::from_str(&parent.payload_json).unwrap();
            payload["confirmed"] = json!(false);
            f.connection
                .execute(
                    "UPDATE background_tasks SET payload_json=?1 WHERE id=?2",
                    rusqlite::params![payload.to_string(), parent.id.to_string()],
                )
                .unwrap();
        }
        let mut engine = new_scheduler(&path);
        let failed = wait_until(&mut engine, &f.connection, parent.id, |r| {
            r.state == TaskState::Failed
        });
        assert_eq!(
            failed.error_code.as_deref(),
            Some(if expired {
                "workflow_timeout_requires_new_confirmed_run"
            } else {
                "workflow_start_confirmation_required"
            })
        );
        assert_eq!(
            f.connection
                .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(engine);
    }
}
