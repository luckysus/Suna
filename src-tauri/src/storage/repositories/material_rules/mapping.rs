use super::*;

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
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    projects::require_active(c, ws, &input.project_id)?;
    let registry = registry(c, ws, Some(&input.project_id))?;
    let d = crate::storage::repositories::steel::get(c, ws, &input.dataset_id)?
        .ok_or("dataset_not_found")?;
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
        mapping.push(crate::storage::repositories::steel::DatasetColumnMapping {
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
    let roots = crate::permissions::path::AuthorizedRoots::new(vec![root.to_path_buf()])
        .map_err(|e| e.to_string())?;
    let authorized = roots
        .authorize(&directory.join(format!("规范化-{}.csv", Uuid::new_v4())))
        .map_err(|e| e.to_string())?;
    let path = authorized.local_path();
    let result = (|| {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        roots
            .verify_opened_file(&authorized, &file)
            .map_err(|e| e.to_string())?;
        roots
            .verify_target(&authorized, &path)
            .map_err(|e| e.to_string())?;
        let body = crate::steel::to_csv(&table.headers, &table.rows);
        file.write_all(body.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        roots
            .verify_opened_file(&authorized, &file)
            .map_err(|e| e.to_string())?;
        drop(file);
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })?;
        let hash = crate::steel::hash_dataset_source(&source)?;
        artifacts::atomic(c, || {
            let output = crate::storage::repositories::steel::save_new_preview_scoped(
                c,
                ws,
                &source,
                &hash,
                &preview,
                &mapping,
                &input.project_id,
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
    let dataset = crate::storage::repositories::steel::get(c, ws, &request.dataset_id)?
        .ok_or("rule_dataset_not_found")?;
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
    for key in ["recommendations", "pareto_front", "points"] {
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
