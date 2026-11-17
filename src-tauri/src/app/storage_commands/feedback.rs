use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::agent_feedback::{
    self, AgentFeedbackInput, AgentFeedbackRecord, AgentFeedbackSaveResult,
};

#[tauri::command]
pub fn save_agent_feedback(
    db: tauri::State<DbState>,
    input: AgentFeedbackInput,
) -> Result<AgentFeedbackSaveResult, String> {
    with_conn_mut(&db, |conn| {
        agent_feedback::upsert(conn, current_workspace_id(), input)
    })
}

#[tauri::command]
pub fn get_agent_feedback(
    db: tauri::State<DbState>,
    conversation_id: String,
    message_id: String,
) -> Result<Option<AgentFeedbackRecord>, String> {
    with_conn(&db, |conn| {
        agent_feedback::get(conn, current_workspace_id(), &conversation_id, &message_id)
    })
}
