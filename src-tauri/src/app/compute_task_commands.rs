use crate::db::{current_workspace_id, with_conn, DbState};
use crate::storage::repositories::compute_tasks::{self, ComputeTaskRecord};

/// 第 80 章：`prediction_tasks` / `optimization_tasks` 的查询入口。
///
/// 提交预测 / 优化任务时会写入对应记录，取回结果时同步状态；
/// 这里把记录暴露给「运行记录」类界面。
#[tauri::command]
pub fn list_compute_tasks(
    db: tauri::State<DbState>,
    kind: String,
) -> Result<Vec<ComputeTaskRecord>, String> {
    with_conn(&db, |connection| {
        compute_tasks::list(connection, current_workspace_id(), &kind)
    })
}
