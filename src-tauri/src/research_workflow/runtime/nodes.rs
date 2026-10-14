use super::*;
pub(super) fn checkpoint(
    c: &mut Connection,
    task: &TaskRecord,
    cp: &Value,
    progress: u8,
    due: bool,
) -> Result<(), String> {
    let delay = if cfg!(test) { 100 } else { 2000 };
    let next = due.then(|| (Utc::now() + Duration::milliseconds(delay)).to_rfc3339());
    repository::checkpoint(
        c,
        &task.workspace_id,
        task.id,
        task.attempt,
        Some(&cp.to_string()),
        progress,
        next.as_deref(),
    )
    .map_err(|_| "workflow_checkpoint_stale")?;
    Ok(())
}

pub(super) fn child(
    c: &Connection,
    task: &TaskRecord,
    id: Uuid,
    project: &str,
    dataset: &str,
    kind: &str,
    role: &str,
    source_sha: &Value,
) -> Result<TaskRecord, String> {
    let record = repository::get(c, &task.workspace_id, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_child_missing")?;
    let payload: Value =
        serde_json::from_str(&record.payload_json).map_err(|_| "workflow_child_payload")?;
    let owns:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND background_task_id=?3)",params![task.workspace_id,project,id.to_string()],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
    if record.kind != kind || payload["payload"]["dataset_id"] != dataset || !owns {
        return Err("workflow_child_ownership".into());
    }
    if !crate::research_workflow::provenance::owned_nodes(c, task)?
        .iter()
        .any(|owned| owned.id == id)
    {
        return Err("workflow_child_ownership".into());
    }
    let bound:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_run_sources s JOIN research_artifacts a ON a.workspace_id=s.workspace_id AND a.id=s.artifact_id JOIN research_runs r ON r.workspace_id=s.workspace_id AND r.id=s.run_id WHERE s.workspace_id=?1 AND a.source_key=?2 AND s.relation=?3 AND r.background_task_id=?4)",params![task.workspace_id,format!("workflow-definition:{}",task.id),role,id.to_string()],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
    if !bound {
        return Err("workflow_child_role_mismatch".into());
    }
    if kind.starts_with("compute_train_") && payload["payload"]["data_version"] != *source_sha {
        return Err("workflow_child_source_changed".into());
    }
    Ok(record)
}

pub(super) fn pending(
    c: &mut Connection,
    task: &TaskRecord,
    cp: &mut Value,
    record: &TaskRecord,
) -> Result<bool, String> {
    if record.state == TaskState::Completed {
        return Ok(false);
    }
    if matches!(
        record.state,
        TaskState::Failed | TaskState::Cancelled | TaskState::Interrupted | TaskState::Paused
    ) {
        if cp["failed_child"] == record.id.to_string() {
            repository::retry_manually(
                c,
                &task.workspace_id,
                record.id,
                record.attempt,
                record.state,
            )
            .map_err(|_| "workflow_child_retry_failed")?;
            cp["failed_child"] = Value::Null;
        } else {
            cp["failed_child"] = json!(record.id);
            checkpoint(c, task, cp, task.progress, false)?;
            return Err("workflow_child_failed".into());
        }
    }
    checkpoint(c, task, cp, task.progress, true)?;
    Ok(true)
}

pub(super) fn parse_id(value: &Value) -> Result<Uuid, String> {
    Uuid::parse_str(value.as_str().ok_or("workflow_checkpoint_invalid")?)
        .map_err(|_| "workflow_checkpoint_invalid".into())
}

pub(super) fn ensure_active(c: &Connection, task: &TaskRecord) -> Result<(), String> {
    let current = repository::get(c, &task.workspace_id, task.id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    if current.state != TaskState::Running
        || current.attempt != task.attempt
        || current.cancel_requested
        || current.payload_json != task.payload_json
    {
        return Err("workflow_checkpoint_stale".into());
    }
    Ok(())
}

pub fn request_cancellation(c: &mut Connection, ws: &str, id: Uuid) -> Result<(), String> {
    let task = repository::get(c, ws, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    if task.kind != KIND {
        return Err("workflow_task_invalid".into());
    }
    crate::research_workflow::provenance::owned_nodes(c, &task)?;
    match task.state {
        TaskState::Running => {
            repository::request_running_cancellation(c, ws, id, task.attempt)
                .map_err(|_| "workflow_cancel_conflict")?;
        }
        TaskState::Queued
        | TaskState::WaitingExternal
        | TaskState::Paused
        | TaskState::Interrupted => {
            repository::transition(
                c,
                ws,
                id,
                task.attempt,
                task.state,
                TaskState::Cancelled,
                None,
            )
            .map_err(|_| "workflow_cancel_conflict")?;
        }
        _ => return Err("workflow_not_cancellable".into()),
    }
    // Freeze the parent first. Existing repository CAS transactions stay intact;
    // interrupted cancellation is finished by maintenance on the next tick.
    let latest = repository::get(c, ws, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    crate::research_workflow::provenance::cancel_owned_nodes(c, &latest).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_workflow::{
        provenance,
        runtime_support::{setup, submit as submit_root},
        templates,
    };
    #[test]
    fn checkpoints_cannot_swap_two_same_algorithm_performance_models() {
        let (mut f, _) = setup();
        f.template.models[1].algorithm = "linear_regression".into();
        let saved = templates::save(&mut f.connection, "local", &f.template, 1).unwrap();
        f.template = serde_json::from_value(saved["template"].clone()).unwrap();
        let root = submit_root(&mut f, Uuid::new_v4());
        let mut nodes = Vec::new();
        for (index, model) in f.template.models.iter().enumerate() {
            let request = TrainSteelDatasetRequest {
                dataset_id: f.input.dataset_id.clone(),
                target_column: model.target_column,
                feature_columns: model.feature_columns.clone(),
                split_policy: Some(model.split_policy.clone()),
                algorithm: Some(model.algorithm.clone()),
            };
            let node = delivery::scoped(&f.connection, Some(&f.input.project_id), || {
                logic::train_steel_dataset_on_connection(&f.connection, "local", &request)
            })
            .unwrap();
            provenance::bind_node(
                &f.connection,
                &root,
                &node,
                &format!("workflow_train_{index}"),
            )
            .unwrap();
            nodes.push(node);
        }
        let source = json!(f.input.dataset_sha256);
        assert_eq!(
            child(
                &f.connection,
                &root,
                nodes[1].id,
                &f.input.project_id,
                &f.input.dataset_id,
                "compute_train_linear_regression",
                "workflow_train_0",
                &source
            )
            .unwrap_err(),
            "workflow_child_role_mismatch"
        );
        assert_eq!(
            child(
                &f.connection,
                &root,
                nodes[0].id,
                &f.input.project_id,
                &f.input.dataset_id,
                "compute_train_linear_regression",
                "workflow_train_0",
                &source
            )
            .unwrap()
            .id,
            nodes[0].id
        );
        assert_eq!(
            f.connection
                .query_row("SELECT COUNT(*) FROM steel_models", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
}
