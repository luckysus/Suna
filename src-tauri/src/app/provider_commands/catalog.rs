use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogModelEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub dimension: Option<u32>,
    #[serde(default)]
    pub input: Vec<String>,
    #[serde(default)]
    pub reasoning: Option<bool>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogCredentialField {
    pub id: String,
    pub label: String,
    #[serde(default = "default_true")]
    pub secret: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogExtraField {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub secret: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogProvider {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default_urls: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "model_types")]
    pub model_types: Vec<String>,
    pub kind: String,
    #[serde(default)]
    pub models: Vec<CatalogModelEntry>,
    #[serde(default)]
    pub icon_svg: Option<String>,
    #[serde(default)]
    pub credential_fields: Vec<CatalogCredentialField>,
    #[serde(default)]
    pub extra_fields: Vec<CatalogExtraField>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CatalogDocument {
    version: u32,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    synced_at: Option<String>,
    providers: Vec<CatalogProvider>,
}

/// WeKnora raw catalog entry (id and/or match pattern).
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct WeknoraModelSpec {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    match_pattern: Option<String>,
    #[serde(rename = "match", default)]
    match_glob: Option<String>,
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    dimension: Option<u32>,
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    reasoning: Option<bool>,
    #[serde(default)]
    deprecated: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct WeknoraCatalogDocument {
    providers: std::collections::BTreeMap<String, Vec<WeknoraModelSpec>>,
}

fn load_catalog() -> Result<CatalogDocument, String> {
    let raw = include_str!("../../../resources/model_provider_catalog.json");
    serde_json::from_str(raw).map_err(|error| format!("invalid model catalog: {error}"))
}

fn load_weknora_raw() -> Result<WeknoraCatalogDocument, String> {
    let raw = include_str!("../../../resources/weknora_models.generated.json");
    serde_json::from_str(raw).map_err(|error| format!("invalid weknora catalog: {error}"))
}

fn map_suna_type(model_type: &str) -> &str {
    match model_type {
        "chat" | "vllm" => "KnowledgeQA",
        "embedding" => "Embedding",
        "rerank" => "Rerank",
        "asr" => "ASR",
        other => other,
    }
}

fn entry_type(spec: &WeknoraModelSpec) -> String {
    match spec.r#type.as_deref() {
        None | Some("") => "KnowledgeQA".to_string(),
        Some(value) => value.to_string(),
    }
}

fn glob_match(pattern: &str, value: &str) -> bool {
    let pattern = pattern.to_ascii_lowercase();
    let value = value.to_ascii_lowercase();
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == value;
    }
    if !value.starts_with(parts[0]) {
        return false;
    }
    let mut rest = &value[parts[0].len()..];
    for part in &parts[1..parts.len() - 1] {
        if let Some(idx) = rest.find(part) {
            rest = &rest[idx + part.len()..];
        } else {
            return false;
        }
    }
    rest.ends_with(parts[parts.len() - 1])
}

