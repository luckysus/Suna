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
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub ordinal: usize,
    pub field_id: String,
    pub source_unit: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizeRequest {
    pub project_id: String,
    pub dataset_id: String,
    pub mappings: Vec<ColumnMapping>,
}
/// Explicit user-confirmed normalization writes a new managed CSV and immutable mapping version.
pub fn normalize_dataset(
    c: &Connection,
    ws: &str,
    root: &std::path::Path,
    input: NormalizeRequest,
) -> Result<super::steel::SteelDatasetRecord, String> {
    projects::require_active(c, ws, &input.project_id)?;
    let registry = registry(c, ws, Some(&input.project_id))?;
    let d = super::steel::get(c, ws, &input.dataset_id)?.ok_or("dataset_not_found")?;
    if crate::steel::hash_dataset_source(&d.source_path)? != d.source_sha256 {
        return Err("dataset_source_changed".into());
    }
    let mut table = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
        source_path: d.source_path.clone(),
        sheet: Some(d.selected_sheet.clone()),
    })?;
    if table.truncated {
        return Err("cannot_normalize_truncated_dataset".into());
    }
    let mut seen = std::collections::HashSet::new();
    let mut mapping = vec![];
    let mut traces = vec![];
    for m in &input.mappings {
        if !seen.insert(m.ordinal) || m.ordinal >= table.headers.len() {
            return Err("invalid_material_mapping_ordinal".into());
        }
        let f = registry
            .fields
            .iter()
            .find(|f| f.id == m.field_id)
            .ok_or("unknown_material_field")?;
        if f.value_type != "float" {
            return Err("normalize_numeric_fields_only".into());
        }
        let conversion = materials::conversion(&m.source_unit, &f.canonical_unit)?;
        for row in &mut table.rows {
            if row[m.ordinal].trim().is_empty() {
                continue;
            }
            let raw = row[m.ordinal]
                .parse::<f64>()
                .map_err(|_| "non_numeric_material_column")?;
            let normalized = materials::convert(raw, &m.source_unit, &f.canonical_unit)?;
            if f.min.is_some_and(|v| normalized < v) || f.max.is_some_and(|v| normalized > v) {
                return Err("material_value_outside_unit_legal_domain".into());
            }
            row[m.ordinal] = normalized.to_string();
        }
        traces.push(json!({"source_name":table.headers[m.ordinal],"source_unit":m.source_unit,"field_id":m.field_id,"normalized_unit":f.canonical_unit,"schema_version":registry.schema_version,"conversion_rule":{"scale":conversion.0,"shift":conversion.1}}));
        mapping.push(super::steel::DatasetColumnMapping {
            ordinal: m.ordinal,
            canonical_field: Some(m.field_id.clone()),
            unit: Some(f.canonical_unit.clone()),
        });
    }
    if mapping.is_empty() {
        return Err("select_explicit_material_mappings".into());
    }
    let directory = root.join("material-datasets");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let canonical_root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let canonical_dir = std::fs::canonicalize(directory).map_err(|e| e.to_string())?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err("material_directory_outside_data_root".into());
    }
    let path = canonical_dir.join(format!("规范化-{}.csv", Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        let body = crate::steel::to_csv(&table.headers, &table.rows);
        file.write_all(body.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })?;
        let hash = crate::steel::hash_dataset_source(&source)?;
        artifacts::atomic(c, || {
            let output = super::steel::save_preview_on_connection(
                c, ws, &source, &hash, &preview, &mapping,
            )?;
            projects::attach_in_transaction(
                c,
                ws,
                &input.project_id,
                "dataset",
                &d.id,
                "attached",
            )?;
            projects::attach_in_transaction(
                c,
                ws,
                &input.project_id,
                "dataset",
                &output.id,
                "derived_from",
            )?;
            c.execute(
                "INSERT INTO material_mapping_versions VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    ws,
                    Uuid::new_v4().to_string(),
                    input.project_id,
                    d.id,
                    output.id,
                    registry.schema_version,
                    json!(traces).to_string(),
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(err)?;
            if let (Some(a), Some(b)) = (
                artifacts::dataset_snapshot(c, ws, &input.project_id, &d.id)?,
                artifacts::dataset_snapshot(c, ws, &input.project_id, &output.id)?,
            ) {
                artifacts::edge(c, ws, &a, &b, "supersedes")?;
            }
            Ok(output)
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

pub fn optimization_rules(
    c: &Connection,
    ws: &str,
    request: &mut crate::app::compute_commands::logic::OptimizeSteelProcessRequest,
) -> Result<Option<RuleSnapshot>, String> {
    let Some(p) = artifacts::current(c, ws)? else {
        return Ok(None);
    };
    let enabled: Vec<String> = list(c, ws, &p)?
        .into_iter()
        .filter(|r| r.rule.enabled)
        .map(|r| r.rule.id)
        .collect();
    if enabled.is_empty() {
        return Ok(None);
    }
    let dataset = super::steel::get(c, ws, &request.dataset_id)?.ok_or("rule_dataset_not_found")?;
    let mut variables = vec![];
    for variable in &request.variables {
        let Some(field) = variable["fieldId"].as_str() else {
            continue;
        };
        let name = variable["name"].as_str().ok_or("variable_name_missing")?;
        let unit = variable["unit"]
            .as_str()
            .ok_or("material_variable_unit_missing")?;
        if let Some(column) = dataset.columns.iter().find(|column| {
            column.canonical_field.as_deref() == Some(name) || column.original_name == name
        }) {
            if let Some(original) = &column.unit {
                let (s, b) = materials::conversion(unit, original)?;
                if s != 1. || b != 0. {
                    return Err("optimization_variable_unit_must_match_trained_dataset: normalize a new version before training".into());
                }
            }
        }
        variables.push(Variable {
            name: name.into(),
            field_id: field.into(),
            unit: unit.into(),
            min: variable["min"].as_f64().ok_or("variable_min_missing")?,
            max: variable["max"].as_f64().ok_or("variable_max_missing")?,
            fixed_value: variable["fixedValue"].as_f64(),
        });
    }
    let snapshot = compile(c, ws, &p, variables, &enabled)?;
    request
        .constraints
        .extend(materials::worker_constraints(&snapshot.compiled));
    Ok(Some(snapshot))
}
pub fn annotate_result(result: &mut Value, payload: &Value) -> Result<(), String> {
    let Some(raw) = payload.get("material_rule_snapshot") else {
        return Ok(());
    };
    let snapshot: RuleSnapshot = serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
    for key in ["recommendations", "pareto_front"] {
        if let Some(points) = result[key].as_array_mut() {
            for point in points {
                let values: BTreeMap<String, f64> =
                    serde_json::from_value(point["values"].clone()).map_err(|e| e.to_string())?;
                let assessments = materials::assess(&snapshot.compiled, &values)?;
                if assessments
                    .iter()
                    .any(|r| r.severity == "hard" && !r.satisfied)
                {
                    return Err("worker_candidate_violates_frozen_hard_rule".into());
                }
                point["material_assessment"] = json!({"hardFeasible":true,"supportAssessed":assessments.iter().any(|r|r.severity=="support"),"softPenalty":assessments.iter().map(|r|r.penalty).sum::<f64>(),"supportRisk":assessments.iter().any(|r|r.severity=="support"&&!r.satisfied),"rules":assessments});
            }
        }
    }
    result["material_rule_snapshot"] = raw.clone();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::research_projects::SaveResearchProject;
    use super::*;
    fn fixture() -> (Connection, String) {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            SaveResearchProject {
                id: None,
                name: "合成材料项目".into(),
                description: "".into(),
                material_system: Some("wear-resistant-steel".into()),
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        (c, p.id)
    }
    fn rule() -> Rule {
        serde_json::from_value(json!({"id":"equipment","name":"合成设备下限","severity":"hard","enabled":true,"penalty":1,"provenance":{"type":"equipment","sourceRef":"test-device","sourceVersion":"1","reviewed":true},"expression":{"kind":"range","fieldId":"process.quench_temperature","unit":"degC","min":850}})).unwrap()
    }
    #[test]
    fn rule_versions_are_scoped_immutable_and_frozen() {
        let (c, p) = fixture();
        save(&c, "ws", &p, rule(), 0).unwrap();
        let v = Variable {
            name: "temperature".into(),
            field_id: "process.quench_temperature".into(),
            unit: "K".into(),
            min: 1073.15,
            max: 1273.15,
            fixed_value: None,
        };
        let frozen = compile(&c, "ws", &p, vec![v], &["equipment".into()]).unwrap();
        let mut changed = rule();
        changed.enabled = false;
        save(&c, "ws", &p, changed.clone(), 1).unwrap();
        assert_eq!(frozen.rules[0].version, 1);
        assert!(frozen.rules[0].rule.enabled);
        assert!(!list(&c, "ws", &p).unwrap()[0].rule.enabled);
        assert!(save(&c, "ws", &p, changed, 1).is_err());
        assert!(list(&c, "other", &p).is_err());
        assert!(c.execute("DELETE FROM material_rule_versions", []).is_err());
    }
    #[test]
    fn explicit_schema_overrides_and_ambiguous_hardness() {
        let (c, p) = fixture();
        let mut field = materials::base().fields[0].clone();
        field.aliases.push("用户碳含量".into());
        assert!(save_overlay(&c, "ws", &p, vec![field.clone()], vec![]).is_err());
        save_overlay(&c, "ws", &p, vec![field.clone()], vec![field.id.clone()]).unwrap();
        assert_eq!(
            materials::suggest(&registry(&c, "ws", Some(&p)).unwrap(), "用户碳含量").len(),
            1
        );
    }
    #[test]
    fn normalization_writes_new_file_and_mapping_without_touching_original() {
        let (c, p) = fixture();
        let root = std::env::temp_dir().join(format!("suna-material-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("原始.csv");
        std::fs::write(&path, "C,temperature\n0.004,1173.15\n0.005,1273.15\n").unwrap();
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })
        .unwrap();
        let hash = crate::steel::hash_dataset_source(&source).unwrap();
        let original = super::super::steel::save_preview_on_connection(
            &c,
            "ws",
            &source,
            &hash,
            &preview,
            &[],
        )
        .unwrap();
        let normalized = normalize_dataset(
            &c,
            "ws",
            &root,
            NormalizeRequest {
                project_id: p.clone(),
                dataset_id: original.id.clone(),
                mappings: vec![
                    ColumnMapping {
                        ordinal: 0,
                        field_id: "chem.c".into(),
                        source_unit: "fraction".into(),
                    },
                    ColumnMapping {
                        ordinal: 1,
                        field_id: "process.quench_temperature".into(),
                        source_unit: "K".into(),
                    },
                ],
            },
        )
        .unwrap();
        assert_ne!(original.id, normalized.id);
        assert_eq!(crate::steel::hash_dataset_source(&source).unwrap(), hash);
        assert_eq!(
            normalized.columns[0].canonical_field.as_deref(),
            Some("chem.c")
        );
        assert_eq!(normalized.columns[0].min, Some(0.4));
        assert!((normalized.columns[1].min.unwrap() - 900.).abs() < 1e-9);
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM material_mapping_versions", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
