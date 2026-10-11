pub mod bundled;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_SKILL_BYTES: usize = 1024 * 1024;
const MAX_ENABLED_SKILLS: usize = 12;
const MAX_RENDERED_SKILL_PROMPT_CHARS: usize = 12_000;
const MAX_RENDERED_SKILL_BODY_CHARS: usize = 8_000;
pub const SKILLS_SETTING_KEY: &str = "extensions.skills.enabled";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillScope {
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRoot {
    pub scope: SkillScope,
    pub path: PathBuf,
}

impl SkillRoot {
    pub fn new(scope: SkillScope, path: PathBuf) -> Self {
        Self { scope, path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSource {
    pub scope: SkillScope,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecord {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub compatibility: Vec<String>,
    pub body: String,
    pub source: SkillSource,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillErrorCode {
    ReadFailed,
    Oversized,
    InvalidUtf8,
    MissingFrontmatter,
    InvalidFrontmatter,
    MissingField,
    InvalidField,
    InvalidName,
    InvalidVersion,
    InvalidCompatibility,
    Incompatible,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillLoadError {
    pub code: SkillErrorCode,
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDiscoveryReport {
    pub skills: Vec<SkillRecord>,
    pub errors: Vec<SkillLoadError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSummary {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub compatibility: Vec<String>,
    pub source: SkillSource,
    pub content_sha256: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedSkill {
    pub name: String,
    pub version: String,
    pub content_sha256: String,
    pub trigger_reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillCatalog {
    pub skills: Vec<SkillSummary>,
    pub errors: Vec<SkillLoadError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedSkills {
    pub prompt: String,
    pub enabled_versions: Vec<String>,
    pub loaded: Vec<LoadedSkill>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillContext {
    pub summaries: Vec<SkillSummary>,
    pub rendered: RenderedSkills,
    pub errors: Vec<SkillLoadError>,
}

pub fn discover_skills(roots: &[SkillRoot], app_version: &str) -> SkillDiscoveryReport {
    let mut roots = roots.to_vec();
    roots.sort_by(|left, right| {
        left.path
            .to_string_lossy()
            .cmp(&right.path.to_string_lossy())
    });

    let mut report = SkillDiscoveryReport::default();
    let mut selected = BTreeMap::<String, SkillRecord>::new();
    for root in roots {
        let mut directories = match fs::read_dir(&root.path) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
                .collect::<Vec<_>>(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                report.errors.push(SkillLoadError {
                    code: SkillErrorCode::ReadFailed,
                    path: root.path,
                    message: error.to_string(),
                });
                continue;
            }
        };
        directories.sort_by_key(|entry| entry.file_name());

        for directory in directories {
            let path = directory.path().join("SKILL.md");
            if !path.is_file() {
                continue;
            }
            let Some(skill) = load_skill(&path, root.scope, app_version, &mut report.errors) else {
                continue;
            };
            if selected.contains_key(&skill.name) {
                report.errors.push(SkillLoadError {
                    code: SkillErrorCode::Duplicate,
                    path: skill.source.path.clone(),
                    message: format!(
                        "skill '{}' is shadowed by a higher-precedence source",
                        skill.name
                    ),
                });
                continue;
            }
            selected.insert(skill.name.clone(), skill);
        }
    }
    report.skills = selected.into_values().collect();
    report
}

pub fn default_skill_roots() -> Vec<SkillRoot> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        push_root(
            &mut roots,
            SkillScope::User,
            home.join(".suna").join("skills"),
        );
    }
    roots
}

fn push_root(roots: &mut Vec<SkillRoot>, scope: SkillScope, path: PathBuf) {
    if !roots
        .iter()
        .any(|root| root.scope == scope && root.path == path)
    {
        roots.push(SkillRoot::new(scope, path));
    }
}

pub fn summarize_skills(
    skills: &[SkillRecord],
    enabled_names: &BTreeSet<String>,
) -> Vec<SkillSummary> {
    skills
        .iter()
        .map(|skill| SkillSummary {
            name: skill.name.clone(),
            description: skill.description.clone(),
            version: skill.version.clone(),
            tags: skill.tags.clone(),
            compatibility: skill.compatibility.clone(),
            source: skill.source.clone(),
            content_sha256: skill.content_sha256.clone(),
            enabled: enabled_names.contains(&skill.name),
        })
        .collect()
}

pub fn render_enabled_skills(
    skills: &[SkillRecord],
    enabled_names: &BTreeSet<String>,
) -> RenderedSkills {
    render_skills(skills, enabled_names, |_| {
        Some("enabled_by_user".to_string())
    })
}

pub fn render_relevant_skills(
    skills: &[SkillRecord],
    enabled_names: &BTreeSet<String>,
    query: &str,
) -> RenderedSkills {
    let reasons = skills
        .iter()
        .filter(|skill| enabled_names.contains(&skill.name))
        .filter_map(|skill| {
            match_skill_query(skill, query).map(|reason| (skill.name.clone(), reason))
        })
        .collect::<BTreeMap<_, _>>();

    if reasons.is_empty() {
        return render_enabled_skills(skills, enabled_names);
    }

    render_skills(skills, enabled_names, |skill| {
        reasons.get(&skill.name).cloned()
    })
}

fn render_skills<F>(
    skills: &[SkillRecord],
    enabled_names: &BTreeSet<String>,
    mut trigger_reason: F,
) -> RenderedSkills
where
    F: FnMut(&SkillRecord) -> Option<String>,
{
    let mut prompt_sections = Vec::new();
    let mut enabled_versions = Vec::new();
    let mut loaded = Vec::new();
    let mut prompt_chars = "enabled_skills:".chars().count();

    for skill in skills
        .iter()
        .filter(|skill| enabled_names.contains(&skill.name))
    {
        if loaded.len() >= MAX_ENABLED_SKILLS {
            break;
        }
        let Some(trigger_reason) = trigger_reason(skill) else {
            continue;
        };
        let version = format!("{}@{}#{}", skill.name, skill.version, skill.content_sha256);
        let header = format!("## {} (v{})", skill.name, skill.version);
        let available = MAX_RENDERED_SKILL_PROMPT_CHARS
            .saturating_sub(prompt_chars + header.chars().count() + 4);
        if available == 0 {
            break;
        }
        let body = truncate_text(&skill.body, available.min(MAX_RENDERED_SKILL_BODY_CHARS));
        let section = format!("{header}\n{body}");
        prompt_chars += section.chars().count() + 2;
        prompt_sections.push(section);
        enabled_versions.push(version);
        loaded.push(LoadedSkill {
            name: skill.name.clone(),
            version: skill.version.clone(),
            content_sha256: skill.content_sha256.clone(),
            trigger_reason,
        });
    }

    let prompt = if prompt_sections.is_empty() {
        String::new()
    } else {
        format!("enabled_skills:\n\n{}", prompt_sections.join("\n\n"))
    };
    RenderedSkills {
        prompt,
        enabled_versions,
        loaded,
    }
}

fn match_skill_query(skill: &SkillRecord, query: &str) -> Option<String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return None;
    }

    for tag in &skill.tags {
        if tag_matches_query(tag, &query) {
            return Some(format!("matched_query_tag:{tag}"));
        }
    }
    if metadata_matches_query(&skill.name, &query) {
        return Some("matched_query_name".to_string());
    }
    if metadata_matches_query(&skill.description, &query) {
        return Some("matched_query_description".to_string());
    }
    None
}

fn tag_matches_query(tag: &str, query: &str) -> bool {
    let tag_lower = tag.to_lowercase();
    query.contains(&tag_lower)
        || (tag_lower == "steel" && has_any(query, &["钢", "钢铁", "合金", "热轧", "q345"]))
        || (tag_lower == "evidence" && has_any(query, &["证据", "引用", "文献"]))
}

fn metadata_matches_query(value: &str, query: &str) -> bool {
    let value = value.to_lowercase();
    value.contains(query)
        || query
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|term| term.chars().count() >= 2)
            .any(|term| value.contains(term))
}

fn has_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

pub fn load_enabled_names(
    connection: &rusqlite::Connection,
    workspace_id: &str,
) -> Result<BTreeSet<String>, String> {
    let raw =
        crate::storage::repositories::settings::get(connection, workspace_id, SKILLS_SETTING_KEY)?;
    Ok(parse_enabled_names(raw.as_deref()))
}

pub fn parse_enabled_names(raw: Option<&str>) -> BTreeSet<String> {
    let Some(raw) = raw else {
        return BTreeSet::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return BTreeSet::new();
    };
    value
        .get("enabled")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .filter(|name| valid_name(name))
        .map(str::to_string)
        .collect()
}

pub fn save_enabled_names(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    names: &BTreeSet<String>,
) -> Result<(), String> {
    let value = serde_json::json!({
        "version": 1,
        "enabled": names.iter().collect::<Vec<_>>(),
    });
    crate::storage::repositories::settings::set(
        connection,
        workspace_id,
        SKILLS_SETTING_KEY,
        &value.to_string(),
    )
}

pub fn load_context(
    connection: &rusqlite::Connection,
    workspace_id: &str,
    app_version: &str,
) -> Result<SkillContext, String> {
    let enabled_names = load_enabled_names(connection, workspace_id)?;
    let report = discover_skills(&default_skill_roots(), app_version);
    Ok(SkillContext {
        summaries: summarize_skills(&report.skills, &enabled_names),
        rendered: render_enabled_skills(&report.skills, &enabled_names),
        errors: report.errors,
    })
}

pub fn load_context_for_query(
    connection: &rusqlite::Connection,
    workspace_id: &str,
    app_version: &str,
    query: &str,
) -> Result<SkillContext, String> {
    let enabled_names = load_enabled_names(connection, workspace_id)?;
    let report = discover_skills(&default_skill_roots(), app_version);
    Ok(SkillContext {
        summaries: summarize_skills(&report.skills, &enabled_names),
        rendered: render_relevant_skills(&report.skills, &enabled_names, query),
        errors: report.errors,
    })
}

pub fn catalog(
    connection: &rusqlite::Connection,
    workspace_id: &str,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    let context = load_context(connection, workspace_id, app_version)?;
    Ok(SkillCatalog {
        skills: context.summaries,
        errors: context.errors,
    })
}

pub fn set_enabled(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    name: &str,
    enabled: bool,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err("skill name is invalid".to_string());
    }
    let report = discover_skills(&default_skill_roots(), app_version);
    if !report.skills.iter().any(|skill| skill.name == name) {
        return Err("skill not found".to_string());
    }
    let mut names = load_enabled_names(connection, workspace_id)?;
    if enabled {
        validate_enabled_limit(&names, name)?;
        names.insert(name.to_string());
    } else {
        names.remove(name);
    }
    save_enabled_names(connection, workspace_id, &names)?;
    catalog(connection, workspace_id, app_version)
}

fn validate_enabled_limit(names: &BTreeSet<String>, name: &str) -> Result<(), String> {
    if !names.contains(name) && names.len() >= MAX_ENABLED_SKILLS {
        return Err(format!(
            "最多同时启用 {MAX_ENABLED_SKILLS} 个 Skill，请先停用一个已启用的 Skill"
        ));
    }
    Ok(())
}

fn load_skill(
    path: &Path,
    scope: SkillScope,
    app_version: &str,
    errors: &mut Vec<SkillLoadError>,
) -> Option<SkillRecord> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            push_error(errors, SkillErrorCode::ReadFailed, path, error.to_string());
            return None;
        }
    };
    if bytes.len() > MAX_SKILL_BYTES {
        push_error(
            errors,
            SkillErrorCode::Oversized,
            path,
            "SKILL.md exceeds the size limit",
        );
        return None;
    }
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            push_error(
                errors,
                SkillErrorCode::InvalidUtf8,
                path,
                "SKILL.md is not valid UTF-8",
            );
            return None;
        }
    };
    let (fields, body) = match parse_frontmatter(&text) {
        Ok(value) => value,
        Err((code, message)) => {
            push_error(errors, code, path, message);
            return None;
        }
    };
    let Some(name) = required(&fields, "name", path, errors) else {
        return None;
    };
    if !valid_name(&name) {
        push_error(
            errors,
            SkillErrorCode::InvalidName,
            path,
            "skill name contains invalid characters",
        );
        return None;
    }
    let Some(description) = required(&fields, "description", path, errors) else {
        return None;
    };
    let version = fields
        .get("version")
        .cloned()
        .unwrap_or_else(|| "0.0.0".to_string());
    if !valid_version(&version) {
        push_error(
            errors,
            SkillErrorCode::InvalidVersion,
            path,
            "skill version must be major.minor.patch",
        );
        return None;
    }
    let tags = match fields.get("tags") {
        Some(value) => match parse_tags(value) {
            Ok(value) => value,
            Err(message) => {
                push_error(errors, SkillErrorCode::InvalidField, path, message);
                return None;
            }
        },
        None => Vec::new(),
    };
    let compatibility = match fields.get("compatibility") {
        Some(value) => match parse_compatibility(value) {
            Ok(value) => value,
            Err(message) => {
                push_error(errors, SkillErrorCode::InvalidCompatibility, path, message);
                return None;
            }
        },
        None => Vec::new(),
    };
    match compatible(&compatibility, app_version) {
        Ok(true) => {}
        Ok(false) => {
            push_error(
                errors,
                SkillErrorCode::Incompatible,
                path,
                "skill is incompatible with this Suna version",
            );
            return None;
        }
        Err(message) => {
            push_error(errors, SkillErrorCode::InvalidCompatibility, path, message);
            return None;
        }
    }

    let digest = Sha256::digest(text.as_bytes());
    Some(SkillRecord {
        name,
        description,
        version,
        tags,
        compatibility,
        body,
        source: SkillSource {
            scope,
            path: path.to_path_buf(),
        },
        content_sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}

fn parse_frontmatter(
    text: &str,
) -> Result<(BTreeMap<String, String>, String), (SkillErrorCode, String)> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err((
            SkillErrorCode::MissingFrontmatter,
            "SKILL.md must start with frontmatter".to_string(),
        ));
    }
    let mut fields = BTreeMap::new();
    let mut closed = false;
    for line in lines.by_ref() {
        let line = line.trim();
        if line == "---" {
            closed = true;
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err((
                SkillErrorCode::InvalidFrontmatter,
                "frontmatter lines must use key: value".to_string(),
            ));
        };
        let key = key.trim();
        let value = unquote(value.trim());
        if key.is_empty() || value.is_empty() {
            return Err((
                SkillErrorCode::InvalidFrontmatter,
                "frontmatter keys and values must not be empty".to_string(),
            ));
        }
        if fields.insert(key.to_string(), value).is_some() {
            return Err((
                SkillErrorCode::InvalidField,
                format!("frontmatter field '{key}' is duplicated"),
            ));
        }
    }
    if !closed {
        return Err((
            SkillErrorCode::InvalidFrontmatter,
            "frontmatter closing marker is missing".to_string(),
        ));
    }
    Ok((
        fields,
        lines.collect::<Vec<_>>().join("\n").trim().to_string(),
    ))
}

fn required(
    fields: &BTreeMap<String, String>,
    name: &str,
    path: &Path,
    errors: &mut Vec<SkillLoadError>,
) -> Option<String> {
    match fields.get(name) {
        Some(value) => Some(value.clone()),
        None => {
            push_error(
                errors,
                SkillErrorCode::MissingField,
                path,
                format!("frontmatter field '{name}' is missing"),
            );
            None
        }
    }
}

fn parse_compatibility(value: &str) -> Result<Vec<String>, String> {
    let value = value.trim();
    if value.is_empty() || value == "*" {
        return Ok(Vec::new());
    }
    let values = if value.starts_with('[') && value.ends_with(']') {
        value[1..value.len() - 1]
            .split(',')
            .map(|item| unquote(item.trim()))
            .collect::<Vec<_>>()
    } else {
        vec![unquote(value)]
    };
    if values.iter().any(|item| item.is_empty()) {
        return Err("compatibility contains an empty constraint".to_string());
    }
    for item in &values {
        let version = item
            .strip_prefix("suna")
            .unwrap_or(item)
            .trim_start_matches(">=")
            .trim_start_matches('=')
            .trim();
        if !valid_version(version) {
            return Err(format!("invalid compatibility constraint '{item}'"));
        }
    }
    Ok(values)
}

fn parse_tags(value: &str) -> Result<Vec<String>, String> {
    let tags = parse_list(value);
    if tags.iter().any(|tag| !valid_name(tag)) {
        return Err("skill tags must use letters, digits, dot, dash, or underscore".to_string());
    }
    Ok(tags)
}

fn compatible(constraints: &[String], app_version: &str) -> Result<bool, String> {
    let current = parse_version(app_version)
        .ok_or_else(|| "Suna version must be major.minor.patch".to_string())?;
    for constraint in constraints {
        let version = constraint
            .strip_prefix("suna")
            .unwrap_or(constraint)
            .trim_start_matches(">=")
            .trim_start_matches('=')
            .trim();
        let required =
            parse_version(version).ok_or_else(|| "invalid compatibility version".to_string())?;
        if current < required {
            return Ok(false);
        }
    }
    Ok(true)
}

fn parse_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.is_empty() {
        return Vec::new();
    }
    if value.starts_with('[') && value.ends_with(']') {
        value[1..value.len() - 1]
            .split(',')
            .map(|item| unquote(item.trim()))
            .filter(|item| !item.is_empty())
            .collect()
    } else {
        vec![unquote(value)]
    }
}

