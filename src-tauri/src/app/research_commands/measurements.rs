use crate::db::{current_workspace_id, with_conn, DbState};
use crate::storage::repositories::experiment_measurements as measurements;
use crate::storage::repositories::research_artifacts as artifacts;
use crate::storage::repositories::research_projects as projects;

#[tauri::command]
pub fn record_experiment_execution(
    db: tauri::State<DbState>,
    request: measurements::RecordExecution,
) -> Result<measurements::Execution, String> {
    with_conn(&db, |c| {
        measurements::record(c, current_workspace_id(), request)
    })
}
#[tauri::command]
pub fn list_experiment_executions(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<measurements::Execution>, String> {
    with_conn(&db, |c| {
        measurements::list(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(25),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn review_experiment_execution(
    db: tauri::State<DbState>,
    id: String,
    state: String,
    reason: String,
    expected_state: String,
) -> Result<measurements::Execution, String> {
    with_conn(&db, |c| {
        measurements::review(
            c,
            current_workspace_id(),
            &id,
            &state,
            &reason,
            &expected_state,
        )
    })
}
#[tauri::command]
pub fn create_research_candidate_plan(
    db: tauri::State<DbState>,
    project_id: String,
    artifact_id: String,
    index: usize,
    title: String,
) -> Result<crate::storage::repositories::experiments::ExperimentRecord, String> {
    with_conn(&db, |c| {
        measurements::candidate_plan(
            c,
            current_workspace_id(),
            &project_id,
            &artifact_id,
            index,
            &title,
        )
    })
}
#[tauri::command]
pub fn compare_experiment_predictions(
    db: tauri::State<DbState>,
    id: String,
    artifact_id: String,
    candidate: usize,
) -> Result<Vec<measurements::Comparison>, String> {
    with_conn(&db, |c| {
        measurements::compare(c, current_workspace_id(), &id, &artifact_id, candidate)
    })
}
#[tauri::command]
pub fn create_measurement_dataset_version(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: measurements::DatasetVersionRequest,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = crate::db::app_data_directory(&app)?;
    with_conn(&db, |c| {
        measurements::create_dataset_version(c, current_workspace_id(), &root, request)
    })
}

#[tauri::command]
pub fn find_research_task_artifact(
    db: tauri::State<DbState>,
    project_id: String,
    task_id: String,
) -> Result<Option<String>, String> {
    use rusqlite::OptionalExtension;
    with_conn(&db, |c| {
        projects::get(c, current_workspace_id(), &project_id)?;
        artifacts::reconcile_task(c, current_workspace_id(), &task_id)?;
        c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND source_key=?3",rusqlite::params![current_workspace_id(),project_id,task_id],|r|r.get(0)).optional().map_err(|e|e.to_string())
    })
}
