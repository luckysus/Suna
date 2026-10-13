use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};

#[tauri::command]
pub fn export_research_manifest(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<serde_json::Value, String> {
    with_conn(&db, |c| {
        crate::storage::repositories::research_delivery::manifest(
            c,
            current_workspace_id(),
            &project_id,
        )
    })
}
#[tauri::command]
pub fn generate_research_project_report(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<crate::storage::repositories::reports::ReportRecord, String> {
    with_conn_mut(&db, |c| {
        crate::storage::repositories::research_delivery::report(
            c,
            current_workspace_id(),
            &project_id,
        )
    })
}
#[tauri::command]
pub fn get_research_run_navigation(
    db: tauri::State<DbState>,
    task_id: String,
) -> Result<serde_json::Value, String> {
    use rusqlite::OptionalExtension;
    with_conn(&db, |c| {
        let result=c.query_row("SELECT r.project_id,s.run_id,a.conversation_id,r.kind FROM research_runs r LEFT JOIN agent_task_sources s ON s.workspace_id=r.workspace_id AND s.task_id=r.background_task_id LEFT JOIN agent_runs a ON a.workspace_id=s.workspace_id AND a.id=s.run_id WHERE r.workspace_id=?1 AND r.background_task_id=?2 ORDER BY r.created_at LIMIT 1",rusqlite::params![current_workspace_id(),task_id],|r|Ok(serde_json::json!({"projectId":r.get::<_,String>(0)?,"agentRunId":r.get::<_,Option<String>>(1)?,"conversationId":r.get::<_,Option<String>>(2)?,"kind":r.get::<_,String>(3)?}))).optional().map_err(|e|e.to_string())?;
        result.ok_or("research_run_not_found".into())
    })
}