fn parse_version(value: &str) -> Option<(u64, u64, u64)> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn valid_version(value: &str) -> bool {
    parse_version(value).is_some()
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].trim().to_string()
    } else {
        value.to_string()
    }
}

fn push_error(
    errors: &mut Vec<SkillLoadError>,
    code: SkillErrorCode,
    path: &Path,
    message: impl Into<String>,
) {
    errors.push(SkillLoadError {
        code,
        path: path.to_path_buf(),
        message: message.into(),
    });
}

fn truncate_text(value: &str, limit: usize) -> String {
    let mut result = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        result.push('\u{2026}');
    }
    result
}

// ---------------------------------------------------------------------------
// 页面内 Skill 管理（第 53 章）：创建 / 更新 / 复制 / 删除，写入用户目录。

/// 页面编辑器的结构化输入；`tools` / `knowledge_base` 作为元数据写入
/// frontmatter，正文 `body` 即 Agent 的工作流程提示。
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDraft {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub knowledge_base: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    pub body: String,
}

const MAX_SKILL_BODY_CHARS: usize = 60_000;
const MAX_SKILL_DESCRIPTION_CHARS: usize = 300;

pub fn user_skills_root() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".suna").join("skills"))
        .ok_or_else(|| "home directory is unavailable".to_string())
}

