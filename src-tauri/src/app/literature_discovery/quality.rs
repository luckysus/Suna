use super::*;
use crate::storage::repositories::literature_discovery::quality;

#[tauri::command]
pub fn preview_literature_evidence_quality(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        quality::preview(c, current_workspace_id(), &project_id)
    })
}
#[tauri::command]
pub fn archive_literature_evidence_quality(
    db: tauri::State<DbState>,
    project_id: String,
    expected_sha256: String,
) -> Result<Value, String> {
    with_conn_mut(&db, |c| {
        quality::archive(c, current_workspace_id(), &project_id, &expected_sha256)
    })
}
