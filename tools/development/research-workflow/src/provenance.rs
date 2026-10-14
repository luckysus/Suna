//! Existing immutable research artifacts bind each child to its approved parent.
//! No extra task state, model registry or scheduler is introduced by this draft.
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use suna::storage::repositories::research_artifacts as artifacts;
use suna::tasks::{repository, TaskRecord, TaskState};
use uuid::Uuid;

fn request(task: &TaskRecord) -> Result<Value, String> {
    serde_json::from_str(&task.payload_json).map_err(|_| "workflow_payload_invalid".into())
}

pub fn create_definition(c: &Connection, root: &TaskRecord) -> Result<(), String> {
    let input = request(root)?;
    let project = input["project_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    let run = artifacts::bind(c, &root.workspace_id, project, &root.id.to_string())?;
    let cp: Value = serde_json::from_str(
        root.checkpoint_json
            .as_deref()
            .ok_or("workflow_checkpoint_invalid")?,
    )
    .map_err(|_| "workflow_checkpoint_invalid")?;
    let id = artifacts::create(
        c,
        &root.workspace_id,
        project,
        "analysis_result",
        "已确认科研工作流输入",
        &format!("workflow-definition:{}", root.id),
        Some(&run),
        json!({"schemaVersion":"research-workflow-definition.v1","request":input,"approvedDeadline":cp["deadline"]}),
    )?;
    let dataset = input["dataset_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    if let Some(source) = artifacts::dataset_snapshot(c, &root.workspace_id, project, dataset)? {
        c.execute(
            "INSERT OR IGNORE INTO research_run_sources VALUES(?1,?2,?3,'uses_dataset')",
            params![root.workspace_id, run, source],
        )
        .map_err(|_| "workflow_storage_error")?;
        artifacts::edge(c, &root.workspace_id, &source, &id, "uses_dataset")?;
    }
    Ok(())
}

pub fn definition(c: &Connection, root: &TaskRecord) -> Result<(String, DateTime<Utc>), String> {
    let input = request(root)?;
    let project = input["project_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    let id: String = c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='analysis_result' AND source_key=?3",
        params![root.workspace_id, project, format!("workflow-definition:{}",root.id)], |r|r.get(0))
        .map_err(|_| "workflow_definition_missing")?;
    let artifact = artifacts::get(c, &root.workspace_id, &id)?;
    let payload = artifact.payload.ok_or("workflow_definition_invalid")?;
    if payload["schemaVersion"] != "research-workflow-definition.v1" || payload["request"] != input
    {
        return Err("workflow_definition_changed".into());
    }
    let deadline = DateTime::parse_from_rfc3339(
        payload["approvedDeadline"]
            .as_str()
            .ok_or("workflow_definition_invalid")?,
    )
    .map_err(|_| "workflow_definition_invalid")?
    .with_timezone(&Utc);
    Ok((id, deadline))
}

fn role_kind(input: &Value, role: &str) -> Result<&'static str, String> {
    let index = match role {
        "workflow_train_0" => 0,
        "workflow_train_1" => 1,
        "workflow_optimization" => return Ok("compute_optimize_constrained"),
        _ => return Err("workflow_child_role_invalid".into()),
    };
    Ok(
        if input["template"]["models"][index]["algorithm"] == "linear_regression" {
            "compute_train_linear_regression"
        } else {
            "compute_train_sklearn_model"
        },
    )
}

