use crate::agent::profiles;
use crate::providers::profiles::ProviderCapability;
use crate::storage::repositories::{provider_profiles, settings};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

pub const MODEL_IN_USE_PREFIX: &str = "MODEL_IN_USE:";
pub const MODEL_IN_USE_CODE: u32 = 2300;
const USAGE_LIST_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageResource {
    pub id: String,
    pub name: String,
    pub bindings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageMemory {
    pub bindings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelUsageDetails {
    pub knowledge_bases: Vec<ModelUsageResource>,
    pub agents: Vec<ModelUsageResource>,
    pub long_term_memory: ModelUsageMemory,
    pub knowledge_base_total: usize,
    pub agent_total: usize,
}

impl ModelUsageDetails {
    pub fn is_blocking(&self) -> bool {
        !self.knowledge_bases.is_empty()
            || !self.agents.is_empty()
            || !self.long_term_memory.bindings.is_empty()
    }
}

fn capability_binding(capability: &str) -> &'static str {
    match capability {
        "chat" => "chat_model",
        "embedding" => "embedding_model",
        "rerank" => "rerank_model",
        "document_parser" => "document_parser_model",
        "vllm" => "vlm_model",
        "asr" => "asr_model",
        _ => "unknown",
    }
}

fn push_memory_binding(bindings: &mut BTreeSet<String>, binding: &'static str) {
    bindings.insert(binding.to_string());
}

pub fn collect_profile_usage(
    conn: &Connection,
    workspace_id: &str,
    profile_id: Uuid,
) -> Result<ModelUsageDetails, String> {
    let id = profile_id.to_string();
    let mut memory_bindings = BTreeSet::new();

    let defaults = provider_profiles::list_default_capabilities(conn, workspace_id)?;
    for (capability, profile) in defaults {
        if profile == id {
            push_memory_binding(&mut memory_bindings, capability_binding(&capability));
        }
    }

    for key in [
        "onboarding.completed",
        "onboarding.retrieval",
        "knowledge.preferences",
        "agent.preferences",
        "model.preferences",
    ] {
        let raw = settings::get(conn, workspace_id, key)?;
        if let Some(value) = raw {
            scan_settings_json(&value, &id, &mut memory_bindings);
        }
    }

    let (knowledge_bases, kb_total) = knowledge_base_usage(conn, workspace_id, &id)?;
    let (agents, agent_total) = agent_usage(conn, workspace_id, &id)?;

    Ok(ModelUsageDetails {
        knowledge_bases,
        agents,
        long_term_memory: ModelUsageMemory {
            bindings: memory_bindings.into_iter().collect(),
        },
        knowledge_base_total: kb_total,
        agent_total,
    })
}

fn scan_settings_json(raw: &str, profile_id: &str, bindings: &mut BTreeSet<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return;
    };
    let object = value.as_object();
    if object.is_none() {
        return;
    }
    let object = object.unwrap();
    let checks: [(&str, &'static str); 12] = [
        ("llm_profile_id", "chat_model"),
        ("embedding_profile_id", "embedding_model"),
        ("reranker_profile_id", "rerank_model"),
        ("mineru_profile_id", "document_parser_model"),
        ("vlm_profile_id", "vlm_model"),
        ("asr_profile_id", "asr_model"),
        ("summary_profile_id", "summary_model"),
        ("image_processing_profile_id", "image_processing_model"),
        ("wiki_synthesis_profile_id", "wiki_synthesis_model"),
        ("auto_tag_profile_id", "auto_tag_model"),
        ("query_understand_profile_id", "query_understand_model"),
        ("follow_up_profile_id", "follow_up_model"),
    ];
    for (field, binding) in checks {
        if object.get(field).and_then(|v| v.as_str()) == Some(profile_id) {
            push_memory_binding(bindings, binding);
        }
    }
}

fn knowledge_base_usage(
    conn: &Connection,
    workspace_id: &str,
    profile_id: &str,
) -> Result<(Vec<ModelUsageResource>, usize), String> {
    let mut statement = conn
        .prepare(
            "SELECT kb.id, kb.name, COUNT(DISTINCT v.id) AS version_count
             FROM knowledge_document_versions v
             JOIN knowledge_source_documents d ON d.id = v.document_id AND d.workspace_id = v.workspace_id
             JOIN knowledge_bases kb ON kb.id = d.knowledge_base_id AND kb.workspace_id = d.workspace_id
             WHERE v.workspace_id = ?1 AND v.embedding_profile_id = ?2
             GROUP BY kb.id, kb.name
             ORDER BY kb.name ASC
             LIMIT ?3",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            rusqlite::params![workspace_id, profile_id, USAGE_LIST_LIMIT + 1],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(|error| error.to_string())?;
    let mut collected = Vec::new();
    let mut total = 0usize;
    for row in rows {
        let row = row.map_err(|error| error.to_string())?;
        total += 1;
        if collected.len() < USAGE_LIST_LIMIT {
            collected.push(ModelUsageResource {
                id: row.0,
                name: row.1,
                bindings: vec!["embedding_model".to_string()],
            });
        }
    }
    Ok((collected, total))
}

fn agent_usage(
    conn: &Connection,
    workspace_id: &str,
    profile_id: &str,
) -> Result<(Vec<ModelUsageResource>, usize), String> {
    let profiles = profiles::list(conn, workspace_id)?;
    let mut matches: Vec<ModelUsageResource> = Vec::new();
    for profile in profiles {
        if profile.provider_id.as_deref() == Some(profile_id) {
            matches.push(ModelUsageResource {
                id: profile.id.clone(),
                name: profile.name.clone(),
                bindings: vec!["chat_model".to_string()],
            });
        }
    }
    let total = matches.len();
    matches.truncate(USAGE_LIST_LIMIT);
    Ok((matches, total))
}

pub fn delete_blocked_error(details: &ModelUsageDetails) -> String {
    format!(
        "{}{}",
        MODEL_IN_USE_PREFIX,
        serde_json::json!({
            "code": MODEL_IN_USE_CODE,
            "details": details,
        })
    )
}

pub fn model_type_to_capability(model_type: &str) -> Option<ProviderCapability> {
    match model_type {
        "chat" => Some(ProviderCapability::Chat),
        "embedding" => Some(ProviderCapability::Embedding),
        "rerank" => Some(ProviderCapability::Rerank),
        "document_parser" => Some(ProviderCapability::DocumentParser),
        "vllm" => Some(ProviderCapability::Vllm),
        "asr" => Some(ProviderCapability::Asr),
        _ => None,
    }
}
