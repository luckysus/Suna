use crate::agent::profiles::{self, AgentProfile};
use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::settings;
use serde::Serialize;
use std::collections::BTreeSet;

/// 第 49 章：工具中心启停状态的设置键（JSON 数组，元素为工具 id）。
pub const TOOLS_DISABLED_KEY: &str = "tools.disabled";

pub fn load_disabled_tools(
    connection: &rusqlite::Connection,
    workspace_id: &str,
) -> BTreeSet<String> {
    let raw = settings::get(connection, workspace_id, TOOLS_DISABLED_KEY).unwrap_or(None);
    parse_disabled_tools(raw.as_deref())
}

fn parse_disabled_tools(raw: Option<&str>) -> BTreeSet<String> {
    let Some(raw) = raw else {
        return BTreeSet::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return BTreeSet::new();
    };
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect()
}

fn save_disabled_tools(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    disabled: &BTreeSet<String>,
) -> Result<(), String> {
    let value = serde_json::json!(disabled.iter().collect::<Vec<_>>());
    settings::set(
        connection,
        workspace_id,
        TOOLS_DISABLED_KEY,
        &value.to_string(),
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCapabilitySummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub source: String,
}

#[tauri::command]
pub fn list_agent_profiles(db: tauri::State<DbState>) -> Result<Vec<AgentProfile>, String> {
    with_conn(&db, |connection| {
        profiles::list(connection, current_workspace_id())
    })
}

#[tauri::command]
pub fn list_agent_profile_presets() -> Vec<AgentProfile> {
    profiles::presets()
}

#[tauri::command]
pub fn save_agent_profile(
    db: tauri::State<DbState>,
    profile: AgentProfile,
) -> Result<AgentProfile, String> {
    with_conn_mut(&db, |connection| {
        profiles::save(connection, current_workspace_id(), profile)
    })
}

#[tauri::command]
pub fn reset_agent_profile(db: tauri::State<DbState>, id: String) -> Result<AgentProfile, String> {
    with_conn_mut(&db, |connection| {
        profiles::reset(connection, current_workspace_id(), &id)
    })
}

#[tauri::command]
pub fn delete_agent_profile(db: tauri::State<DbState>, id: String) -> Result<(), String> {
    with_conn_mut(&db, |connection| {
        profiles::delete(connection, current_workspace_id(), &id)
    })
}

#[tauri::command]
pub fn list_tool_capabilities(
    db: tauri::State<DbState>,
) -> Result<Vec<ToolCapabilitySummary>, String> {
    with_conn(&db, |connection| {
        let disabled = load_disabled_tools(connection, current_workspace_id());
        Ok(tool_capability_summaries(&disabled))
    })
}

#[tauri::command]
pub fn set_tool_enabled(
    db: tauri::State<DbState>,
    id: String,
    enabled: bool,
) -> Result<Vec<ToolCapabilitySummary>, String> {
    with_conn_mut(&db, |connection| {
        let workspace_id = current_workspace_id();
        let mut disabled = load_disabled_tools(connection, workspace_id);
        if enabled {
            disabled.remove(&id);
        } else {
            disabled.insert(id.clone());
        }
        save_disabled_tools(connection, workspace_id, &disabled)?;
        settings::record_audit(connection, workspace_id, "tool.update", &id, "success")?;
        Ok(tool_capability_summaries(&disabled))
    })
}

pub fn tool_capability_summaries(disabled: &BTreeSet<String>) -> Vec<ToolCapabilitySummary> {
    profiles::builtin_tool_capabilities()
        .into_iter()
        .map(|(id, name, description)| ToolCapabilitySummary {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            enabled: !disabled.contains(id),
            source: "builtin".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{list_agent_profile_presets, tool_capability_summaries};
    use std::collections::BTreeSet;

    #[test]
    fn exposes_the_documented_agent_roster() {
        let agents = list_agent_profile_presets();
        assert_eq!(agents.len(), 9);
        assert_eq!(agents[0].id, "master");
        assert!(agents.iter().all(|agent| agent.enabled));
    }

    #[test]
    fn exposes_actual_runtime_tool_ids() {
        let tools = tool_capability_summaries(&BTreeSet::new());
        assert!(tools
            .iter()
            .any(|tool| tool.id == "steel.predict_performance" && tool.enabled));
        assert!(tools
            .iter()
            .all(|tool| tool.source == "builtin" && tool.id.contains('.')));
    }

    #[test]
    fn disabled_tool_ids_flip_the_enabled_flag() {
        let mut disabled = BTreeSet::new();
        disabled.insert("builtin.powershell".to_string());
        let tools = tool_capability_summaries(&disabled);
        let powershell = tools
            .iter()
            .find(|tool| tool.id == "builtin.powershell")
            .expect("powershell capability");
        assert!(!powershell.enabled);
        assert!(tools
            .iter()
            .filter(|tool| tool.id != "builtin.powershell")
            .all(|tool| tool.enabled));
    }

    #[test]
    fn disabled_tool_setting_round_trips_through_settings() {
        let mut connection = rusqlite::Connection::open_in_memory().expect("open database");
        crate::storage::migrations::migrate(&mut connection).expect("migrate database");
        let workspace = "local";

        let mut disabled = super::load_disabled_tools(&connection, workspace);
        assert!(disabled.is_empty(), "no tool is disabled by default");
        disabled.insert("builtin.write_file".to_string());
        disabled.insert("steel.optimize_constrained".to_string());
        super::save_disabled_tools(&mut connection, workspace, &disabled)
            .expect("save disabled tools");

        let reloaded = super::load_disabled_tools(&connection, workspace);
        assert_eq!(reloaded, disabled);
    }
}
