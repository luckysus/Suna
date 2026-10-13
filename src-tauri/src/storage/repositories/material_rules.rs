use super::{research_artifacts as artifacts, research_projects as projects};
use crate::steel::materials::{self, Compiled, Field, Registry, Rule, Variable};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
pub fn registry(c: &Connection, ws: &str, p: Option<&str>) -> Result<Registry, String> {
    let mut base = materials::base();
    // Reuse installed, hash-verified domain package resources. Package overrides
    // must be declared explicitly; project revisions then form the final layer.
    for package in super::domains::list(c, ws)?
        .into_iter()
        .filter(|p| p.active && p.id != "steel")
    {
        let loaded = crate::domains::load_package(
            std::path::Path::new(&package.path),
            env!("CARGO_PKG_VERSION"),
        )
        .map_err(|e| e.to_string())?;
        if crate::domains::compute_package_digest(std::path::Path::new(&package.path))
            .map_err(|e| e.to_string())?
            != package.package_sha256
        {
            return Err("material_domain_package_hash_changed".into());
        }
        for asset in loaded
            .assets
            .iter()
            .filter(|a| a.spec.path == "assets/data-mappings.json")
        {
            let body: Value =
                serde_json::from_slice(&std::fs::read(&asset.path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if let Some(schema) = body.get("materials_schema") {
                let fields: Vec<Field> =
                    serde_json::from_value(schema["fields"].clone()).map_err(|e| e.to_string())?;
                let overrides: Vec<String> =
                    serde_json::from_value(schema.get("overrides").cloned().unwrap_or(json!([])))
                        .map_err(|e| e.to_string())?;
                base = materials::overlay(base, fields, &overrides)?;
            }
        }
    }

    if let Some(p) = p {
        let project = projects::get(c, ws, p)?;
        if let Some(system) = project.material_system {
            base.material_system = system;
        }
        let mut q=c.prepare("SELECT fields_json,overrides_json FROM material_schema_overlays WHERE workspace_id=?1 AND project_id=?2 ORDER BY version").map_err(err)?;
        let rows = q
            .query_map(params![ws, p], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        for (fields, overrides) in rows {
            base = materials::overlay(
                base,
                serde_json::from_str(&fields).map_err(|e| e.to_string())?,
                &serde_json::from_str::<Vec<String>>(&overrides).map_err(|e| e.to_string())?,
            )?;
        }
    }
    Ok(base)
}
pub fn save_overlay(
    c: &Connection,
    ws: &str,
    p: &str,
    fields: Vec<Field>,
    overrides: Vec<String>,
) -> Result<u32, String> {
    artifacts::atomic(c, || {
        projects::require_active(c, ws, p)?;
        materials::overlay(registry(c, ws, Some(p))?, fields.clone(), &overrides)?;
        let version:u32=c.query_row("SELECT COALESCE(MAX(version),0)+1 FROM material_schema_overlays WHERE workspace_id=?1 AND project_id=?2",params![ws,p],|r|r.get(0)).map_err(err)?;
        c.execute(
            "INSERT INTO material_schema_overlays VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                ws,
                p,
                version,
                json!(fields).to_string(),
                json!(overrides).to_string(),
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(err)?;
        projects::audit(
            c,
            ws,
            p,
            "material_schema_revised",
            json!({"version":version}),
        )?;
        Ok(version)
    })
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VersionedRule {
    pub version: u32,
    pub rule: Rule,
}
pub fn list(c: &Connection, ws: &str, p: &str) -> Result<Vec<VersionedRule>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT version,body_json FROM material_rule_versions r WHERE workspace_id=?1 AND project_id=?2 AND version=(SELECT MAX(version) FROM material_rule_versions x WHERE x.workspace_id=r.workspace_id AND x.project_id=r.project_id AND x.id=r.id) ORDER BY id LIMIT 128").map_err(err)?;
    let rows = q
        .query_map(params![ws, p], |r| {
            Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    rows.into_iter()
        .map(|(version, s)| {
            Ok(VersionedRule {
                version,
                rule: serde_json::from_str(&s).map_err(|e| e.to_string())?,
            })
        })
        .collect()
}
pub fn save(
    c: &Connection,
    ws: &str,
    p: &str,
    rule: Rule,
    expected_version: u32,
) -> Result<VersionedRule, String> {
    artifacts::atomic(c, || {
        projects::require_active(c, ws, p)?;
        if rule.id.is_empty()
            || rule.id.len() > 128
            || rule.name.is_empty()
            || rule.name.len() > 300
            || !matches!(rule.severity.as_str(), "hard" | "soft" | "support")
            || !rule.penalty.is_finite()
            || rule.penalty <= 0.
            || rule.provenance.source_ref.len() > 1000
            || rule.provenance.source_version.len() > 200
            || !matches!(
                rule.provenance.r#type.as_str(),
                "standard" | "user" | "literature" | "equipment" | "dataset"
            )
            || rule.provenance.source_ref.starts_with(['/', '\\'])
            || rule.provenance.source_ref.contains(":\\")
        {
            return Err("invalid_rule_metadata".into());
        }
        let registry = registry(c, ws, Some(p))?;
        match &rule.expression {
            materials::Expression::Range {
                field_id,
                unit,
                min,
                max,
            } => {
                let field = registry
                    .fields
                    .iter()
                    .find(|f| &f.id == field_id)
                    .ok_or("unknown_material_field")?;
                materials::conversion(unit, &field.canonical_unit)?;
                if min.is_none() && max.is_none()
                    || min.zip(*max).is_some_and(|(a, b)| a > b)
                    || min.is_some_and(|v| !v.is_finite())
                    || max.is_some_and(|v| !v.is_finite())
                {
                    return Err("invalid_rule_range".into());
                }
            }
            materials::Expression::Linear {
                coefficients,
                units,
                operator,
                value,
            } => {
                if coefficients.is_empty()
                    || coefficients.len() > 64
                    || units.len() != coefficients.len()
                    || !value.is_finite()
                    || !matches!(operator.as_str(), "ge" | "le" | "eq")
                {
                    return Err("invalid_linear_rule".into());
                }
                for (id, value) in coefficients {
                    let field = registry
                        .fields
                        .iter()
                        .find(|f| &f.id == id)
                        .ok_or("unknown_material_field")?;
                    materials::conversion(
                        units.get(id).ok_or("rule_unit_missing")?,
                        &field.canonical_unit,
                    )?;
                    if !value.is_finite() {
                        return Err("nonfinite_rule_coefficient".into());
                    }
                }
            }
        }
        if rule.provenance.reviewed && rule.provenance.source_ref.trim().is_empty() {
            return Err("reviewed_rule_requires_source".into());
        }
        if rule.provenance.r#type == "dataset" {
            projects::target_title(c, ws, "dataset", &rule.provenance.source_ref)?
                .ok_or("rule_dataset_not_in_workspace")?;
        }
        if rule.provenance.r#type == "literature" {
            let saved = super::settings::get(c, ws, "suna.literature.evidence")?
                .ok_or("rule_evidence_missing")?;
            let records: Value = serde_json::from_str(&saved).map_err(|e| e.to_string())?;
            let record = records
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .find(|r| r["id"].as_str() == Some(rule.provenance.source_ref.as_str()))
                })
                .filter(|r| r["reviewed"] == true)
                .ok_or("rule_evidence_requires_original_review")?;
            let document = record["documentId"]
                .as_str()
                .ok_or("rule_evidence_document_missing")?;
            projects::target_title(c, ws, "document", document)?
                .ok_or("rule_evidence_document_unavailable")?;
        }

        let current:u32=c.query_row("SELECT COALESCE(MAX(version),0) FROM material_rule_versions WHERE workspace_id=?1 AND project_id=?2 AND id=?3",params![ws,p,rule.id],|r|r.get(0)).map_err(err)?;
        if current != expected_version {
            return Err("material_rule_changed".into());
        }
        if current == 0 && list(c, ws, p)?.len() >= 128 {
            return Err("material_rule_limit".into());
        }
        let version = current + 1;
        c.execute(
            "INSERT INTO material_rule_versions VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                ws,
                p,
                rule.id,
                version,
                json!(rule).to_string(),
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(err)?;
        projects::audit(
            c,
            ws,
            p,
            "material_rule_revised",
            json!({"rule_id":rule.id,"version":version}),
        )?;
        Ok(VersionedRule { version, rule })
    })
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuleSnapshot {
    pub schema_version: String,
    pub project_id: String,
    pub rule_set_hash: String,
    pub rules: Vec<VersionedRule>,
    pub compiled: Vec<Compiled>,
    pub variables: Vec<Variable>,
}
pub fn compile(
    c: &Connection,
    ws: &str,
    p: &str,
    variables: Vec<Variable>,
    selected: &[String],
) -> Result<RuleSnapshot, String> {
    projects::require_active(c, ws, p)?;
    let registry = registry(c, ws, Some(p))?;
    let all = list(c, ws, p)?;
    let mut unique = std::collections::HashSet::new();
    let mut rules = vec![];
    for id in selected {
        if !unique.insert(id) {
            return Err("duplicate_material_rule_selection".into());
        }
        let rule = all
            .iter()
            .find(|r| &r.rule.id == id)
            .ok_or("material_rule_not_found")?;
        if !rule.rule.enabled {
            return Err("selected_material_rule_disabled".into());
        }
        rules.push(rule.clone());
    }
    let compiled = materials::compile(
        &registry,
        &variables,
        &rules.iter().map(|r| r.rule.clone()).collect::<Vec<_>>(),
    )?;
    let hash = format!(
        "{:x}",
        Sha256::digest(
            json!({"registry":registry,"rules":rules,"variables":variables})
                .to_string()
                .as_bytes()
        )
    );
    Ok(RuleSnapshot {
        schema_version: registry.schema_version,
        project_id: p.into(),
        rule_set_hash: hash,
        rules,
        compiled,
        variables,
    })
}
#[cfg(test)]
#[path = "material_rules/tests.rs"]
mod tests;

mod mapping;
pub use mapping::{
    annotate_result, normalize_dataset, optimization_rules, ColumnMapping, NormalizeRequest,
};
