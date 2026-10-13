use crate::db::{current_workspace_id, with_conn, DbState};
use crate::storage::repositories::research_artifacts as artifacts;

#[tauri::command]
pub fn list_research_runs(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Run>, String> {
    with_conn(&db, |c| {
        artifacts::reconcile_project(c, current_workspace_id(), &project_id)?;
        artifacts::runs(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn list_research_artifacts(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Artifact>, String> {
    with_conn(&db, |c| {
        artifacts::list(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_artifact(
    db: tauri::State<DbState>,
    id: String,
) -> Result<artifacts::Artifact, String> {
    with_conn(&db, |c| artifacts::get(c, current_workspace_id(), &id))
}
#[tauri::command]
pub fn list_research_artifact_edges(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Edge>, String> {
    with_conn(&db, |c| {
        artifacts::edges(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
