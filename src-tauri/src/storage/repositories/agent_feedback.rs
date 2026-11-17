use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFeedbackInput {
    pub conversation_id: String,
    pub message_id: String,
    pub rating: String,
    pub reason: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFeedbackRecord {
    pub id: i64,
    pub conversation_id: String,
    pub message_id: String,
    pub rating: String,
    pub reason: Option<String>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentFeedbackSaveResult {
    pub ok: bool,
    pub id: i64,
    pub created_at: String,
}

pub fn upsert(
    conn: &mut Connection,
    workspace_id: &str,
    input: AgentFeedbackInput,
) -> Result<AgentFeedbackSaveResult, String> {
    let rating = input.rating.trim().to_ascii_lowercase();
    if rating != "up" && rating != "down" {
        return Err("rating 必须是 up 或 down".into());
    }
    let conversation_id = input.conversation_id.trim().to_string();
    let message_id = input.message_id.trim().to_string();
    if conversation_id.is_empty() || message_id.is_empty() {
        return Err("conversation_id and message_id are required".into());
    }
    let reason = input
        .reason
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let content: String = input.content.chars().take(2000).collect();
    let created_at = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO agent_feedback
           (workspace_id, conversation_id, message_id, rating, reason, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(workspace_id, conversation_id, message_id) DO UPDATE SET
           rating = excluded.rating,
           reason = excluded.reason,
           content = excluded.content,
           created_at = excluded.created_at",
        params![
            workspace_id,
            conversation_id,
            message_id,
            rating,
            reason,
            content,
            created_at,
        ],
    )
    .map_err(|error| error.to_string())?;

    let id: i64 = conn
        .query_row(
            "SELECT id FROM agent_feedback
             WHERE workspace_id = ?1 AND conversation_id = ?2 AND message_id = ?3",
            params![workspace_id, conversation_id, message_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    // Match steel-agent: down votes become a reflection memory for later turns; up removes it.
    let reflection_id = format!("feedback:{message_id}");
    if rating == "down" {
        let reason_label = reason.clone().unwrap_or_default();
        let lesson = format!(
            "用户对该回答给出负反馈{}；后续相似问题应更严格检查是否回答了问题、是否遵守证据边界、是否遗漏用户约束。",
            if reason_label.is_empty() {
                String::new()
            } else {
                format!("（原因：{reason_label}）")
            }
        );
        let _ = crate::storage::repositories::memories::save(
            conn,
            workspace_id,
            crate::models::MemoryInput {
                id: Some(reflection_id),
                scope: "conversation".into(),
                r#type: "reflection_memory".into(),
                title: "用户负反馈".into(),
                description: reason_label,
                body: lesson,
                tags_json: r#"["user_feedback"]"#.into(),
                enabled: true,
            },
        );
    } else {
        let _ = crate::storage::repositories::memories::delete(conn, workspace_id, &reflection_id);
    }

    Ok(AgentFeedbackSaveResult {
        ok: true,
        id,
        created_at,
    })
}

pub fn get(
    conn: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    message_id: &str,
) -> Result<Option<AgentFeedbackRecord>, String> {
    conn.query_row(
        "SELECT id, conversation_id, message_id, rating, reason, content, created_at
         FROM agent_feedback
         WHERE workspace_id = ?1 AND conversation_id = ?2 AND message_id = ?3",
        params![workspace_id, conversation_id, message_id],
        |row| {
            Ok(AgentFeedbackRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                message_id: row.get(2)?,
                rating: row.get(3)?,
                reason: row.get(4)?,
                content: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub fn delete_for_conversation(
    conn: &Connection,
    workspace_id: &str,
    conversation_id: &str,
) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM agent_feedback WHERE workspace_id = ?1 AND conversation_id = ?2",
        params![workspace_id, conversation_id],
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations;

    fn open_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        migrations::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn upsert_updates_same_message_vote() {
        let mut conn = open_db();
        let first = upsert(
            &mut conn,
            "ws",
            AgentFeedbackInput {
                conversation_id: "c1".into(),
                message_id: "m1".into(),
                rating: "up".into(),
                reason: None,
                content: "hello".into(),
            },
        )
        .unwrap();
        let second = upsert(
            &mut conn,
            "ws",
            AgentFeedbackInput {
                conversation_id: "c1".into(),
                message_id: "m1".into(),
                rating: "down".into(),
                reason: Some("incorrect".into()),
                content: "hello again".into(),
            },
        )
        .unwrap();
        assert_eq!(first.id, second.id);
        let row = get(&conn, "ws", "c1", "m1").unwrap().unwrap();
        assert_eq!(row.rating, "down");
        assert_eq!(row.reason.as_deref(), Some("incorrect"));
    }
}
