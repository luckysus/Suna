use crate::db::{app_data_directory, current_workspace_id, with_conn, DbState};
use crate::storage::repositories::microstructure as images;
#[tauri::command]
pub fn import_microstructure_image(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: images::ImportImage,
) -> Result<images::ImageRecord, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        images::import(c, current_workspace_id(), &root, request)
    })
}
#[tauri::command]
pub fn list_microstructure_images(
    db: tauri::State<DbState>,
    project_id: String,
    offset: Option<u32>,
) -> Result<Vec<images::ImageRecord>, String> {
    with_conn(&db, |c| {
        images::list(c, current_workspace_id(), &project_id, offset.unwrap_or(0))
    })
}
#[tauri::command]
pub fn get_microstructure_preview(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    id: String,
) -> Result<Vec<u8>, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        images::preview(c, current_workspace_id(), &root, &id)
    })
}
#[tauri::command]
pub fn save_microstructure_annotation(
    db: tauri::State<DbState>,
    image_id: String,
    request: images::Annotation,
) -> Result<images::AnnotationRecord, String> {
    with_conn(&db, |c| {
        images::annotate(c, current_workspace_id(), &image_id, request)
    })
}
#[tauri::command]
pub fn get_microstructure_annotation(
    db: tauri::State<DbState>,
    image_id: String,
) -> Result<Option<images::AnnotationRecord>, String> {
    with_conn(&db, |c| {
        images::latest_annotation(c, current_workspace_id(), &image_id)
    })
}
#[tauri::command]
pub fn analyze_microstructure_images(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    project_id: String,
    annotation_ids: Vec<String>,
) -> Result<crate::app::task_commands::tasks::BackgroundTaskResponse, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        images::submit(
            c,
            current_workspace_id(),
            &root,
            &project_id,
            annotation_ids,
        )
        .map(crate::app::task_commands::tasks::background_task_response)
    })
}
#[tauri::command]
pub fn list_microstructure_analyses(
    db: tauri::State<DbState>,
    image_id: String,
) -> Result<serde_json::Value, String> {
    with_conn(&db, |c| {
        images::analyses(c, current_workspace_id(), &image_id, false)
    })
}
#[tauri::command]
pub fn review_microstructure_analysis(
    db: tauri::State<DbState>,
    id: String,
    state: String,
    note: String,
) -> Result<(), String> {
    with_conn(&db, |c| {
        images::review(c, current_workspace_id(), &id, &state, &note)
    })
}
#[tauri::command]
pub fn get_microstructure_task_result(
    db: tauri::State<DbState>,
    id: String,
) -> Result<Option<serde_json::Value>, String> {
    with_conn(&db, |c| {
        let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
        let task = crate::tasks::repository::get(c, current_workspace_id(), id)
            .map_err(|e| e.to_string())?
            .ok_or("task_not_found")?;
        if task.kind != "compute_analyze_microstructure" {
            return Err("not_image_analysis_task".into());
        }
        if task.state != crate::tasks::TaskState::Completed {
            return Ok(None);
        }
        let raw: serde_json::Value =
            serde_json::from_str(task.checkpoint_json.as_deref().ok_or("missing_result")?)
                .map_err(|e| e.to_string())?;
        Ok(Some(raw["result"].clone()))
    })
}
