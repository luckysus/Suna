//! Read-only package library; client supplies an ID and bounded offset, never a path.
use crate::db::app_data_directory;
use crate::research_package::managed::library;
use serde_json::Value;

#[tauri::command]
pub async fn list_research_package_isolations(
    app: tauri::AppHandle,
    offset: u32,
) -> Result<Value, String> {
    let root = app_data_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || library::list(&root, offset))
        .await
        .map_err(|_| "package_isolation_read_failed")?
}

#[tauri::command]
pub async fn open_research_package_isolation(
    app: tauri::AppHandle,
    isolation_id: String,
    offset: u32,
) -> Result<Value, String> {
    let root = app_data_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || library::open(&root, &isolation_id, offset))
        .await
        .map_err(|_| "package_isolation_read_failed")?
}
