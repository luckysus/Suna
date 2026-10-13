use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::research_projects as projects;
use crate::storage::repositories::research_projects::{ResearchProject, SaveResearchProject};
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
