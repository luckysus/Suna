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
    let registry = super::material_rules::registry(c, ws, Some(&input.project_id))?;
    for (name, unit) in &input.variable_units {
        let suggestions = crate::steel::materials::suggest(&registry, name);
        if suggestions.len() == 1 {
            let field = registry
                .fields
                .iter()
                .find(|f| f.id == suggestions[0].field_id)
                .ok_or("field_missing")?;
            crate::steel::materials::conversion(unit, &field.canonical_unit)?;
        }
    }
    for measurement in &input.measurements {
        let suggestions = crate::steel::materials::suggest(&registry, &measurement.property);
        if suggestions.len() == 1 {
            let field = registry
                .fields
                .iter()
                .find(|f| f.id == suggestions[0].field_id)
                .ok_or("field_missing")?;
            crate::steel::materials::conversion(&measurement.unit, &field.canonical_unit)?;
        } else if suggestions.len() > 1 {
            return Err("ambiguous_measured_property_requires_canonical_field_id".into());
        }
    }

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
#[cfg(test)]
#[path = "experiment_measurements/tests.rs"]
mod tests;

mod dataset_versions;
pub use dataset_versions::{create_dataset_version, DatasetVersionRequest};

mod analysis;
pub use analysis::{candidate_plan, compare, reviewed_table, snapshot_plan, Comparison};
