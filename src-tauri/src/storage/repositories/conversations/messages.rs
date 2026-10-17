use super::{ensure_id_available, normalize_role, now};
use crate::models::{HistoryHit, Message, MessagePage};
use crate::retrieval::{search, SearchDocument};
use rusqlite::{params, Connection, Transaction};
use uuid::Uuid;

pub fn list_messages(
    conn: &Connection,
    workspace_id: &str,
    conversation_id: &str,
) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, conversation_id, role, content, response_json, created_at
         FROM messages WHERE workspace_id = ?1 AND conversation_id = ?2
         ORDER BY created_at ASC, rowid ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![workspace_id, conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                response_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

const DEFAULT_MESSAGE_PAGE_LIMIT: usize = 80;
const MAX_MESSAGE_PAGE_LIMIT: usize = 200;

pub fn list_messages_page(
    conn: &Connection,
    workspace_id: &str,
    conversation_id: &str,
    limit: usize,
    before_rowid: Option<i64>,
    before_created_at: Option<&str>,
) -> Result<MessagePage, String> {
    let limit = limit.clamp(1, MAX_MESSAGE_PAGE_LIMIT);
    let fetch_limit = limit + 1;
    let rows: Vec<Message> =
        if let (Some(rowid), Some(created_at)) = (before_rowid, before_created_at) {
            let mut stmt = conn
                .prepare(
                    "SELECT id, conversation_id, role, content, response_json, created_at
         FROM messages
         WHERE workspace_id = ?1 AND conversation_id = ?2
           AND (created_at < ?3 OR (created_at = ?3 AND rowid < ?4))
         ORDER BY created_at DESC, rowid DESC
         LIMIT ?5",
                )
                .map_err(|error| error.to_string())?;
            let mapped = stmt
                .query_map(
                    params![
                        workspace_id,
                        conversation_id,
                        created_at,
                        rowid,
                        fetch_limit
                    ],
                    |row| {
                        Ok(Message {
                            id: row.get(0)?,
                            conversation_id: row.get(1)?,
                            role: row.get(2)?,
                            content: row.get(3)?,
                            response_json: row.get(4)?,
                            created_at: row.get(5)?,
                        })
                    },
                )
                .map_err(|error| error.to_string())?;
            mapped
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT id, conversation_id, role, content, response_json, created_at
         FROM messages
         WHERE workspace_id = ?1 AND conversation_id = ?2
         ORDER BY created_at DESC, rowid DESC
         LIMIT ?3",
                )
                .map_err(|error| error.to_string())?;
            let mapped = stmt
                .query_map(params![workspace_id, conversation_id, fetch_limit], |row| {
                    Ok(Message {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        response_json: row.get(4)?,
                        created_at: row.get(5)?,
                    })
                })
                .map_err(|error| error.to_string())?;
            mapped
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };

    let has_older = rows.len() > limit;
    let page_rows = if has_older {
        rows.into_iter().take(limit).collect::<Vec<_>>()
    } else {
        rows
    };
    let mut messages = page_rows;
    messages.reverse();

    let (oldest_rowid, oldest_created_at) = if messages.is_empty() {
        (None, None)
    } else {
        let oldest = &messages[0];
        let rowid = conn
            .query_row(
                "SELECT rowid FROM messages WHERE workspace_id = ?1 AND id = ?2",
                params![workspace_id, oldest.id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        (Some(rowid), Some(oldest.created_at.clone()))
    };

    Ok(MessagePage {
        messages,
        has_older,
        oldest_rowid,
        oldest_created_at,
    })
}

pub fn list_messages_page_default(
    conn: &Connection,
    workspace_id: &str,
    conversation_id: &str,
) -> Result<MessagePage, String> {
    list_messages_page(
        conn,
        workspace_id,
        conversation_id,
        DEFAULT_MESSAGE_PAGE_LIMIT,
        None,
        None,
    )
}

pub fn search_history(
    conn: &Connection,
    workspace_id: &str,
    query: &str,
    conversation_id: Option<&str>,
    exclude_current: bool,
    limit: usize,
) -> Result<Vec<HistoryHit>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT m.id, m.conversation_id, c.title, m.role, m.content, m.created_at
         FROM messages m LEFT JOIN conversations c
           ON c.workspace_id = m.workspace_id AND c.id = m.conversation_id
         WHERE m.workspace_id = ?1 AND TRIM(m.content) != ''
         ORDER BY m.created_at DESC LIMIT 400",
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![workspace_id], |row| {
            Ok(HistoryHit {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_title: row
                    .get::<_, Option<String>>(2)?
                    .unwrap_or_else(|| "Local conversation".to_string()),
                role: row.get(3)?,
                content: row.get(4)?,
                created_at: row.get(5)?,
                score: 0.0,
                snippet: String::new(),
            })
        })
        .map_err(|error| error.to_string())?;
    let mut hits = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if let Some(conversation_id) = conversation_id.filter(|value| !value.trim().is_empty()) {
        if exclude_current {
            hits.retain(|hit| hit.conversation_id != conversation_id);
        } else {
            hits.retain(|hit| hit.conversation_id == conversation_id);
        }
    }
    rank_history_hits(query, hits, limit.clamp(1, 20))
}