fn validate_draft(draft: &SkillDraft) -> Result<(), String> {
    let name = draft.name.trim();
    if !valid_name(name) {
        return Err("Skill 名称只能包含字母、数字、点、横线或下划线（≤96 字符）".to_string());
    }
    let description = draft.description.trim();
    if description.is_empty() || description.chars().count() > MAX_SKILL_DESCRIPTION_CHARS {
        return Err("Skill 描述不能为空且不超过 300 字符".to_string());
    }
    if !draft.body.trim().is_empty() && draft.body.chars().count() > MAX_SKILL_BODY_CHARS {
        return Err("Skill 正文过长".to_string());
    }
    if let Some(version) = &draft.version {
        if !valid_version(version) {
            return Err("Skill 版本必须是 major.minor.patch".to_string());
        }
    }
    for tag in &draft.tags {
        if !valid_name(tag) {
            return Err(format!(
                "Skill 标签 {tag} 只能使用字母、数字、点、横线或下划线"
            ));
        }
    }
    for tool in &draft.tools {
        if !valid_name(tool) {
            return Err(format!("工具标识 {tool} 无效"));
        }
    }
    if let Some(kb) = &draft.knowledge_base {
        if kb.trim().is_empty() {
            return Err("knowledge_base 不能为空字符串".to_string());
        }
    }
    if let Some(agent_id) = &draft.agent_id {
        if agent_id.trim().is_empty() {
            return Err("agent_id 不能为空字符串".to_string());
        }
    }
    if let Some(model_id) = &draft.model_id {
        if model_id.trim().is_empty() {
            return Err("model_id 不能为空字符串".to_string());
        }
    }
    Ok(())
}

