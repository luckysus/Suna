//! Read-only projection: Inspector receives IDs and states, never task payloads.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use suna::storage::repositories::{research_artifacts as artifacts, research_projects as projects};
use suna::tasks::{repository, TaskRecord, TaskState};
use uuid::Uuid;

fn task_status(task: &TaskRecord) -> Value {
    json!({"id":task.id,"kind":task.kind,"state":task.state,"progress":task.progress,
        "attempt":task.attempt,"error_code":task.error_code,"cancel_requested":task.cancel_requested,
        "can_cancel":!task.cancel_requested&&matches!(task.state,TaskState::Queued|TaskState::Running|TaskState::WaitingExternal|TaskState::Paused|TaskState::Interrupted),
        "can_retry":matches!(task.state,TaskState::Failed|TaskState::Cancelled|TaskState::Paused|TaskState::Interrupted),
        "created_at":task.created_at,"updated_at":task.updated_at,"started_at":task.started_at,"finished_at":task.finished_at})
}

pub fn get(c: &Connection, ws: &str, project: &str, id: Uuid) -> Result<Value, String> {
    projects::get(c, ws, project)?;
    let root = repository::get(c, ws, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    let input: Value =
        serde_json::from_str(&root.payload_json).map_err(|_| "workflow_payload_invalid")?;
    if root.kind != crate::runtime::KIND || input["project_id"] != project {
        return Err("workflow_project_mismatch".into());
    }
    let (definition, deadline) = crate::provenance::definition(c, &root)?;
    let definition_hash = artifacts::get(c, ws, &definition)?.content_sha256;
    let nodes = crate::provenance::owned_nodes(c, &root)?
        .into_iter()
        .map(|node| task_status(&node))
        .collect::<Vec<_>>();
    let result:Option<String>=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='analysis_result' AND source_key=?3",
        params![ws,project,format!("workflow-result:{id}")],|r|r.get(0)).optional().map_err(|_|"workflow_storage_error")?;
    let result = if let Some(id) = result {
        let artifact = artifacts::get(c, ws, &id)?;
        let payload = artifact.payload.ok_or("workflow_result_invalid")?;
        if payload["schemaVersion"] != "research-workflow-result.v1"
            || payload["definitionArtifactId"] != definition
        {
            return Err("workflow_result_invalid".into());
        }
        let mut result = serde_json::Map::new();
        for key in ["optimizationTaskId", "proposedPlanId"] {
            let value = payload["result"][key]
                .as_str()
                .filter(|v| Uuid::parse_str(v).is_ok())
                .ok_or("workflow_result_invalid")?;
            result.insert(key.into(), json!(value));
        }
        for key in ["trainingTaskIds", "registeredModelIds"] {
            let ids = payload["result"][key]
                .as_array()
                .filter(|ids| ids.len() == 2)
                .ok_or("workflow_result_invalid")?;
            if ids
                .iter()
                .any(|id| id.as_str().is_none_or(|id| Uuid::parse_str(id).is_err()))
            {
                return Err("workflow_result_invalid".into());
            }
            result.insert(key.into(), json!(ids));
        }
        result.insert("artifactId".into(), json!(artifact.id));
        result.insert("contentSha256".into(), json!(artifact.content_sha256));
        result.insert("confirmationRequiredToAccept".into(), json!(true));
        result.insert("measurementsCreated".into(), json!(false));
        Value::Object(result)
    } else {
        Value::Null
    };
    Ok(
        json!({"projectId":project,"task":task_status(&root),"nodes":nodes,
        "definitionArtifactId":definition,"definitionContentSha256":definition_hash,
        "templateId":input["template"]["id"],"templateRevision":input["template"]["revision"],
        "templateContentSha256":input["template_content_sha256"],"approvedDeadline":deadline.to_rfc3339(),"result":result}),
    )
}

pub fn list(
    c: &Connection,
    ws: &str,
    project: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Value>, String> {
    projects::get(c, ws, project)?;
    let mut q=c.prepare("SELECT t.id FROM background_tasks t JOIN research_runs r ON r.workspace_id=t.workspace_id AND r.background_task_id=t.id WHERE t.workspace_id=?1 AND r.project_id=?2 AND t.kind=?3 ORDER BY t.created_at DESC,t.id LIMIT ?4 OFFSET ?5")
        .map_err(|_|"workflow_storage_error")?;
    let ids = q
        .query_map(
            params![
                ws,
                project,
                crate::runtime::KIND,
                limit.clamp(1, 50),
                offset.min(10000)
            ],
            |r| r.get::<_, String>(0),
        )
        .map_err(|_| "workflow_storage_error")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "workflow_storage_error")?;
    ids.into_iter()
        .map(|id| {
            get(
                c,
                ws,
                project,
                Uuid::parse_str(&id).map_err(|_| "workflow_task_invalid")?,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_tests::{setup, submit};
    #[test]
    fn inspector_projection_reopens_and_lists_runs_without_private_payload_or_checkpoint() {
        let (mut f, path) = setup();
        let root = submit(&mut f, Uuid::new_v4());
        f.connection.execute("UPDATE background_tasks SET checkpoint_json=json_set(checkpoint_json,'$.secret','synthetic-private-sentinel') WHERE id=?1",[root.id.to_string()]).unwrap();
        let before: i64 = f
            .connection
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap();
        let status = get(&f.connection, "local", &f.input.project_id, root.id).unwrap();
        assert_eq!(status["task"]["state"], "queued");
        assert_eq!(status["task"]["can_cancel"], true);
        assert_eq!(status["nodes"], json!([]));
        for forbidden in [
            "synthetic-private-sentinel",
            "payload_json",
            "checkpoint_json",
            "splitPolicy",
            "featureColumns",
            f.root.to_string_lossy().as_ref(),
        ] {
            assert!(!status.to_string().contains(forbidden));
        }
        assert_eq!(
            list(&f.connection, "local", &f.input.project_id, 25, 0).unwrap(),
            vec![status.clone()]
        );
        assert!(list(&f.connection, "local", &f.input.project_id, 25, 25)
            .unwrap()
            .is_empty());
        assert_eq!(
            f.connection
                .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            before
        );
        let reopened = Connection::open(path).unwrap();
        assert_eq!(
            get(&reopened, "local", &f.input.project_id, root.id).unwrap(),
            status
        );
        drop(reopened);
    }
    #[test]
    fn status_cannot_read_or_cancel_another_workspace_or_project() {
        let (mut f, _) = setup();
        let root = submit(&mut f, Uuid::new_v4());
        assert!(get(
            &f.connection,
            "other-workspace",
            &f.input.project_id,
            root.id
        )
        .is_err());
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
        assert!(get(&f.connection, "local", &other.id, root.id).is_err());
        assert!(list(&f.connection, "local", &other.id, 25, 0)
            .unwrap()
            .is_empty());
        assert!(crate::runtime::request_cancellation(
            &mut f.connection,
            "other-workspace",
            root.id
        )
        .is_err());
        assert_eq!(
            repository::get(&f.connection, "local", root.id)
                .unwrap()
                .unwrap()
                .state,
            TaskState::Queued
        );
    }
}
