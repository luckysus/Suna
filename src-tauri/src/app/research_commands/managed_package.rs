//! Native paths stay in the host. Assets are restored only into a separate isolation.
use crate::db::{app_data_directory, current_workspace_id, with_conn, DbState};
use crate::research_package::{managed, selected_file};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri_plugin_dialog::DialogExt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    project_id: String,
    asset_ids: Vec<String>,
    confirmed_asset_rights: bool,
}

#[tauri::command]
pub fn list_research_managed_assets(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<Value, String> {
    let root = app_data_directory(&app)?;
    with_conn(&db, |c| {
        managed::catalog::inventory(c, current_workspace_id(), &root, &project_id)
    })
}
fn capture(
    app: &tauri::AppHandle,
    db: &tauri::State<'_, DbState>,
    request: &Selection,
) -> Result<managed::catalog::Capture, String> {
    let root = app_data_directory(app)?;
    with_conn(db, |c| {
        managed::catalog::capture(
            c,
            current_workspace_id(),
            &root,
            &request.project_id,
            &request.asset_ids,
            request.confirmed_asset_rights,
        )
    })
}
#[tauri::command]
pub async fn preview_research_managed_package(
    app: tauri::AppHandle,
    db: tauri::State<'_, DbState>,
    request: Selection,
) -> Result<Value, String> {
    let captured = capture(&app, &db, &request)?;
    tauri::async_runtime::spawn_blocking(move || Ok(captured.prepare()?.summary().clone()))
        .await
        .map_err(|_| "package_preview_failed")?
}
#[tauri::command]
pub async fn export_research_managed_package(
    app: tauri::AppHandle,
    db: tauri::State<'_, DbState>,
    request: Selection,
    expected_sha256: String,
) -> Result<Option<Value>, String> {
    let captured = capture(&app, &db, &request)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Value>, String> {
        let prepared = captured.prepare()?.confirm(&expected_sha256)?;
        let mut result = prepared.summary().clone();
        let bytes = prepared.into_bytes();
        let Some(saved) =
            crate::app::export_commands::save_selected_file(&app, "科研项目受管资产包.zip", &bytes)
                .map_err(|_| "package_export_write_failed")?
        else {
            return Ok(None);
        };
        if saved.bytes != bytes.len() {
            return Err("package_export_write_failed".into());
        }
        result["writeVerified"] = json!(true);
        Ok(Some(result))
    })
    .await
    .map_err(|_| "package_export_failed")?
}
fn selected(app: &tauri::AppHandle) -> Result<Option<managed::Verified>, String> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Suna 受管科研资产包", &["zip"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file
        .into_path()
        .map_err(|_| "package_selected_path_not_authorized")?;
    let bytes = selected_file::read_bytes(&path, managed::MAX_TOTAL)?;
    managed::archive::inspect(&bytes).map(Some)
}
#[tauri::command]
pub async fn inspect_research_managed_package_file(
    app: tauri::AppHandle,
) -> Result<Option<Value>, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Value>, String> {
        let Some(package) = selected(&app)? else {
            return Ok(None);
        };
        Ok(Some(package.summary()))
    })
    .await
    .map_err(|_| "package_inspection_failed")?
}
#[tauri::command]
pub async fn import_research_managed_package_isolated(
    app: tauri::AppHandle,
    expected_sha256: String,
    confirmed: bool,
) -> Result<Option<Value>, String> {
    if !confirmed {
        return Err("package_isolation_confirmation_required".into());
    }
    let root = app_data_directory(&app)?;
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<Value>, String> {
        let Some(package) = selected(&app)? else {
            return Ok(None);
        };
        managed::staging::import(&package, &root, &expected_sha256).map(Some)
    })
    .await
    .map_err(|_| "package_isolation_import_failed")?
}
