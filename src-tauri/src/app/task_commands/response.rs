use crate::tasks::{TaskRecord, TaskState};
use serde::Serialize;
use uuid::Uuid;
#[derive(Debug, Serialize)]
pub struct BackgroundTaskResponse {
    pub id: Uuid,
    pub kind: String,
    pub dataset_id: Option<String>,
    pub state: TaskState,
    pub progress: u8,
    pub attempt: u32,
    pub error_code: Option<String>,
    pub cancel_requested: bool,
    pub can_cancel: bool,
    pub can_retry: bool,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub file_name: Option<String>,
}
pub(crate) fn background_task_response(task: TaskRecord) -> BackgroundTaskResponse {
    let dataset_id = crate::compute::handler::task_dataset_id(&task);
    let file_name = crate::tasks::mineru_file_name(&task);
    let can_cancel = !task.cancel_requested
        && matches!(
            task.state,
            TaskState::Queued
                | TaskState::Running
                | TaskState::WaitingExternal
                | TaskState::Paused
                | TaskState::Interrupted
        );
    let can_retry = matches!(
        task.state,
        TaskState::Failed | TaskState::Cancelled | TaskState::Interrupted | TaskState::Paused
    );
    BackgroundTaskResponse {
        id: task.id,
        kind: task.kind,
        dataset_id,
        state: task.state,
        progress: task.progress,
        attempt: task.attempt,
        error_code: task.error_code,
        cancel_requested: task.cancel_requested,
        can_cancel,
        can_retry,
        created_at: task.created_at,
        updated_at: task.updated_at,
        started_at: task.started_at,
        finished_at: task.finished_at,
        file_name,
    }
}
