use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::{research_projects as projects, settings};
use projects::{ResearchObject, ResearchProject, ResearchSummary, SaveResearchProject};
use serde::Deserialize;

#[tauri::command]
pub fn save_research_project(
    db: tauri::State<DbState>,
    request: SaveResearchProject,
) -> Result<ResearchProject, String> {
    with_conn_mut(&db, |c| projects::save(c, current_workspace_id(), request))
}
#[tauri::command]
pub fn get_research_project(
    db: tauri::State<DbState>,
    id: String,
) -> Result<ResearchProject, String> {
    with_conn(&db, |c| projects::get(c, current_workspace_id(), &id))
}
#[tauri::command]
pub fn list_research_projects(
    db: tauri::State<DbState>,
    search: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchProject>, String> {
    with_conn(&db, |c| {
        projects::list(
            c,
            current_workspace_id(),
            search.as_deref().unwrap_or(""),
            status.as_deref(),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn archive_research_project(
    db: tauri::State<DbState>,
    id: String,
    archived: bool,
) -> Result<ResearchProject, String> {
    with_conn_mut(&db, |c| {
        projects::archive(c, current_workspace_id(), &id, archived)
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchObjectRequest {
    pub project_id: String,
    pub kind: String,
    pub id: String,
    #[serde(default = "attached")]
    pub relation: String,
}
fn attached() -> String {
    "attached".into()
}
#[tauri::command]
pub fn attach_research_object(
    db: tauri::State<DbState>,
    request: ResearchObjectRequest,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        projects::attach(
            c,
            current_workspace_id(),
            &request.project_id,
            &request.kind,
            &request.id,
            &request.relation,
        )
    })
}
#[tauri::command]
pub fn detach_research_object(
    db: tauri::State<DbState>,
    request: ResearchObjectRequest,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        projects::detach(
            c,
            current_workspace_id(),
            &request.project_id,
            &request.kind,
            &request.id,
            &request.relation,
        )
    })
}
#[tauri::command]
pub fn list_research_project_objects(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchObject>, String> {
    with_conn(&db, |c| {
        projects::objects(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn list_research_attachable_objects(
    db: tauri::State<DbState>,
    kind: String,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchObject>, String> {
    with_conn(&db, |c| {
        projects::available(
            c,
            current_workspace_id(),
            &kind,
            search.as_deref().unwrap_or(""),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_project_summary(
    db: tauri::State<DbState>,
    id: String,
) -> Result<ResearchSummary, String> {
    with_conn(&db, |c| projects::summary(c, current_workspace_id(), &id))
}
const CURRENT_PROJECT: &str = "suna.research.current_project";
#[tauri::command]
pub fn get_current_research_project(
    db: tauri::State<DbState>,
) -> Result<Option<ResearchProject>, String> {
    with_conn(&db, |c| {
        let saved = settings::get(c, current_workspace_id(), CURRENT_PROJECT)?;
        let id: Option<String> = saved.and_then(|s| serde_json::from_str(&s).ok());
        id.map(|id| projects::get(c, current_workspace_id(), &id))
            .transpose()
    })
}
#[tauri::command]
pub fn set_current_research_project(
    db: tauri::State<DbState>,
    id: Option<String>,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        if let Some(id) = &id {
            projects::require_active(c, current_workspace_id(), id)?;
        }
        settings::set(
            c,
            current_workspace_id(),
            CURRENT_PROJECT,
            &serde_json::to_string(&id).map_err(|e| e.to_string())?,
        )
    })
}

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
