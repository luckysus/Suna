//! Native pickers own paths. ZIP work and dialogs never hold the SQLite mutex.
use crate::db::{current_workspace_id, with_conn, DbState};
use crate::research_package::{delivery, projection, selected_file, temporary, valid_sha};
use serde_json::{json, Value};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn preview_research_package(
    db: tauri::State<'_, DbState>,
    project_id: String,
) -> Result<Value, String> {
    let project = with_conn(&db, |c| {
        projection::from_connection(c, current_workspace_id(), &project_id)
    })?;
    tauri::async_runtime::spawn_blocking(move || -> Result<Value, String> {
        Ok(delivery::prepare(project)?.summary().clone())
    })
    .await
    .map_err(|_| "package_preview_failed")?
}

#[tauri::command]
pub async fn export_research_package(
    app: tauri::AppHandle,
    db: tauri::State<'_, DbState>,
    project_id: String,
    expected_sha256: String,
) -> Result<Option<Value>, String> {
    if !valid_sha(&expected_sha256) {
        return Err("package_preview_changed".into());
    }
    let project = with_conn(&db, |c| {
        projection::from_connection(c, current_workspace_id(), &project_id)
    })?;
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Value>, String> {
        let prepared = delivery::prepare(project)?.confirm(&expected_sha256)?;
        let mut receipt = prepared.summary().clone();
        let bytes = prepared.into_bytes();
        let Some(saved) =
            crate::app::export_commands::save_selected_file(&app, "科研项目元数据包.zip", &bytes)
                .map_err(|_| "package_export_write_failed")?
        else {
            return Ok(None);
        };
        if saved.bytes != bytes.len() {
            return Err("package_export_write_failed".into());
        }
        // The shared writer checks both destination bytes and regular-file safety.
        // The user's full destination path is omitted from the package receipt.
        receipt["writeVerified"] = json!(true);
        Ok(Some(receipt))
    })
    .await
    .map_err(|_| "package_export_failed")?
}

#[tauri::command]
pub async fn inspect_research_package_file(app: tauri::AppHandle) -> Result<Option<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Value>, String> {
        let Some(selected) = app
            .dialog()
            .file()
            .add_filter("Suna 科研项目元数据包", &["zip"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = selected
            .into_path()
            .map_err(|_| "package_selected_path_not_authorized")?;
        let selected = selected_file::read(&path)?;
        let mut result = selected.package.summary();
        result["packageSha256"] = json!(selected.bytes_sha256);
        result["bytes"] = json!(selected.bytes);
        result["dryRun"] = temporary::simulate_import(&selected.package)?;
        Ok(Some(result))
    })
    .await
    .map_err(|_| "package_inspection_failed")?
}