fn render_skill_markdown(name: &str, draft: &SkillDraft) -> String {
    // 用 Unicode 转义表示换行，避免源码里的转义歧义。
    const LF: char = '\u{0A}';
    const CR: char = '\u{0D}';
    let version = draft.version.clone().unwrap_or_else(|| "1.0.0".to_string());
    let description = draft.description.trim().replace([CR, LF], " ");
    let mut fields: Vec<String> = vec![
        String::from("---"),
        format!("name: {name}"),
        format!("description: {description}"),
        format!("version: {version}"),
    ];
    if !draft.tags.is_empty() {
        fields.push(format!("tags: [{}]", draft.tags.join(", ")));
    }
    if !draft.tools.is_empty() {
        fields.push(format!("tools: [{}]", draft.tools.join(", ")));
    }
    if let Some(kb) = draft
        .knowledge_base
        .as_deref()
        .map(str::trim)
        .filter(|kb| !kb.is_empty())
    {
        fields.push(format!("knowledge_base: {kb}"));
    }
    if let Some(agent_id) = draft
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        fields.push(format!("agent_id: {agent_id}"));
    }
    if let Some(model_id) = draft
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        fields.push(format!("model_id: {model_id}"));
    }
    fields.push(String::new());
    fields.push(draft.body.trim().to_string());
    let mut markdown = fields.join(&LF.to_string());
    markdown.push(LF);
    markdown
}

