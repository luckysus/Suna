use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

/// 第 56 / 80 章：科研报告落库（0035 迁移建立 `reports` 表）。
///
/// 报告正文以 Markdown 保存；`kind` 是报告类型（研究报告 / 实验报告 / 文献综述 / 技术报告），
/// `format` 记录导出形态，`knowledge_base_ids_json` 保存生成时引用的知识库，
/// `metadata_json` 留给前端放图表与自定义字段。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReportRecord {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub topic: String,
    pub format: String,
    pub content_markdown: String,
    pub dataset_id: Option<String>,
    pub knowledge_base_ids_json: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct NewReport<'a> {
    pub kind: &'a str,
    pub title: &'a str,
    pub topic: &'a str,
    pub format: &'a str,
    pub content_markdown: &'a str,
    pub dataset_id: Option<&'a str>,
    pub knowledge_base_ids_json: &'a str,
    pub metadata_json: &'a str,
}

pub const REPORT_FORMATS: [&str; 4] = ["markdown", "docx", "pptx", "pdf"];
const MAX_CONTENT_CHARS: usize = 200_000;

const SELECT_REPORT: &str = "SELECT id, kind, title, topic, format, content_markdown, dataset_id,
    knowledge_base_ids_json, metadata_json, created_at, updated_at
    FROM reports WHERE workspace_id = ?1 AND id = ?2";

fn row_to_report(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportRecord> {
    Ok(ReportRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        title: row.get(2)?,
        topic: row.get(3)?,
        format: row.get(4)?,
        content_markdown: row.get(5)?,
        dataset_id: row.get(6)?,
        knowledge_base_ids_json: row.get(7)?,
        metadata_json: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn validate_json(value: &str, label: &str) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(value)
        .map_err(|error| format!("{label} must be valid JSON: {error}"))?;
    Ok(())
}

pub fn create(
    connection: &mut Connection,
    workspace_id: &str,
    report: NewReport<'_>,
) -> Result<ReportRecord, String> {
    super::research_artifacts::atomic(connection, || {
        create_in_transaction(connection, workspace_id, report)
    })
}

pub fn create_in_transaction(
    connection: &Connection,
    workspace_id: &str,
    report: NewReport<'_>,
) -> Result<ReportRecord, String> {
    let kind = report.kind.trim();
    if kind.is_empty() || kind.chars().count() > 40 {
        return Err("report kind must be 1-40 characters".to_string());
    }
    let title = report.title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err("report title must be 1-200 characters".to_string());
    }
    if report.topic.len() > 2000 {
        return Err("report topic is too long".to_string());
    }
    if !REPORT_FORMATS.contains(&report.format) {
        return Err(format!(
            "report format must be one of {}",
            REPORT_FORMATS.join(", ")
        ));
    }
    if report.content_markdown.chars().count() > MAX_CONTENT_CHARS {
        return Err("report content is too long".to_string());
    }
    validate_json(report.knowledge_base_ids_json, "knowledge_base_ids")?;
    validate_json(report.metadata_json, "metadata")?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO reports
               (workspace_id, id, kind, title, topic, format, content_markdown,
                dataset_id, knowledge_base_ids_json, metadata_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                workspace_id,
                id,
                kind,
                title,
                report.topic,
                report.format,
                report.content_markdown,
                report.dataset_id,
                report.knowledge_base_ids_json,
                report.metadata_json,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    get(connection, workspace_id, &id)?
        .ok_or_else(|| "created report could not be read back".to_string())
}

pub fn get(
    connection: &Connection,
    workspace_id: &str,
    id: &str,
) -> Result<Option<ReportRecord>, String> {
    connection
        .query_row(SELECT_REPORT, params![workspace_id, id], row_to_report)
        .optional()
        .map_err(|error| error.to_string())
}

pub fn list(connection: &Connection, workspace_id: &str) -> Result<Vec<ReportRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, kind, title, topic, format, content_markdown, dataset_id,
                    knowledge_base_ids_json, metadata_json, created_at, updated_at
             FROM reports WHERE workspace_id = ?1
             ORDER BY created_at DESC LIMIT 200",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], row_to_report)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn delete(connection: &mut Connection, workspace_id: &str, id: &str) -> Result<(), String> {
    let deleted = connection
        .execute(
            "DELETE FROM reports WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
        )
        .map_err(|error| error.to_string())?;
    if deleted == 0 {
        return Err("report was not found".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create, delete, get, list, NewReport};
    use crate::storage::migrations::migrate;
    use rusqlite::Connection;

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open database");
        migrate(&mut connection).expect("migrate database");
        connection
    }

    fn sample() -> NewReport<'static> {
        NewReport {
            kind: "研究报告",
            title: "Q690 高强钢工艺研究",
            topic: "回火温度对强韧性的影响",
            format: "markdown",
            content_markdown: "# 结论\n\n回火温度是首要敏感变量。",
            dataset_id: Some("dataset-1"),
            knowledge_base_ids_json: r#"["kb-1"]"#,
            metadata_json: r#"{"charts":2}"#,
        }
    }

    #[test]
    fn creates_lists_reads_and_deletes_reports() {
        let mut connection = database();
        let created = create(&mut connection, "ws", sample()).expect("create report");
        assert_eq!(created.kind, "研究报告");
        assert_eq!(created.format, "markdown");

        assert_eq!(list(&connection, "ws").expect("list").len(), 1);
        let fetched = get(&connection, "ws", &created.id)
            .expect("get")
            .expect("present");
        assert!(fetched.content_markdown.contains("回火温度"));

        delete(&mut connection, "ws", &created.id).expect("delete");
        assert!(list(&connection, "ws")
            .expect("list after delete")
            .is_empty());
        assert!(delete(&mut connection, "ws", &created.id).is_err());
    }

    #[test]
    fn rejects_invalid_payloads() {
        let mut connection = database();

        let mut bad_format = sample();
        bad_format.format = "exe";
        assert!(create(&mut connection, "ws", bad_format).is_err());

        let mut empty_title = sample();
        empty_title.title = "   ";
        assert!(create(&mut connection, "ws", empty_title).is_err());

        let mut bad_metadata = sample();
        bad_metadata.metadata_json = "{not json";
        assert!(create(&mut connection, "ws", bad_metadata).is_err());

        let mut bad_bases = sample();
        bad_bases.knowledge_base_ids_json = "nope";
        assert!(create(&mut connection, "ws", bad_bases).is_err());
    }
}
