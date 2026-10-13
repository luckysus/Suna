use crate::db::{current_workspace_id, with_conn, DbState};

#[tauri::command]
pub fn recommend_next_research_experiments(
    db: tauri::State<DbState>,
    request: crate::storage::repositories::research_feedback::RecommendRequest,
) -> Result<crate::app::task_commands::tasks::BackgroundTaskResponse, String> {
    with_conn(&db, |c| {
        crate::storage::repositories::research_feedback::submit(c, current_workspace_id(), request)
            .map(crate::app::task_commands::tasks::background_task_response)
    })
}
#[tauri::command]
pub fn get_experiment_replicate_statistics(
    db: tauri::State<DbState>,
    project_id: String,
    plan_id: String,
) -> Result<Vec<crate::storage::repositories::research_feedback::Statistics>, String> {
    with_conn(&db, |c| {
        crate::storage::repositories::research_feedback::statistics(
            c,
            current_workspace_id(),
            &project_id,
            &plan_id,
        )
    })
}

#[tauri::command]
pub fn create_research_design_candidate_plan(
    db: tauri::State<DbState>,
    project_id: String,
    task_id: String,
    index: usize,
    title: String,
) -> Result<crate::storage::repositories::experiments::ExperimentRecord, String> {
    with_conn(&db, |c| {
        crate::storage::repositories::research_feedback::design_plan(
            c,
            current_workspace_id(),
            &project_id,
            &task_id,
            index,
            &title,
        )
    })
}