fn write_user_skill(root: &Path, name: &str, markdown: &str) -> Result<(), String> {
    let skill_dir = root.join(name);
    if markdown.len() > MAX_SKILL_BYTES {
        return Err("Skill 文件超过大小上限".to_string());
    }
    fs::create_dir_all(&skill_dir).map_err(|error| error.to_string())?;
    fs::write(skill_dir.join("SKILL.md"), markdown).map_err(|error| error.to_string())
}

fn require_user_skill(report: &SkillDiscoveryReport, name: &str) -> Result<SkillRecord, String> {
    report
        .skills
        .iter()
        .find(|skill| skill.name == name)
        .cloned()
        .ok_or_else(|| "skill not found".to_string())
}

pub fn create_skill(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    draft: SkillDraft,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    validate_draft(&draft)?;
    let name = draft.name.trim().to_string();
    let root = user_skills_root()?;
    let report = discover_skills(&default_skill_roots(), app_version);
    if report.skills.iter().any(|skill| skill.name == name) {
        return Err(format!("skill '{name}' already exists"));
    }
    write_user_skill(&root, &name, &render_skill_markdown(&name, &draft))?;
    catalog(connection, workspace_id, app_version)
}

pub fn update_skill(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    name: &str,
    draft: SkillDraft,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    validate_draft(&draft)?;
    let name = name.trim();
    if !valid_name(name) {
        return Err("skill name is invalid".to_string());
    }
    let root = user_skills_root()?;
    let report = discover_skills(&default_skill_roots(), app_version);
    let record = require_user_skill(&report, name)?;
    if record.source.scope != SkillScope::User {
        return Err("只有用户目录中的 Skill 支持编辑".to_string());
    }
    write_user_skill(&root, name, &render_skill_markdown(name, &draft))?;
    catalog(connection, workspace_id, app_version)
}