pub fn bind_node(
    c: &Connection,
    root: &TaskRecord,
    child: &TaskRecord,
    role: &str,
) -> Result<(), String> {
    let input = request(root)?;
    let (definition, _) = definition(c, root)?;
    let payload = request(child)?;
    if child.workspace_id != root.workspace_id
        || child.kind != role_kind(&input, role)?
        || payload["payload"]["dataset_id"] != input["dataset_id"]
    {
        return Err("workflow_child_ownership".into());
    }
    if role.starts_with("workflow_train_")
        && payload["payload"]["data_version"] != input["dataset_sha256"]
    {
        return Err("workflow_child_source_changed".into());
    }
    let run: String = c.query_row("SELECT id FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND background_task_id=?3",
        params![root.workspace_id,input["project_id"].as_str(),child.id.to_string()], |r|r.get(0))
        .map_err(|_| "workflow_child_ownership")?;
    let existing: u32 = c.query_row("SELECT COUNT(*) FROM research_run_sources WHERE workspace_id=?1 AND artifact_id=?2 AND relation=?3 AND run_id<>?4",
        params![root.workspace_id,definition,role,run],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
    if existing != 0 {
        return Err("workflow_duplicate_child_role".into());
    }
    c.execute(
        "INSERT OR IGNORE INTO research_run_sources VALUES(?1,?2,?3,?4)",
        params![root.workspace_id, run, definition, role],
    )
    .map_err(|_| "workflow_storage_error")?;
    Ok(())
}

pub fn owned_nodes(c: &Connection, root: &TaskRecord) -> Result<Vec<TaskRecord>, String> {
    let input = request(root)?;
    let (definition, _) = definition(c, root)?;
    let mut query = c.prepare("SELECT r.background_task_id,s.relation,r.project_id FROM research_run_sources s JOIN research_runs r ON r.workspace_id=s.workspace_id AND r.id=s.run_id WHERE s.workspace_id=?1 AND s.artifact_id=?2 AND s.relation LIKE 'workflow_%' ORDER BY s.relation LIMIT 4")
        .map_err(|_|"workflow_storage_error")?;
    let refs = query
        .query_map(params![root.workspace_id, definition], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "workflow_storage_error")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "workflow_storage_error")?;
    if refs.len() > 3 {
        return Err("workflow_child_budget".into());
    }
    let mut nodes = Vec::new();
    let mut roles = std::collections::HashSet::new();
    for (id, role, project) in refs {
        if !roles.insert(role.clone()) || input["project_id"] != project {
            return Err("workflow_child_ownership".into());
        }
        let child = repository::get(
            c,
            &root.workspace_id,
            Uuid::parse_str(&id).map_err(|_| "workflow_child_invalid")?,
        )
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_child_missing")?;
        let payload = request(&child)?;
        if child.kind != role_kind(&input, &role)?
            || payload["payload"]["dataset_id"] != input["dataset_id"]
            || (role.starts_with("workflow_train_")
                && payload["payload"]["data_version"] != input["dataset_sha256"])
        {
            return Err("workflow_child_ownership".into());
        }
        nodes.push(child);
    }
    Ok(nodes)
}

pub fn cancel_owned_nodes(c: &mut Connection, root: &TaskRecord) -> Result<(), String> {
    // Validate every node before any mutation; mutable checkpoints never authorize cancellation.
    let nodes = owned_nodes(c, root)?;
    for node in nodes {
        match node.state {
            TaskState::Running => {
                repository::request_running_cancellation(
                    c,
                    &root.workspace_id,
                    node.id,
                    node.attempt,
                )
                .map_err(|_| "workflow_cancel_conflict")?;
            }
            TaskState::Queued
            | TaskState::WaitingExternal
            | TaskState::Interrupted
            | TaskState::Paused => {
                repository::transition(
                    c,
                    &root.workspace_id,
                    node.id,
                    node.attempt,
                    node.state,
                    TaskState::Cancelled,
                    None,
                )
                .map_err(|_| "workflow_cancel_conflict")?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn complete_summary(c: &Connection, root: &TaskRecord, cp: &Value) -> Result<String, String> {
    let input = request(root)?;
    let (definition, _) = definition(c, root)?;
    let project = input["project_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    let run = artifacts::bind(c, &root.workspace_id, project, &root.id.to_string())?;
    let id = artifacts::create(
        c,
        &root.workspace_id,
        project,
        "analysis_result",
        "科研工作流结果（计划待确认）",
        &format!("workflow-result:{}", root.id),
        Some(&run),
        json!({"schemaVersion":"research-workflow-result.v1","definitionArtifactId":definition,
            "templateId":input["template"]["id"],"templateRevision":input["template"]["revision"],
            "templateContentSha256":input["template_content_sha256"],"dryRunSha256":input["dry_run_sha256"],
            "result":cp["result"],"status":"nodes_completed_plan_proposed","measurementsCreated":false}),
    )?;
    artifacts::edge(c, &root.workspace_id, &definition, &id, "generated_by_run")?;
    for model in cp["models"]
        .as_array()
        .ok_or("workflow_checkpoint_invalid")?
    {
        let source:String = c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='model_snapshot' AND source_key=?3",
            params![root.workspace_id,project,format!("registered:{}",model.as_str().ok_or("workflow_checkpoint_invalid")?)],|r|r.get(0)).map_err(|_|"workflow_model_artifact_missing")?;
        artifacts::edge(c, &root.workspace_id, &source, &id, "uses_model")?;
    }
    for (kind, key, relation) in [
        (
            "optimization_result",
            &cp["optimization"],
            "generated_by_run",
        ),
        ("experiment_plan", &cp["plan"], "recommends_experiment"),
    ] {
        let raw = key.as_str().ok_or("workflow_checkpoint_invalid")?;
        let source_key = if kind == "experiment_plan" {
            format!("plan:{raw}")
        } else {
            raw.to_owned()
        };
        let source:String = c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind=?3 AND source_key=?4",
            params![root.workspace_id,project,kind,source_key],|r|r.get(0)).map_err(|_|"workflow_result_artifact_missing")?;
        artifacts::edge(c, &root.workspace_id, &source, &id, relation)?;
    }
    Ok(id)
}
