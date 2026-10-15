use super::*;
use crate::storage::repositories::literature_discovery::experimental_context as context;

#[tauri::command]
pub fn list_literature_context_sources(
    db: tauri::State<DbState>,
    project_id: String,
    offset: Option<u32>,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        context::sources(c, current_workspace_id(), &project_id, offset.unwrap_or(0))
    })
}
#[tauri::command]
pub fn archive_literature_experimental_context(
    db: tauri::State<DbState>,
    project_id: String,
    request: context::SaveContext,
) -> Result<Value, String> {
    with_conn_mut(&db, |c| {
        context::save(c, current_workspace_id(), &project_id, request)
    })
}
#[tauri::command]
pub fn list_literature_experimental_contexts(
    db: tauri::State<DbState>,
    project_id: String,
    evidence_artifact_id: String,
    offset: Option<u32>,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        context::list(
            c,
            current_workspace_id(),
            &project_id,
            &evidence_artifact_id,
            offset.unwrap_or(0),
        )
    })
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
