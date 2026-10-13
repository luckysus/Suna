use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::research_projects as projects;
use crate::storage::repositories::research_projects::{
    ResearchObject, ResearchProject, ResearchSummary,
};
use crate::storage::repositories::settings;

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