fn find_weknora_spec(
    document: &WeknoraCatalogDocument,
    provider_id: Option<&str>,
    model_id: &str,
    model_type: &str,
) -> Option<(String, WeknoraModelSpec)> {
    let needle = model_id.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return None;
    }
    let want = map_suna_type(model_type);
    let want_vllm = model_type.eq_ignore_ascii_case("vllm");

    let owned_key;
    let providers: Vec<(&String, &Vec<WeknoraModelSpec>)> = if let Some(id) = provider_id {
        owned_key = id.to_string();
        document
            .providers
            .get(&owned_key)
            .map(|entries| vec![(&owned_key, entries)])
            .unwrap_or_default()
    } else {
        document.providers.iter().collect()
    };

    for (provider, entries) in providers {
        let candidates: Vec<&WeknoraModelSpec> = entries
            .iter()
            .filter(|entry| {
                if entry.deprecated.unwrap_or(false) {
                    return false;
                }
                let entry_ty = entry_type(entry);
                if want_vllm {
                    entry_ty == "KnowledgeQA" && entry.input.iter().any(|item| item == "image")
                } else {
                    entry_ty.eq_ignore_ascii_case(want)
                        || (want == "KnowledgeQA" && entry_ty.is_empty())
                }
            })
            .collect();

        for entry in &candidates {
            if entry
                .id
                .as_deref()
                .map(|id| id.eq_ignore_ascii_case(&needle))
                .unwrap_or(false)
            {
                return Some((provider.clone(), (*entry).clone()));
            }
        }
        let mut best: Option<&WeknoraModelSpec> = None;
        let mut best_score = -1i32;
        for entry in &candidates {
            let pattern = entry
                .match_glob
                .as_deref()
                .or(entry.match_pattern.as_deref())
                .unwrap_or("");
            if pattern.is_empty() {
                continue;
            }
            if glob_match(pattern, &needle) {
                let literal = pattern
                    .split('*')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches('*');
                let score = literal.len() as i32;
                if score > best_score {
                    best = Some(*entry);
                    best_score = score;
                }
            }
        }
        if let Some(entry) = best {
            return Some((provider.clone(), entry.clone()));
        }
    }
    None
}

pub fn resolve_capabilities(
    provider_id: Option<&str>,
    model_id: &str,
    model_type: &str,
) -> Result<serde_json::Value, String> {
    let document = load_catalog()?;
    let provider_id = provider_id.filter(|value| !value.trim().is_empty());

    // Prefer exact picker entries from the bundled Suna catalog.
    let provider = provider_id
        .and_then(|id| document.providers.iter().find(|entry| entry.id == id))
        .or_else(|| {
            document
                .providers
                .iter()
                .find(|entry| entry.models.iter().any(|model| model.id == model_id))
        });
    if let Some(provider) = provider {
        if let Some(model) = provider.models.iter().find(|model| model.id == model_id) {
            return Ok(serde_json::json!({
                "cataloged": true,
                "provider": provider.id,
                "model_type": model_type,
                "context_window": model.context_window,
                "max_output_tokens": model.max_output_tokens,
                "dimension": model.dimension,
                "input": model.input,
                "supports_vision": model.input.iter().any(|item| item == "image"),
                "reasoning": model.reasoning,
                "source": model.source,
                "match": false,
            }));
        }
    }

    // Fall back to WeKnora family match rules (qwen3*, gpt-5*, ...).
    if let Ok(raw) = load_weknora_raw() {
        if let Some((provider, model)) = find_weknora_spec(&raw, provider_id, model_id, model_type)
        {
            return Ok(serde_json::json!({
                "cataloged": true,
                "provider": provider,
                "model_type": model_type,
                "context_window": model.context_window,
                "max_output_tokens": model.max_output_tokens,
                "dimension": model.dimension,
                "input": model.input,
                "supports_vision": model.input.iter().any(|item| item == "image"),
                "reasoning": model.reasoning,
                "match": model.match_glob.or(model.match_pattern),
                "resolved_name": model.name.or(model.id),
            }));
        }
    }

    Ok(serde_json::json!({
        "cataloged": false,
        "provider": provider_id,
        "model_type": model_type,
        "context_window": null,
        "dimension": null,
        "input": [],
        "supports_vision": false,
    }))
}

pub fn list_providers(model_type: Option<&str>) -> Result<Vec<CatalogProvider>, String> {
    let document = load_catalog()?;
    let filter = model_type
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && *value != "all");
    Ok(document
        .providers
        .into_iter()
        .filter(|provider| {
            filter.as_ref().map_or(true, |model_type| {
                provider.model_types.iter().any(|entry| entry == model_type)
            })
        })
        .map(|provider| {
            if filter.is_none() {
                return provider;
            }
            let model_type = filter.as_ref().unwrap();
            let models = provider
                .models
                .iter()
                .filter(|model| {
                    model.types.is_empty() || model.types.iter().any(|t| t == model_type)
                })
                .cloned()
                .collect();
            CatalogProvider { models, ..provider }
        })
        .collect())
}
