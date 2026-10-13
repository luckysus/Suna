use crate::db::{app_data_directory, current_workspace_id, with_conn, DbState};
use crate::storage::repositories::industrial_sources as sources;
#[tauri::command]
pub fn list_material_compute_adapters() -> Vec<sources::Adapter> {
    sources::adapters()
}
#[tauri::command]
pub fn import_material_compute_file(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: sources::FileImport,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        sources::file_import(c, current_workspace_id(), &root, request)
    })
}
#[tauri::command]
pub fn save_database_read_profile(
    db: tauri::State<DbState>,
    request: sources::ReadProfile,
) -> Result<String, String> {
    with_conn(&db, |c| {
        sources::save_profile(c, current_workspace_id(), request)
    })
}
#[tauri::command]
pub fn list_database_read_profiles(db: tauri::State<DbState>) -> Result<serde_json::Value, String> {
    with_conn(&db, |c| sources::profiles(c, current_workspace_id()))
}
#[tauri::command]
pub fn submit_production_dataset_query(
    db: tauri::State<DbState>,
    project_id: String,
    profile_id: String,
    batch: String,
) -> Result<crate::app::task_commands::tasks::BackgroundTaskResponse, String> {
    with_conn(&db, |c| {
        sources::submit_read(c, current_workspace_id(), &project_id, &profile_id, &batch)
            .map(crate::app::task_commands::tasks::background_task_response)
    })
}
#[tauri::command]
pub fn import_production_query_snapshot(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    project_id: String,
    task_id: String,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        sources::import_query(c, current_workspace_id(), &project_id, &root, &task_id)
    })
}
#[tauri::command]
pub fn get_production_query_preview(
    db: tauri::State<DbState>,
    id: String,
) -> Result<Option<serde_json::Value>, String> {
    with_conn(&db, |c| {
        let uid = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
        let task = crate::tasks::repository::get(c, current_workspace_id(), uid)
            .map_err(|e| e.to_string())?
            .ok_or("task_not_found")?;
        if task.kind != crate::database::query_task::DATABASE_QUERY_KIND {
            return Err("not_production_query".into());
        }
        if task.state != crate::tasks::TaskState::Completed {
            return Ok(None);
        }
        let r = crate::storage::repositories::database_query_results::get(
            c,
            current_workspace_id(),
            uid,
        )?
        .ok_or("query_result_missing")?;
        Ok(Some(
            serde_json::json!({"taskId":id,"rowCount":r.row_count,"columns":r.columns,"previewRows":r.rows.into_iter().take(20).collect::<Vec<_>>(),"truncated":r.truncated,"queryTime":r.created_at}),
        ))
    })
}