pub fn duplicate_skill(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    name: &str,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    let name = name.trim();
    let root = user_skills_root()?;
    let report = discover_skills(&default_skill_roots(), app_version);
    let record = require_user_skill(&report, name)?;
    let existing: BTreeSet<String> = report
        .skills
        .iter()
        .map(|skill| skill.name.clone())
        .collect();
    let mut candidate = format!("{name}-copy");
    let mut suffix = 2;
    while existing.contains(&candidate) {
        candidate = format!("{name}-copy{suffix}");
        suffix += 1;
    }
    let draft = SkillDraft {
        name: candidate.clone(),
        description: record.description.clone(),
        version: Some(record.version.clone()),
        tags: record.tags.clone(),
        tools: Vec::new(),
        knowledge_base: None,
        agent_id: None,
        model_id: None,
        body: record.body.clone(),
    };
    write_user_skill(
        &root,
        &candidate,
        &render_skill_markdown(&candidate, &draft),
    )?;
    catalog(connection, workspace_id, app_version)
}

pub fn delete_skill(
    connection: &mut rusqlite::Connection,
    workspace_id: &str,
    name: &str,
    app_version: &str,
) -> Result<SkillCatalog, String> {
    let name = name.trim();
    if !valid_name(name) {
        return Err("skill name is invalid".to_string());
    }
    let root = user_skills_root()?;
    let report = discover_skills(&default_skill_roots(), app_version);
    let record = require_user_skill(&report, name)?;
    if record.source.scope != SkillScope::User {
        return Err("只有用户目录中的 Skill 支持删除".to_string());
    }
    // 双重防护：目录名必须等于 Skill 名，且位于用户 Skill 根之下。
    let skill_dir = root.join(name);
    if !skill_dir.is_dir() || skill_dir.parent() != Some(root.as_path()) {
        return Err("skill directory is outside the user skill root".to_string());
    }
    fs::remove_dir_all(&skill_dir).map_err(|error| error.to_string())?;
    let mut names = load_enabled_names(connection, workspace_id)?;
    names.remove(name);
    save_enabled_names(connection, workspace_id, &names)?;
    catalog(connection, workspace_id, app_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skill_roots_only_include_the_suna_user_directory() {
        let home = dirs::home_dir().expect("home directory");
        let roots = default_skill_roots();

        assert_eq!(
            roots,
            [SkillRoot::new(
                SkillScope::User,
                home.join(".suna").join("skills"),
            )]
        );
    }

    #[test]
    fn enabling_a_new_skill_respects_the_runtime_limit() {
        let names = (0..MAX_ENABLED_SKILLS)
            .map(|index| format!("skill-{index}"))
            .collect::<BTreeSet<_>>();

        let error = validate_enabled_limit(&names, "new-skill")
            .expect_err("the skill limit must be enforced");
        assert!(error.contains("最多同时启用"));
    }

    #[test]
    fn re_enabling_an_existing_skill_is_idempotent_at_the_limit() {
        let names = (0..MAX_ENABLED_SKILLS)
            .map(|index| format!("skill-{index}"))
            .collect::<BTreeSet<_>>();

        validate_enabled_limit(&names, "skill-0")
            .expect("an already enabled skill may remain enabled");
    }
}
