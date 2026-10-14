//! Fault injection validates lifecycle guards; real modeling remains in runtime_tests.
use crate::app::compute_commands::logic::{self, TrainSteelDatasetRequest};
use crate::research_workflow::{lifecycle, provenance, runtime, runtime_tests::*};
use crate::storage::repositories::research_delivery;
use crate::tasks::scheduler::{
    HandlerContext, HandlerError, HandlerFuture, HandlerOutcome, TaskHandler,
};
use crate::tasks::{repository, TaskRecord, TaskState};
use chrono::{Duration as TimeDelta, Utc};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct ControlledLongNode {
    started: Arc<AtomicBool>,
}
struct AdjustableClock(std::sync::Mutex<chrono::DateTime<Utc>>);
impl crate::tasks::scheduler::Clock for AdjustableClock {
    fn now(&self) -> chrono::DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}
impl TaskHandler for ControlledLongNode {
    fn kind(&self) -> &str {
        "compute_train_linear_regression"
    }
    fn resumable(&self) -> bool {
        true
    }
    fn run(&self, _task: TaskRecord, ctx: HandlerContext) -> HandlerFuture {
        let started = self.started.clone();
        Box::pin(async move {
            started.store(true, Ordering::SeqCst);
            let limit = Instant::now() + Duration::from_secs(5);
            while Instant::now() < limit {
                if ctx
                    .cancellation_requested()
                    .map_err(|_| HandlerError::permanent("synthetic_guard_storage"))?
                {
                    return Ok(HandlerOutcome::Cancelled);
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(HandlerError::permanent("synthetic_guard_not_cancelled"))
        })
    }
}

fn unrelated(f: &mut crate::research_workflow::testing::Fixture) -> TaskRecord {
    let m = &f.template.models[0];
    let request = TrainSteelDatasetRequest {
        dataset_id: f.input.dataset_id.clone(),
        target_column: m.target_column,
        feature_columns: m.feature_columns.clone(),
        algorithm: Some(m.algorithm.clone()),
        split_policy: Some(m.split_policy.clone()),
    };
    let node = research_delivery::scoped(&f.connection, Some(&f.input.project_id), || {
        logic::train_steel_dataset_on_connection(&f.connection, "local", &request)
    })
    .unwrap();
    repository::transition(
        &mut f.connection,
        "local",
        node.id,
        node.attempt,
        node.state,
        TaskState::Paused,
        None,
    )
    .unwrap()
}

#[test]
fn active_single_worker_is_cancelled_on_approved_deadline_without_touching_other_jobs() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let other = unrelated(&mut f);
    let started = Arc::new(AtomicBool::new(false));
    let clock = Arc::new(AdjustableClock(std::sync::Mutex::new(Utc::now())));
    let mut s = scheduler_with_override_and_clock(
        &path,
        Some(Arc::new(ControlledLongNode {
            started: started.clone(),
        })),
        clock.clone(),
    );
    let waiting = wait_until(&mut s, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    let cp: Value = serde_json::from_str(waiting.checkpoint_json.as_deref().unwrap()).unwrap();
    let child = uuid::Uuid::parse_str(cp["children"][0].as_str().unwrap()).unwrap();
    wait_until(&mut s, &f.connection, child, |r| {
        r.state == TaskState::Running && started.load(Ordering::SeqCst)
    });
    let (_, deadline) = provenance::definition(&f.connection, &parent).unwrap();
    *clock.0.lock().unwrap() = deadline + TimeDelta::seconds(1);
    // The real Scheduler maintenance hook runs while its only worker is busy.
    s.tick().unwrap();
    assert_eq!(record(&f.connection, parent.id).state, TaskState::Failed);
    assert_eq!(
        record(&f.connection, parent.id).error_code.as_deref(),
        Some("workflow_timeout_requires_new_confirmed_run")
    );
    assert!(record(&f.connection, child).cancel_requested);
    wait_until(&mut s, &f.connection, child, |r| {
        r.state == TaskState::Cancelled
    });
    assert_eq!(record(&f.connection, other.id).state, TaskState::Paused);
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    drain(&mut s);
    drop(s);
}

#[test]
fn checkpoint_tampering_cannot_cancel_unrelated_same_project_task() {
    let (mut f, path) = setup();
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let other = unrelated(&mut f);
    let mut s = new_scheduler(&path);
    let waiting = wait_until(&mut s, &f.connection, parent.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    let mut cp: Value = serde_json::from_str(waiting.checkpoint_json.as_deref().unwrap()).unwrap();
    let owned = uuid::Uuid::parse_str(cp["children"][0].as_str().unwrap()).unwrap();
    cp["children"] = json!([other.id]);
    f.connection
        .execute(
            "UPDATE background_tasks SET checkpoint_json=?1 WHERE id=?2",
            rusqlite::params![cp.to_string(), parent.id.to_string()],
        )
        .unwrap();
    runtime::request_cancellation(&mut f.connection, "local", parent.id).unwrap();
    let actual = record(&f.connection, owned);
    assert!(actual.state == TaskState::Cancelled || actual.cancel_requested);
    assert_eq!(record(&f.connection, other.id).state, TaskState::Paused);
    assert!(lifecycle::sweep(&mut f.connection, "local", Utc::now())
        .unwrap()
        .is_empty());
    drain(&mut s);
    drop(s);
}

#[test]
fn unsaved_edits_cannot_execute_under_a_saved_template_revision() {
    let (mut f, _) = setup();
    f.template.seed += 1;
    let preview = crate::research_workflow::dry_run_on_connection(
        &f.connection,
        "local",
        &f.template,
        &f.input,
    )
    .unwrap();
    assert_eq!(
        runtime::submit(
            &mut f.connection,
            "local",
            &f.template,
            &f.input,
            preview["dryRunSha256"].as_str().unwrap(),
            uuid::Uuid::new_v4(),
            true
        )
        .unwrap_err(),
        "workflow_template_revision_changed"
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn an_invalid_root_cannot_block_deadline_maintenance_of_another_valid_run() {
    let (mut f, path) = setup();
    let bad = submit(&mut f, uuid::Uuid::new_v4());
    let valid = submit(&mut f, uuid::Uuid::new_v4());
    let mut s = new_scheduler(&path);
    wait_until(&mut s, &f.connection, bad.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    wait_until(&mut s, &f.connection, valid.id, |r| {
        r.state == TaskState::WaitingExternal
    });
    // Synthetic corruption is injected only in the mutable parent request.
    f.connection.execute("UPDATE background_tasks SET payload_json=json_set(payload_json,'$.nonce','synthetic-invalid-proof') WHERE id=?1",[bad.id.to_string()]).unwrap();
    let (_, deadline) = provenance::definition(&f.connection, &valid).unwrap();
    let changes =
        lifecycle::sweep(&mut f.connection, "local", deadline + TimeDelta::seconds(1)).unwrap();
    assert!(changes
        .iter()
        .any(|t| t.id == valid.id && t.state == TaskState::Failed));
    assert!(!changes.iter().any(|t| t.id == bad.id));
    assert_eq!(
        record(&f.connection, bad.id).state,
        TaskState::WaitingExternal
    );
    for child in provenance::owned_nodes(&f.connection, &valid).unwrap() {
        assert!(
            child.state == TaskState::Completed
                || child.state == TaskState::Cancelled
                || child.cancel_requested
        );
    }
    drain(&mut s);
}
