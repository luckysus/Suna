use crate::research_workflow::{dry_run_on_connection, runtime};
use crate::tasks::scheduler::{
    HandlerContext, HandlerError, HandlerFuture, HandlerOutcome, TaskHandler,
};
use crate::tasks::{repository, TaskState};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use super::runtime_support::*;

// A real handler writes the actual registered model/checkpoint first. Only the
// fixture's delivery timing is controlled; training, registration, guards and
// recovery still execute the production path.
struct RegistrationBoundary {
    inner: runtime::ModelingWorkflowHandler,
    database: PathBuf,
    reached: Arc<AtomicBool>,
    hold_until_shutdown: bool,
}
impl TaskHandler for RegistrationBoundary {
    fn kind(&self) -> &str {
        runtime::KIND
    }
    fn resumable(&self) -> bool {
        self.inner.resumable()
    }
    fn maintain(
        &self,
        c: &mut rusqlite::Connection,
        ws: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<crate::tasks::TaskRecord>, HandlerError> {
        self.inner.maintain(c, ws, now)
    }
    fn run(&self, task: crate::tasks::TaskRecord, context: HandlerContext) -> HandlerFuture {
        let run = self.inner.run(task.clone(), context.clone());
        let path = self.database.clone();
        let reached = self.reached.clone();
        let hold = self.hold_until_shutdown;
        Box::pin(async move {
            let result = run.await?;
            let at_registration = {
                let c = rusqlite::Connection::open(path).unwrap();
                let current = record(&c, task.id);
                if result == HandlerOutcome::WaitingExternal && current.progress == 40 {
                    let cp: Value =
                        serde_json::from_str(current.checkpoint_json.as_deref().unwrap()).unwrap();
                    assert_eq!(cp["models"].as_array().unwrap().len(), 1);
                    // Simulate delivery after its due time without a machine-speed
                    // sleep. Only this synthetic database is changed.
                    assert_eq!(c.execute("UPDATE background_tasks SET next_run_at='2000-01-01T00:00:00Z' WHERE id=?1",[task.id.to_string()]).unwrap(),1);
                    true
                } else {
                    false
                }
            };
            if at_registration {
                reached.store(true, Ordering::SeqCst);
                if hold {
                    let until = Instant::now() + Duration::from_secs(30);
                    while !context.shutdown_requested() {
                        if Instant::now() >= until {
                            return Err(HandlerError::permanent(
                                "synthetic_registration_barrier_not_stopped",
                            ));
                        }
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                }
            }
            Ok(result)
        })
    }
}
fn registration_scheduler(
    path: &std::path::Path,
    hold: bool,
) -> (crate::tasks::scheduler::Scheduler, Arc<AtomicBool>) {
    let reached = Arc::new(AtomicBool::new(false));
    let handler = RegistrationBoundary {
        inner: runtime::ModelingWorkflowHandler::new(path.into()),
        database: path.into(),
        reached: reached.clone(),
        hold_until_shutdown: hold,
    };
    (
        scheduler_with_override(path, Some(Arc::new(handler))),
        reached,
    )
}

#[test]
fn completed_registration_can_be_reclaimed_before_a_poll_observes_waiting_state() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let (mut scheduler, reached) = registration_scheduler(&path, false);
    let observed = AtomicBool::new(false);
    let done = wait_until(&mut scheduler, &f.connection, parent.id, |r| {
        if r.state == TaskState::WaitingExternal && r.progress == 40 {
            observed.store(true, Ordering::SeqCst);
        }
        matches!(
            r.state,
            TaskState::Completed | TaskState::Failed | TaskState::Cancelled
        )
    });
    assert_eq!(done.state, TaskState::Completed);
    assert!(
        reached.load(Ordering::SeqCst),
        "actual model registration must occur"
    );
    assert!(
        !observed.load(Ordering::SeqCst),
        "a due checkpoint may be reclaimed in the same scheduler tick"
    );
    drain(&mut scheduler);
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
    let saved =
        crate::research_workflow::templates::save(&mut f.connection, "local", &f.template, 1)
            .unwrap();
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
    drain(&mut scheduler);
    drop(scheduler);
}
#[test]
fn restart_reuses_registered_versions_after_registration_checkpoint_gap() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let (mut scheduler, reached) = registration_scheduler(&path, true);
    wait_until(&mut scheduler, &f.connection, parent.id, |r| {
        reached.load(Ordering::SeqCst) && r.state == TaskState::Running && r.progress == 40
    });
    drain(&mut scheduler);
    drop(scheduler);
    // Read the durable checkpoint after the worker and its database handles have
    // stopped. WaitingExternal may be reclaimed in the very same tick; the test
    // must not depend on observing that short-lived state or a stale snapshot.
    let r = record(&f.connection, parent.id);
    assert_eq!(r.state, TaskState::Interrupted);
    assert_eq!(r.progress, 40);
    let mut cp: Value = serde_json::from_str(r.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(cp["models"].as_array().unwrap().len(), 1);
    assert_eq!(cp["children"].as_array().unwrap().len(), 2);
    assert!(cp["optimization"].is_null());
    let model_id = cp["models"][0].as_str().unwrap().to_owned();
    let registered =
        crate::storage::repositories::steel_models::get(&f.connection, "local", &model_id)
            .unwrap()
            .unwrap();
    assert_eq!(registered.version, 1);
    assert_eq!(
        registered.source_task_id.as_deref(),
        cp["children"][0].as_str()
    );
    assert!(!registered.is_active);
    let cards = crate::storage::repositories::steel_models::scientific_cards(
        &f.connection,
        "local",
        &model_id,
    )
    .unwrap();
    let artifact_id: String = f.connection.query_row(
        "SELECT id FROM research_artifacts WHERE workspace_id='local' AND project_id=?1 AND kind='model_snapshot' AND source_key=?2",
        rusqlite::params![f.input.project_id, format!("registered:{model_id}")],
        |r| r.get(0),
    ).unwrap();
    let frozen =
        crate::storage::repositories::research_artifacts::get(&f.connection, "local", &artifact_id)
            .unwrap();
    // Inject only the registration/checkpoint gap, never alter registered model
    // bytes, cards, provenance, cancellation guards or the approved deadline.
    cp["models"] = json!([]);
    assert_eq!(
        f.connection
            .execute(
                "UPDATE background_tasks SET checkpoint_json=?1 WHERE id=?2",
                rusqlite::params![cp.to_string(), parent.id.to_string()],
            )
            .unwrap(),
        1
    );
    let mut resumed = new_scheduler(&path);
    resumed.recover().unwrap();
    let done = completed(&mut resumed, &f.connection, parent.id);
    let recovered: Value = serde_json::from_str(done.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(recovered["models"][0], model_id);
    assert_eq!(recovered["models"].as_array().unwrap().len(), 2);
    assert_eq!(recovered["children"], cp["children"]);
    assert_eq!(recovered["result"]["confirmationRequiredToAccept"], true);
    assert_eq!(recovered["result"]["measurementsCreated"], false);
    drain(&mut resumed);
    drop(resumed);
    let reopened = rusqlite::Connection::open(&path).unwrap();
    let reused = crate::storage::repositories::steel_models::get(&reopened, "local", &model_id)
        .unwrap()
        .unwrap();
    assert_eq!(reused.model_sha256, registered.model_sha256);
    assert!(
        reused == registered,
        "model ID, version, source task, exact model/manifest/artifact bytes and activation must survive recovery"
    );
    assert_eq!(
        crate::storage::repositories::steel_models::scientific_cards(&reopened, "local", &model_id)
            .unwrap(),
        cards
    );
    let preserved =
        crate::storage::repositories::research_artifacts::get(&reopened, "local", &artifact_id)
            .unwrap();
    assert_eq!(preserved.content_sha256, frozen.content_sha256);
    assert_eq!(
        serde_json::to_value(preserved).unwrap(),
        serde_json::to_value(frozen).unwrap()
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
    assert_eq!(
        reopened
            .query_row(
                "SELECT state FROM experiments WHERE id=?1",
                [recovered["plan"].as_str().unwrap()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "proposed"
    );
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        reopened
            .query_row(
                "SELECT COUNT(*) FROM steel_models WHERE is_active=1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    drop(reopened);
}

#[test]
fn failed_child_is_retried_through_existing_repository_without_duplicate_nodes() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let mut engine = new_scheduler(&path);
    let r = wait_until(&mut engine, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    drain(&mut engine);
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
        drain(&mut engine);
        drop(engine);
    }
}