pub fn rank_history_hits(
    query: &str,
    hits: Vec<HistoryHit>,
    limit: usize,
) -> Result<Vec<HistoryHit>, String> {
    let documents = hits
        .iter()
        .enumerate()
        .map(|(index, hit)| SearchDocument {
            index,
            text: format!(
                "{}\n{}\n{}\n{}",
                hit.conversation_title, hit.role, hit.created_at, hit.content
            ),
        })
        .collect::<Vec<_>>();
    Ok(search(query, &documents, limit, 240)
        .into_iter()
        .filter_map(|ranked| {
            let mut hit = hits.get(ranked.index)?.clone();
            hit.score = ranked.score;
            hit.snippet = ranked.snippet;
            Some(hit)
        })
        .collect())
}

pub fn append_message(
    conn: &mut Connection,
    workspace_id: &str,
    conversation_id: &str,
    role: &str,
    content: &str,
    response_json: Option<String>,
) -> Result<Message, String> {
    if conversation_id.trim().is_empty() {
        return Err("conversation_id is required".to_string());
    }
    let id = Uuid::new_v4();
    let timestamp = now();
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    let message = append_message_in_transaction(
        &tx,
        workspace_id,
        conversation_id,
        id,
        role,
        content,
        response_json,
        &timestamp,
    )?;
    tx.commit().map_err(|error| error.to_string())?;
    Ok(message)
}

#[allow(clippy::too_many_arguments)]
pub fn append_message_in_transaction(
    tx: &Transaction<'_>,
    workspace_id: &str,
    conversation_id: &str,
    message_id: Uuid,
    role: &str,
    content: &str,
    response_json: Option<String>,
    timestamp: &str,
) -> Result<Message, String> {
    if conversation_id.trim().is_empty() {
        return Err("conversation_id is required".to_string());
    }
    let role = normalize_role(role)?;
    if !super::basic::belongs_to_workspace(tx, workspace_id, conversation_id)? {
        ensure_id_available(tx, workspace_id, conversation_id)?;
        return Err("conversation not found".to_string());
    }
    let id = message_id.to_string();
    tx.execute(
        "INSERT INTO messages (id, workspace_id, conversation_id, role, content, response_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, workspace_id, conversation_id, role, content, response_json, timestamp],
    ).map_err(|error| error.to_string())?;
    tx.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE workspace_id = ?2 AND id = ?3",
        params![timestamp, workspace_id, conversation_id],
    )
    .map_err(|error| error.to_string())?;
    Ok(Message {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        response_json,
        created_at: timestamp.to_string(),
    })
}
