use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::reports::{self, NewReport, ReportRecord};
use serde::Deserialize;

/// 第 56 / 80 章：科研报告的保存、列表与删除。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReportRequest {
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub topic: String,
    pub format: String,
    pub content_markdown: String,
    #[serde(default)]
    pub dataset_id: Option<String>,
    /// 生成时引用的知识库 id 列表。
    #[serde(default)]
    pub knowledge_base_ids: Vec<String>,
    /// 附加信息（图表数量、来源等）。
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[tauri::command]
pub fn save_report(
    db: tauri::State<DbState>,
    request: SaveReportRequest,
) -> Result<ReportRecord, String> {
    let knowledge_base_ids_json = serde_json::to_string(&request.knowledge_base_ids)
        .map_err(|error| format!("knowledge_base_ids must be serializable: {error}"))?;
    let metadata_json = serde_json::to_string(
        &request
            .metadata
            .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
    )
    .map_err(|error| format!("metadata must be serializable: {error}"))?;
    with_conn_mut(&db, |connection| {
        reports::create(
            connection,
            current_workspace_id(),
            NewReport {
                kind: &request.kind,
                title: &request.title,
                topic: &request.topic,
                format: &request.format,
                content_markdown: &request.content_markdown,
                dataset_id: request.dataset_id.as_deref(),
                knowledge_base_ids_json: &knowledge_base_ids_json,
                metadata_json: &metadata_json,
            },
        )
    })
}

#[tauri::command]
pub fn list_reports(db: tauri::State<DbState>) -> Result<Vec<ReportRecord>, String> {
    with_conn(&db, |connection| {
        reports::list(connection, current_workspace_id())
    })
}

#[tauri::command]
pub fn delete_report(db: tauri::State<DbState>, id: String) -> Result<(), String> {
    with_conn_mut(&db, |connection| {
        reports::delete(connection, current_workspace_id(), &id)
    })
}
