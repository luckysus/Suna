//! Thin IPC boundary; long-running offline probes never hold the SQLite mutex.
use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::research_workflow::{
    entry::{Selection, StartRequest},
    schema::Template,
    status, templates, worker_validation,
};
use serde_json::Value;
use uuid::Uuid;

#[tauri::command]
pub fn list_research_workflow_templates(
    db: tauri::State<DbState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Value>, String> {
    with_conn(&db, |c| {
        templates::list(
            c,
            current_workspace_id(),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_workflow_template(
    db: tauri::State<DbState>,
    id: String,
    revision: u64,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        templates::get(c, current_workspace_id(), &id, revision)
    })
}
#[tauri::command]
pub fn save_research_workflow_template(
    db: tauri::State<DbState>,
    template: Template,
    expected_revision: u64,
) -> Result<Value, String> {
    with_conn_mut(&db, |c| {
        templates::save(c, current_workspace_id(), &template, expected_revision)
    })
}
#[tauri::command]
pub async fn preview_research_workflow(
    app: tauri::AppHandle,
    db: tauri::State<'_, DbState>,
    request: Selection,
) -> Result<Value, String> {
    let template = with_conn(&db, |c| request.template(c, current_workspace_id()))?;
    let config = crate::db::compute_worker_config(&app);
    let worker =
        tauri::async_runtime::spawn_blocking(move || worker_validation::probe(&template, config))
            .await
            .map_err(|_| "workflow_worker_probe_failed")??;
    with_conn(&db, |c| {
        crate::research_workflow::entry::preview(c, current_workspace_id(), &request, &worker)
    })
}
#[tauri::command]
pub async fn start_research_workflow(
    app: tauri::AppHandle,
    db: tauri::State<'_, DbState>,
    request: StartRequest,
) -> Result<Value, String> {
    if !request.confirmed {
        return Err("workflow_start_confirmation_required".into());
    }
    let template = with_conn(&db, |c| {
        request.selection().template(c, current_workspace_id())
    })?;
    let config = crate::db::compute_worker_config(&app);
    let worker =
        tauri::async_runtime::spawn_blocking(move || worker_validation::probe(&template, config))
            .await
            .map_err(|_| "workflow_worker_probe_failed")??;
    with_conn_mut(&db, |c| {
        crate::research_workflow::entry::start(c, current_workspace_id(), &request, &worker)
    })
}
#[tauri::command]
pub fn list_research_workflows(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Value>, String> {
    with_conn(&db, |c| {
        status::list(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(25),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_workflow(
    db: tauri::State<DbState>,
    project_id: String,
    id: Uuid,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        status::get(c, current_workspace_id(), &project_id, id)
    })
}
