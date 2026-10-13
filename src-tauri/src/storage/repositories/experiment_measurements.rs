use super::{experiments, research_artifacts as artifacts, research_projects as projects};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use uuid::Uuid;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementInput {
    pub property: String,
    pub value: f64,
    pub unit: String,
    pub method: String,
    pub uncertainty: Option<f64>,
    #[serde(default = "one")]
    pub repeat_count: u32,
    pub measured_at: String,
    #[serde(default)]
    pub note: String,
}
fn one() -> u32 {
    1
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordExecution {
    pub project_id: String,
    pub plan_id: String,
    pub sample_id: String,
    pub heat_id: Option<String>,
    pub batch_id: Option<String>,
    pub actual_variables: BTreeMap<String, f64>,
    pub variable_units: BTreeMap<String, String>,
    pub performed_at: String,
    pub method: String,
    pub source_type: String,
    #[serde(default)]
    pub note: String,
    pub measurements: Vec<MeasurementInput>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub id: String,
    #[serde(flatten)]
    pub data: MeasurementInput,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub state: String,
    pub reviewer: String,
    pub reason: String,
    pub created_at: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: String,
    pub project_id: String,
    pub plan_id: String,
    pub sample_id: String,
    pub heat_id: Option<String>,
    pub batch_id: Option<String>,
    pub replicate: u32,
    pub actual_variables: BTreeMap<String, f64>,
    pub variable_units: BTreeMap<String, String>,
    pub performed_at: String,
    pub method: String,
    pub source_type: String,
    pub note: String,
    pub review_state: String,
    pub plan_artifact_id: Option<String>,
    pub measurements: Vec<Measurement>,
    pub reviews: Vec<Review>,
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= max && !s.contains('\0')
}
fn timestamp(s: &str) -> Result<(), String> {
    DateTime::parse_from_rfc3339(s)
        .map(|_| ())
        .map_err(|_| "valid ISO8601 timestamp with timezone required".into())
}
fn validate(input: &RecordExecution) -> Result<(), String> {
    if !text(&input.sample_id, 160)
        || !text(&input.method, 300)
        || input.note.chars().count() > 4000
        || input.heat_id.as_ref().is_some_and(|s| !text(s, 160))
        || input.batch_id.as_ref().is_some_and(|s| !text(s, 160))
        || !matches!(
            input.source_type.as_str(),
            "measured" | "synthetic" | "literature"
        )
    {
        return Err("invalid_execution_metadata".into());
    }
    timestamp(&input.performed_at)?;
    if input.variable_units.len() != input.actual_variables.len()
        || input
            .actual_variables
            .keys()
            .any(|k| !input.variable_units.get(k).is_some_and(|u| text(u, 64)))
    {
        return Err("units for each actual variable required".into());
    }
    if input.actual_variables.is_empty()
        || input.actual_variables.len() > 64
        || input
            .actual_variables
            .iter()
            .any(|(k, v)| !text(k, 128) || !v.is_finite())
    {
        return Err("finite named actual variables required".into());
    }
    if input.measurements.is_empty() || input.measurements.len() > 32 {
        return Err("1-32 measurements required".into());
    }
    let mut names = std::collections::HashSet::new();
    for m in &input.measurements {
        if !text(&m.property, 128)
            || !names.insert(&m.property)
            || !m.value.is_finite()
            || !text(&m.unit, 64)
            || !text(&m.method, 300)
            || m.uncertainty.is_some_and(|v| !v.is_finite() || v < 0.)
            || !(1..=10000).contains(&m.repeat_count)
            || m.note.chars().count() > 4000
        {
            return Err("invalid_measurement_or_unit".into());
        }
        timestamp(&m.measured_at)?;
    }
    Ok(())
}
pub fn record(c: &Connection, ws: &str, input: RecordExecution) -> Result<Execution, String> {
    validate(&input)?;
    projects::require_active(c, ws, &input.project_id)?;
    let plan = experiments::get(c, ws, &input.plan_id)?.ok_or("experiment_plan_not_found")?;
    if plan.state != "accepted" {
        return Err("experiment_plan_requires_acceptance".into());
    }
    let linked:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 AND target_kind='experiment' AND target_id=?3)",params![ws,input.project_id,input.plan_id],|r|r.get(0)).map_err(err)?;
    if !linked {
        return Err("plan_not_linked_to_project".into());
    }
    artifacts::atomic(c, || {
        let id = Uuid::new_v4().to_string();
        let replicate:u32=c.query_row("SELECT COALESCE(MAX(replicate),0)+1 FROM experiment_executions WHERE workspace_id=?1 AND plan_id=?2",params![ws,input.plan_id],|r|r.get(0)).map_err(err)?;
        let plan_artifact:Option<String>=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='experiment_plan' AND source_key=?3",params![ws,input.project_id,format!("plan:{}",input.plan_id)],|r|r.get(0)).optional().map_err(err)?;
        c.execute("INSERT INTO experiment_executions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",params![ws,id,input.project_id,input.plan_id,plan_artifact,input.sample_id,input.heat_id,input.batch_id,replicate,json!(input.actual_variables).to_string(),json!(input.variable_units).to_string(),input.performed_at,input.method,input.source_type,input.note,Utc::now().to_rfc3339()]).map_err(err)?;
        for m in &input.measurements {
            c.execute(
                "INSERT INTO experiment_measurements VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    ws,
                    Uuid::new_v4().to_string(),
                    id,
                    m.property,
                    m.value,
                    m.unit,
                    m.method,
                    m.uncertainty,
                    m.repeat_count,
                    m.measured_at,
                    m.note
                ],
            )
            .map_err(err)?;
        }
        append_review(c, ws, &id, "draft", "")?;
        projects::audit(
            c,
            ws,
            &input.project_id,
            "execution_recorded",
            json!({"execution_id":id,"plan_id":input.plan_id,"source_type":input.source_type}),
        )?;
        get(c, ws, &id)
    })
}
pub fn get(c: &Connection, ws: &str, id: &str) -> Result<Execution, String> {
    let mut e=c.query_row("SELECT id,project_id,plan_id,sample_id,heat_id,batch_id,replicate,actual_variables_json,performed_at,method,source_type,note,plan_artifact_id,variable_units_json FROM experiment_executions WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|{let raw:String=r.get(7)?;Ok(Execution{id:r.get(0)?,project_id:r.get(1)?,plan_id:r.get(2)?,sample_id:r.get(3)?,heat_id:r.get(4)?,batch_id:r.get(5)?,replicate:r.get(6)?,actual_variables:serde_json::from_str(&raw).unwrap_or_default(),variable_units:serde_json::from_str(&r.get::<_,String>(13)?).unwrap_or_default(),performed_at:r.get(8)?,method:r.get(9)?,source_type:r.get(10)?,note:r.get(11)?,review_state:"draft".into(),plan_artifact_id:r.get(12)?,measurements:vec![],reviews:vec![]})}).optional().map_err(err)?.ok_or("experiment_execution_not_found")?;
    let mut q=c.prepare("SELECT id,property,value,unit,method,uncertainty,repeat_count,measured_at,note FROM experiment_measurements WHERE workspace_id=?1 AND execution_id=?2 ORDER BY property").map_err(err)?;
    e.measurements = q
        .query_map(params![ws, id], |r| {
            Ok(Measurement {
                id: r.get(0)?,
                data: MeasurementInput {
                    property: r.get(1)?,
                    value: r.get(2)?,
                    unit: r.get(3)?,
                    method: r.get(4)?,
                    uncertainty: r.get(5)?,
                    repeat_count: r.get(6)?,
                    measured_at: r.get(7)?,
                    note: r.get(8)?,
                },
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut q=c.prepare("SELECT state,reviewer,reason,created_at FROM experiment_reviews WHERE workspace_id=?1 AND execution_id=?2 ORDER BY sequence").map_err(err)?;
    e.reviews = q
        .query_map(params![ws, id], |r| {
            Ok(Review {
                state: r.get(0)?,
                reviewer: r.get(1)?,
                reason: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    e.review_state = e
        .reviews
        .last()
        .map(|r| r.state.clone())
        .unwrap_or("draft".into());
    Ok(e)
}
pub fn list(
    c: &Connection,
    ws: &str,
    p: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Execution>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT id FROM experiment_executions WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at DESC,id LIMIT ?3 OFFSET ?4").map_err(err)?;
    let ids = q
        .query_map(params![ws, p, limit.clamp(1, 100), offset], |r| {
            r.get::<_, String>(0)
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    ids.iter().map(|id| get(c, ws, id)).collect()
}
fn append_review(
    c: &Connection,
    ws: &str,
    id: &str,
    state: &str,
    reason: &str,
) -> Result<(), String> {
    c.execute("INSERT INTO experiment_reviews(workspace_id,id,execution_id,state,reviewer,reason,created_at) VALUES(?1,?2,?3,?4,'local_user',?5,?6)",params![ws,Uuid::new_v4().to_string(),id,state,reason,Utc::now().to_rfc3339()]).map_err(err)?;
    Ok(())
}
pub fn review(
    c: &Connection,
    ws: &str,
    id: &str,
    state: &str,
    reason: &str,
    expected_state: &str,
) -> Result<Execution, String> {
    artifacts::atomic(c, || {
        let e = get(c, ws, id)?;
        projects::require_active(c, ws, &e.project_id)?;
        if e.review_state != expected_state {
            return Err("experiment_review_changed".into());
        }
        if !matches!(
            (e.review_state.as_str(), state),
            ("draft" | "rejected", "submitted") | ("submitted", "accepted" | "rejected")
        ) || reason.chars().count() > 4000
            || (state == "rejected" && reason.trim().is_empty())
        {
            return Err("invalid_experiment_review_transition".into());
        }
        append_review(c, ws, id, state, reason)?;
        projects::audit(
            c,
            ws,
            &e.project_id,
            "execution_reviewed",
            json!({"execution_id":id,"state":state}),
        )?;
        if state == "accepted" {
            let a = artifacts::create(
                c,
                ws,
                &e.project_id,
                "experiment_measurement",
                "已审核实测",
                id,
                None,
                json!({"execution_id":id,"source_type":e.source_type,"sample_id":e.sample_id,"actual_variables":e.actual_variables,"variable_units":e.variable_units,"measurements":e.measurements,"reviewed_by":"local_user","reviewed_at":Utc::now().to_rfc3339(),"review_note":reason}),
            )?;
            if let Some(plan) = e.plan_artifact_id {
                artifacts::edge(c, ws, &plan, &a, "measured_against")?;
            }
        }
        get(c, ws, id)
    })
}
/// CSV rows remain separate for each execution; unreviewed/synthetic rows never silently enter training.
pub fn reviewed_table(
    c: &Connection,
    ws: &str,
    p: &str,
    ids: &[String],
    features: &[String],
    properties: &[String],
) -> Result<(Vec<String>, Vec<Vec<Option<String>>>, Vec<String>), String> {
    projects::require_active(c, ws, p)?;
    if ids.is_empty()
        || ids.len() > 5000
        || features.is_empty()
        || properties.is_empty()
        || features.len() > 64
        || properties.len() > 32
    {
        return Err("select reviewed executions, features and targets".into());
    }
    let mut all: Vec<String> = features.iter().chain(properties).cloned().collect();
    if all.iter().collect::<std::collections::HashSet<_>>().len() != all.len() {
        return Err("duplicate dataset fields".into());
    }
    let mut rows = vec![];
    let mut measurement_ids = vec![];
    let mut unique = std::collections::HashSet::new();
    let mut units: BTreeMap<String, String> = BTreeMap::new();
    for id in ids {
        if !unique.insert(id) {
            return Err("duplicate execution selection".into());
        }
        let e = get(c, ws, id)?;
        if e.project_id != p || e.review_state != "accepted" || e.source_type != "measured" {
            return Err(
                "only accepted real measurements in this project may enter training".into(),
            );
        }
        let mut row = vec![];
        for field in features {
            let unit = e.variable_units.get(field).ok_or("missing_variable_unit")?;
            if units.get(field).is_some_and(|old| old != unit) {
                return Err("mixed_variable_units_require_explicit_conversion".into());
            }
            units.insert(field.clone(), unit.clone());
            row.push(Some(
                e.actual_variables
                    .get(field)
                    .ok_or("missing_actual_variable")?
                    .to_string(),
            ))
        }
        for field in properties {
            let m = e
                .measurements
                .iter()
                .find(|m| &m.data.property == field)
                .ok_or("missing_measured_property")?;
            if let Some(unit) = units.get(field) {
                if unit != &m.data.unit {
                    return Err("mixed_measurement_units_require_explicit_conversion".into());
                }
            }
            units.insert(field.clone(), m.data.unit.clone());
            row.push(Some(m.data.value.to_string()));
            measurement_ids.push(m.id.clone());
        }
        row.extend([
            Some(e.sample_id),
            e.heat_id,
            e.batch_id,
            Some(e.id),
            Some(e.source_type),
        ]);
        rows.push(row);
    }
    all.extend(
        [
            "sample_id",
            "heat_id",
            "batch_id",
            "execution_id",
            "source_type",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    Ok((all, rows, measurement_ids))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub execution_id: String,
    pub property: String,
    pub unit: String,
    pub prediction: f64,
    pub actual: f64,
    pub prediction_minus_actual: f64,
}
pub fn compare(
    c: &Connection,
    ws: &str,
    id: &str,
    artifact_id: &str,
    candidate: usize,
) -> Result<Vec<Comparison>, String> {
    let e = get(c, ws, id)?;
    let a = artifacts::get(c, ws, artifact_id)?;
    if a.project_id != e.project_id || a.kind != "optimization_result" {
        return Err("invalid_comparison_source".into());
    }
    let payload = a.payload.ok_or("artifact_payload_missing")?;
    let result = &payload["result"];
    let point = result["recommendations"]
        .get(candidate)
        .ok_or("candidate_not_found")?;
    let specs = result["objective_specs"]
        .as_array()
        .or_else(|| payload["request"]["objective_specs"].as_array())
        .ok_or("objective_schema_missing")?;
    let values = point["objectives"]
        .as_array()
        .ok_or("prediction_values_missing")?;
    let mut comparisons = vec![];
    for (i, spec) in specs.iter().enumerate() {
        if let Some(m) = e
            .measurements
            .iter()
            .find(|m| Some(m.data.property.as_str()) == spec["name"].as_str())
        {
            if Some(m.data.unit.as_str()) != spec["unit"].as_str() {
                return Err("comparison_unit_mismatch".into());
            }
            let prediction = values
                .get(i)
                .and_then(Value::as_f64)
                .ok_or("prediction_missing")?;
            comparisons.push(Comparison {
                execution_id: e.id.clone(),
                property: m.data.property.clone(),
                unit: m.data.unit.clone(),
                prediction,
                actual: m.data.value,
                prediction_minus_actual: prediction - m.data.value,
            });
        }
    }
    Ok(comparisons)
}

pub fn snapshot_plan(c: &Connection, ws: &str, p: &str, id: &str) -> Result<String, String> {
    projects::require_active(c, ws, p)?;
    let plan = experiments::get(c, ws, id)?.ok_or("experiment_plan_not_found")?;
    c.execute("INSERT OR IGNORE INTO research_project_links VALUES(?1,?2,'experiment',?3,'attached',?4,'local_user')",params![ws,p,id,Utc::now().to_rfc3339()]).map_err(err)?;
    let recommendation: Value =
        serde_json::from_str(plan.recommendation_json.as_deref().unwrap_or("null"))
            .map_err(|e| e.to_string())?;
    let a = artifacts::create(
        c,
        ws,
        p,
        "experiment_plan",
        &plan.title,
        &format!("plan:{id}"),
        None,
        json!({"plan_id":id,"variables":serde_json::from_str::<Value>(&plan.variables_json).map_err(|e|e.to_string())?,"recommendation":artifacts::safe_metadata(&recommendation)}),
    )?;
    if let Some(source) = recommendation["optimization_artifact_id"].as_str() {
        let original = artifacts::get(c, ws, source)?;
        if original.project_id != p || original.kind != "optimization_result" {
            return Err("invalid_plan_optimization_source".into());
        }
        artifacts::edge(c, ws, source, &a, "recommends_experiment")?;
    }
    Ok(a)
}
pub fn candidate_plan(
    c: &Connection,
    ws: &str,
    p: &str,
    artifact_id: &str,
    index: usize,
    title: &str,
) -> Result<experiments::ExperimentRecord, String> {
    artifacts::atomic(c, || {
        projects::require_active(c, ws, p)?;
        let a = artifacts::get(c, ws, artifact_id)?;
        if a.project_id != p || a.kind != "optimization_result" {
            return Err("optimization_artifact_not_in_project".into());
        }
        let payload = a.payload.ok_or("artifact_payload_missing")?;
        let result = &payload["result"];
        let point = result["recommendations"]
            .get(index)
            .ok_or("candidate_not_found")?;
        let values = point["values"]
            .as_object()
            .ok_or("candidate_values_missing")?;
        let variables: Vec<Value> = values
            .iter()
            .map(|(name, value)| json!({"name":name,"value":value}))
            .collect();
        let vars = json!(variables).to_string();
        let rec=json!({"optimization_artifact_id":artifact_id,"candidate_index":index,"candidate":point,"objective_specs":result["objective_specs"],"rule_snapshot":payload["request"]["rule_snapshot"]}).to_string();
        let plan = experiments::create_on_connection(
            c,
            ws,
            experiments::NewExperiment {
                title,
                objective: Some("项目优化候选实验"),
                variables_json: &vars,
                recommendation_json: Some(&rec),
                state: "proposed",
            },
        )?;
        snapshot_plan(c, ws, p, &plan.id)?;
        Ok(plan)
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetVersionRequest {
    pub project_id: String,
    pub execution_ids: Vec<String>,
    pub features: Vec<String>,
    pub properties: Vec<String>,
    pub parent_dataset_id: Option<String>,
}
pub fn create_dataset_version(
    c: &Connection,
    ws: &str,
    root: &std::path::Path,
    request: DatasetVersionRequest,
) -> Result<super::steel::SteelDatasetRecord, String> {
    let (headers, new_rows, measurement_ids) = reviewed_table(
        c,
        ws,
        &request.project_id,
        &request.execution_ids,
        &request.features,
        &request.properties,
    )?;
    let first = get(c, ws, &request.execution_ids[0])?;
    let mut units = first.variable_units.clone();
    for m in &first.measurements {
        units.insert(m.data.property.clone(), m.data.unit.clone());
    }
    let mut rows: Vec<Vec<String>> = vec![];
    if let Some(parent) = &request.parent_dataset_id {
        let dataset = super::steel::get(c, ws, parent)?.ok_or("parent_dataset_not_found")?;
        let linked:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 AND target_kind='dataset' AND target_id=?3)",params![ws,request.project_id,parent],|r|r.get(0)).map_err(err)?;
        if !linked {
            return Err("parent_dataset_not_in_project".into());
        }
        if crate::steel::hash_dataset_source(&dataset.source_path)? != dataset.source_sha256 {
            return Err("parent_dataset_source_changed".into());
        }
        let table = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
            source_path: dataset.source_path,
            sheet: Some(dataset.selected_sheet),
        })?;
        if table.truncated {
            return Err("cannot_version_a_truncated_dataset".into());
        }
        let selected: Vec<usize> = request
            .features
            .iter()
            .chain(&request.properties)
            .map(|field| {
                let col = dataset
                    .columns
                    .iter()
                    .find(|column| {
                        column.canonical_field.as_deref() == Some(field.as_str())
                            || column.original_name == *field
                    })
                    .ok_or("parent_field_not_found")?;
                if col.unit.as_ref() != units.get(field) {
                    return Err("parent_dataset_unit_mismatch");
                }
                Ok(col.ordinal)
            })
            .collect::<Result<_, &str>>()?;
        for source in table.rows {
            let mut row: Vec<String> = selected
                .iter()
                .map(|i| source.get(*i).cloned().unwrap_or_default())
                .collect();
            for name in [
                "sample_id",
                "heat_id",
                "batch_id",
                "execution_id",
                "source_type",
            ] {
                row.push(
                    table
                        .headers
                        .iter()
                        .position(|h| h == name)
                        .and_then(|i| source.get(i))
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            rows.push(row);
        }
    }
    rows.extend(
        new_rows
            .into_iter()
            .map(|r| r.into_iter().map(Option::unwrap_or_default).collect()),
    );
    let bytes = crate::steel::to_csv(&headers, &rows).into_bytes();
    let directory = root.join("research-datasets");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let canonical_root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let canonical_dir = std::fs::canonicalize(&directory).map_err(|e| e.to_string())?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err("managed_dataset_directory_outside_root".into());
    }
    let path = canonical_dir.join(format!("实验测量-{}.csv", Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let roots = crate::permissions::path::AuthorizedRoots::new(vec![canonical_root])
        .map_err(|e| e.to_string())?;
    let authorized = roots.authorize(&path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    roots.verify_opened_file(&authorized, &file).map_err(|e|e.to_string())?;
    roots.verify_target(&authorized,&path).map_err(|e|e.to_string())?;
    let result = (|| {
        use std::io::Write;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let source_path = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source_path.clone(),
            sheet: None,
        })?;
        let sha = crate::steel::hash_dataset_source(&source_path)?;
        let mappings: Vec<super::steel::DatasetColumnMapping> = headers
            .iter()
            .enumerate()
            .map(|(ordinal, name)| super::steel::DatasetColumnMapping {
                ordinal,
                canonical_field: Some(if name.bytes().all(|b| b.is_ascii_alphanumeric()||b==b'_'){name.to_ascii_lowercase()}else{format!("measurement_column_{ordinal}")}),
                unit: units.get(name).cloned(),
            })
            .collect();
        artifacts::atomic(c, || {
            projects::require_active(c, ws, &request.project_id)?;
            let dataset = super::steel::save_preview_on_connection(
                c,
                ws,
                &source_path,
                &sha,
                &preview,
                &mappings,
            )?;
            projects::attach_in_transaction(
                c,
                ws,
                &request.project_id,
                "dataset",
                &dataset.id,
                "derived_from",
            )?;
            c.execute(
                "INSERT INTO research_dataset_versions VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    ws,
                    Uuid::new_v4().to_string(),
                    request.project_id,
                    dataset.id,
                    request.parent_dataset_id,
                    json!(measurement_ids).to_string(),
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(err)?;
            let snapshot = artifacts::dataset_snapshot(c, ws, &request.project_id, &dataset.id)?
                .ok_or("dataset_snapshot_missing")?;
            for execution in &request.execution_ids {
                let a:String=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='experiment_measurement' AND source_key=?3",params![ws,request.project_id,execution],|r|r.get(0)).map_err(err)?;
                artifacts::edge(c, ws, &a, &snapshot, "uses_dataset")?;
            }
            if let Some(parent) = &request.parent_dataset_id {
                if let Some(previous) =
                    artifacts::dataset_snapshot(c, ws, &request.project_id, parent)?
                {
                    artifacts::edge(c, ws, &previous, &snapshot, "supersedes")?;
                }
            }
            projects::audit(
                c,
                ws,
                &request.project_id,
                "measurement_dataset_created",
                json!({"dataset_id":dataset.id,"measurement_count":measurement_ids.len(),"training":"manual_confirmation_required"}),
            )?;
            Ok(dataset)
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::research_projects::SaveResearchProject;
    use super::*;
    fn setup() -> (Connection, String, String) {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            SaveResearchProject {
                id: None,
                name: "合成测试".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        let plan = experiments::create(
            &mut c,
            "ws",
            experiments::NewExperiment {
                title: "测试实验",
                objective: None,
                variables_json: r#"[{"name":"C","value":0.4}]"#,
                recommendation_json: None,
                state: "proposed",
            },
        )
        .unwrap();
        snapshot_plan(&c, "ws", &p.id, &plan.id).unwrap();
        experiments::set_state(&mut c, "ws", &plan.id, "accepted").unwrap();
        (c, p.id, plan.id)
    }
    fn input(p: &str, plan: &str, value: f64) -> RecordExecution {
        RecordExecution {
            project_id: p.into(),
            plan_id: plan.into(),
            sample_id: "合成测试-1".into(),
            heat_id: Some("炉次-1".into()),
            batch_id: None,
            actual_variables: BTreeMap::from([("C".into(), 0.4)]),
            variable_units: BTreeMap::from([("C".into(), "wt%".into())]),
            performed_at: "2026-01-01T12:00:00Z".into(),
            method: "测试设备".into(),
            source_type: "measured".into(),
            note: "自动化合成夹具，不代表真实实验".into(),
            measurements: vec![MeasurementInput {
                property: "HV".into(),
                value,
                unit: "HV".into(),
                method: "HV10".into(),
                uncertainty: Some(2.),
                repeat_count: 1,
                measured_at: "2026-01-01T12:00:00Z".into(),
                note: "fixture".into(),
            }],
        }
    }
    fn accept(c: &Connection, id: &str) {
        review(c, "ws", id, "submitted", "提交", "draft").unwrap();
        review(c, "ws", id, "accepted", "确认夹具", "submitted").unwrap();
    }
    #[test]
    fn independent_replicates_and_append_only_review() {
        let (c, p, plan) = setup();
        let a = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
        let b = record(&c, "ws", input(&p, &plan, 410.)).unwrap();
        assert_eq!(b.replicate, a.replicate + 1);
        assert_ne!(a.measurements[0].id, b.measurements[0].id);
        assert!(
            reviewed_table(&c, "ws", &p, &[a.id.clone()], &["C".into()], &["HV".into()]).is_err()
        );
        assert!(review(&c, "ws", &a.id, "accepted", "", "draft").is_err());
        accept(&c, &a.id);
        assert_eq!(get(&c, "ws", &a.id).unwrap().reviews.len(), 3);
        assert!(c
            .execute("UPDATE experiment_measurements SET value=0", [])
            .is_err());
        assert!(get(&c, "other", &a.id).is_err());
        assert_eq!(
            experiments::get(&c, "ws", &plan).unwrap().unwrap().state,
            "accepted"
        );
    }
    #[test]
    fn rejects_missing_units_nan_wrong_project_and_synthetic_training() {
        let (c, p, plan) = setup();
        let mut invalid = input(&p, &plan, 400.);
        invalid.measurements[0].unit.clear();
        assert!(record(&c, "ws", invalid).is_err());
        let mut invalid = input(&p, &plan, f64::NAN);
        assert!(record(&c, "ws", invalid).is_err());
        invalid = input(&p, &plan, 400.);
        invalid.source_type = "synthetic".into();
        let e = record(&c, "ws", invalid).unwrap();
        accept(&c, &e.id);
        assert!(reviewed_table(&c, "ws", &p, &[e.id], &["C".into()], &["HV".into()]).is_err());
        assert!(record(&c, "other", input(&p, &plan, 400.)).is_err());
    }
    #[test]
    fn accepted_measurements_create_new_persistent_dataset_and_lineage() {
        let (c, p, plan) = setup();
        let a = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
        let b = record(&c, "ws", input(&p, &plan, 410.)).unwrap();
        accept(&c, &a.id);
        accept(&c, &b.id);
        let root = std::env::temp_dir().join(format!("suna-实验-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let d = create_dataset_version(
            &c,
            "ws",
            &root,
            DatasetVersionRequest {
                project_id: p.clone(),
                execution_ids: vec![a.id, b.id],
                features: vec!["C".into()],
                properties: vec!["HV".into()],
                parent_dataset_id: None,
            },
        )
        .unwrap();
        assert_eq!(d.row_count, 2);
        assert_eq!(d.mapping_state, "draft");
        assert!(std::path::Path::new(&d.source_path).exists());
        assert_eq!(
            super::super::steel::get(&c, "ws", &d.id)
                .unwrap()
                .unwrap()
                .columns[1]
                .unit
                .as_deref(),
            Some("HV")
        );
        assert_eq!(
            artifacts::edges(&c, "ws", &p, 100, 0)
                .unwrap()
                .iter()
                .filter(|e| e.relation == "uses_dataset")
                .count(),
            2
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn comparison_checks_units_and_defines_prediction_minus_actual() {
        let (c, p, plan) = setup();
        let e = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
        let a=artifacts::create(&c,"ws",&p,"optimization_result","测试优化","fixture",None,json!({"result":{"objective_specs":[{"name":"HV","unit":"HV"}],"recommendations":[{"objectives":[415.],"values":{"C":0.4}}]}})).unwrap();
        assert_eq!(
            compare(&c, "ws", &e.id, &a, 0).unwrap()[0].prediction_minus_actual,
            15.
        );
        let candidate = candidate_plan(&c, "ws", &p, &a, 0, "来源候选").unwrap();
        assert_eq!(candidate.state, "proposed");
        assert!(artifacts::edges(&c, "ws", &p, 100, 0)
            .unwrap()
            .iter()
            .any(|e| e.relation == "recommends_experiment"));
    }
}
