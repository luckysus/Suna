use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::agent::profiles::AgentProfile;

/// 第 80 章：`agents` 表接线。
///
/// Agent 配置原本以 JSON 存在 `settings` 表里；0033 迁移建了 `agents` 表后
/// 改为「代码预置 + 表内覆盖」：预置仍在 `profiles::presets()`，用户改动落表。
/// 旧的 JSON 数据在首次读取时一次性回填（见 `profiles::backfill_legacy_profiles`）。

const SELECT_COLUMNS: &str = "id, name, description, preset, enabled, status, system_prompt,
    provider_id, tool_ids_json, permission_restrictions_json, limits_json";

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentProfile> {
    let tool_ids_json: String = row.get(8)?;
    let restrictions_json: String = row.get(9)?;
    let limits_json: String = row.get(10)?;
    Ok(AgentProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        preset: row.get::<_, i64>(3)? != 0,
        enabled: row.get::<_, i64>(4)? != 0,
        status: row.get(5)?,
        system_prompt: row.get(6)?,
        provider_id: row.get(7)?,
        tool_ids: serde_json::from_str(&tool_ids_json).unwrap_or_default(),
        permission_restrictions: serde_json::from_str(&restrictions_json).unwrap_or_default(),
        limits: serde_json::from_str(&limits_json).unwrap_or_default(),
    })
}

/// 读取表内的 Agent 覆盖项（不含代码预置）。
pub fn list_overrides(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<AgentProfile>, String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM agents WHERE workspace_id = ?1 ORDER BY created_at ASC"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], row_to_profile)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn count(connection: &Connection, workspace_id: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM agents WHERE workspace_id = ?1",
            params![workspace_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())
}

/// 插入或更新一条 Agent 覆盖项。
pub fn upsert(
    connection: &Connection,
    workspace_id: &str,
    profile: &AgentProfile,
) -> Result<(), String> {
    let tool_ids_json = serde_json::to_string(&profile.tool_ids)
        .map_err(|error| format!("tool_ids must be serializable: {error}"))?;
    let restrictions_json = serde_json::to_string(&profile.permission_restrictions)
        .map_err(|error| format!("permission restrictions must be serializable: {error}"))?;
    let limits_json = serde_json::to_string(&profile.limits)
        .map_err(|error| format!("limits must be serializable: {error}"))?;
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO agents
               (workspace_id, id, name, description, preset, enabled, status, system_prompt,
                provider_id, tool_ids_json, permission_restrictions_json, limits_json,
                created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
             ON CONFLICT(workspace_id, id) DO UPDATE SET
               name = excluded.name,
               description = excluded.description,
               preset = excluded.preset,
               enabled = excluded.enabled,
               status = excluded.status,
               system_prompt = excluded.system_prompt,
               provider_id = excluded.provider_id,
               tool_ids_json = excluded.tool_ids_json,
               permission_restrictions_json = excluded.permission_restrictions_json,
               limits_json = excluded.limits_json,
               updated_at = excluded.updated_at",
            params![
                workspace_id,
                profile.id,
                profile.name,
                profile.description,
                i64::from(profile.preset),
                i64::from(profile.enabled),
                profile.status,
                profile.system_prompt,
                profile.provider_id,
                tool_ids_json,
                restrictions_json,
                limits_json,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 删除覆盖项（预置会回落到代码里的默认值）。
pub fn delete(connection: &Connection, workspace_id: &str, id: &str) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM agents WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 生成一条占位 id（仅为兼容旧调用方的 UUID 依赖）。
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::{count, delete, list_overrides, upsert};
    use crate::agent::profiles::{presets, AgentProfile};
    use crate::storage::migrations::migrate;
    use rusqlite::Connection;

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open database");
        migrate(&mut connection).expect("migrate database");
        connection
    }

    fn sample() -> AgentProfile {
        let mut profile = presets().into_iter().next().expect("a preset exists");
        profile.enabled = false;
        profile.name = "自定义 Master".to_string();
        profile
    }

    #[test]
    fn upserts_reads_and_deletes_overrides() {
        let connection = database();
        let profile = sample();
        upsert(&connection, "ws", &profile).expect("insert override");
        assert_eq!(count(&connection, "ws").expect("count"), 1);

        let overrides = list_overrides(&connection, "ws").expect("list overrides");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].name, "自定义 Master");
        assert!(!overrides[0].enabled);
        assert_eq!(overrides[0].limits.max_turns, profile.limits.max_turns);

        let mut updated = profile.clone();
        updated.name = "再次改名".to_string();
        upsert(&connection, "ws", &updated).expect("update override");
        assert_eq!(count(&connection, "ws").expect("count"), 1);
        assert_eq!(list_overrides(&connection, "ws").expect("relist")[0].name, "再次改名");

        delete(&connection, "ws", &profile.id).expect("delete override");
        assert_eq!(count(&connection, "ws").expect("count"), 0);
    }
}
