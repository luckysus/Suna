//! Maintenance called by the existing Scheduler, including while all workers are busy.
use crate::tasks::{repository, TaskRecord, TaskState};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn sweep(c: &mut Connection, ws: &str, now: DateTime<Utc>) -> Result<Vec<TaskRecord>, String> {
    // Only roots with active owned children need maintenance. Cancelled roots
    // are retained here so cancellation still completes after an app restart.
    let ids = {
        let mut q = c.prepare("SELECT DISTINCT t.id FROM background_tasks t JOIN research_artifacts a ON a.workspace_id=t.workspace_id AND a.source_key='workflow-definition:'||t.id AND a.kind='analysis_result' JOIN research_run_sources s ON s.workspace_id=a.workspace_id AND s.artifact_id=a.id JOIN research_runs r ON r.workspace_id=s.workspace_id AND r.id=s.run_id JOIN background_tasks child ON child.workspace_id=r.workspace_id AND child.id=r.background_task_id WHERE t.workspace_id=?1 AND t.kind=?2 AND s.relation LIKE 'workflow_%' AND child.state IN ('queued','running','waiting_external','paused','interrupted') ORDER BY t.created_at,t.id LIMIT 129")
            .map_err(|_|"workflow_storage_error")?;
        let rows = q
            .query_map(params![ws, crate::research_workflow::runtime::KIND], |r| {
                r.get::<_, String>(0)
            })
            .map_err(|_| "workflow_storage_error")?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "workflow_storage_error")?;
        rows
    };
    let mut changed = Vec::new();
    for id in ids.into_iter().take(128) {
        match maintain_root(c, ws, &id, now) {
            Ok(records) => changed.extend(records),
            Err(_) => log::warn!(
                "workflow maintenance incomplete: root={} code=workflow_maintenance_incomplete",
                id
            ),
        }
    }
    Ok(changed)
}
fn maintain_root(
    c: &mut Connection,
    ws: &str,
    id: &str,
    now: DateTime<Utc>,
) -> Result<Vec<TaskRecord>, String> {
    let mut changed = Vec::new();
    let root = repository::get(
        c,
        ws,
        Uuid::parse_str(&id).map_err(|_| "workflow_task_invalid")?,
    )
    .map_err(|_| "workflow_storage_error")?
    .ok_or("workflow_task_missing")?;
    let (_, deadline) = crate::research_workflow::provenance::definition(c, &root)?;
    if matches!(root.state, TaskState::Cancelled) || root.cancel_requested || now >= deadline {
        if now >= deadline
            && matches!(root.state, TaskState::WaitingExternal | TaskState::Running)
            && !root.cancel_requested
        {
            let updated = repository::transition(
                c,
                ws,
                root.id,
                root.attempt,
                root.state,
                TaskState::Failed,
                Some("workflow_timeout_requires_new_confirmed_run"),
            )
            .map_err(|_| "workflow_checkpoint_stale")?;
            changed.push(updated);
        }
        changed.extend(crate::research_workflow::provenance::cancel_owned_nodes(
            c, &root,
        )?);
    }
    Ok(changed)
}
