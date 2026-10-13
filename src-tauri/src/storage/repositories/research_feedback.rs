use super::{
    experiment_measurements as measurements, material_rules as rules,
    research_artifacts as artifacts, research_projects as projects, steel,
};
use crate::steel::materials::{self, Variable};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendRequest {
    pub project_id: String,
    pub dataset_id: String,
    pub property: String,
    pub variables: Vec<Variable>,
    pub strategy: String,
    pub budget: u32,
    pub direction: String,
    pub seed: i64,
    #[serde(default = "distance")]
    pub min_distance: f64,
}
fn distance() -> f64 {
    0.01
}
pub fn submit(
    c: &Connection,
    ws: &str,
    r: RecommendRequest,
) -> Result<crate::tasks::TaskRecord, String> {
    projects::require_active(c, ws, &r.project_id)?;
    if r.variables.is_empty()
        || r.variables.len() > 12
        || !(1..=20).contains(&r.budget)
        || !matches!(
            r.strategy.as_str(),
            "exploitation" | "exploration" | "balanced"
        )
        || !matches!(r.direction.as_str(), "maximize" | "minimize")
        || !r.min_distance.is_finite()
        || !(0.0..=0.5).contains(&r.min_distance)
        || r.seed < 0
        || r.seed > i32::MAX as i64
        || r.variables
            .iter()
            .any(|v| v.min >= v.max || v.fixed_value.is_some())
    {
        return Err("invalid_recommendation_budget_or_search_space".into());
    }
    let version:Option<(String,String)>=c.query_row("SELECT project_id,measurement_ids_json FROM research_dataset_versions WHERE workspace_id=?1 AND dataset_id=?2",params![ws,r.dataset_id],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(|e|e.to_string())?;
    let Some((project, measurement_ids)) = version else {
        return Err("next_round_requires_a_reviewed_measurement_dataset_version".into());
    };
    if project != r.project_id {
        return Err("feedback_dataset_not_in_project".into());
    }
    let ids: Vec<String> = serde_json::from_str(&measurement_ids).map_err(|e| e.to_string())?;
    if ids.is_empty() {
        return Err("feedback_dataset_has_no_reviewed_measurements".into());
    }
    let data = steel::get(c, ws, &r.dataset_id)?.ok_or("feedback_dataset_missing")?;
    if data.mapping_state != "ready" {
        return Err("confirm_mapping_and_activate_feedback_dataset_first".into());
    }
    if crate::steel::hash_dataset_source(&data.source_path)? != data.source_sha256 {
        return Err("feedback_dataset_source_changed".into());
    }
    let table = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
        source_path: data.source_path.clone(),
        sheet: Some(data.selected_sheet.clone()),
    })?;
    if table.rows.len() > 5000 {
        return Err("feedback_GP_row_limit_5000".into());
    }
    if table.truncated {
        return Err("feedback_dataset_truncated".into());
    }
    let column = |name: &str| {
        data.columns
            .iter()
            .find(|col| col.canonical_field.as_deref() == Some(name) || col.original_name == name)
            .ok_or("feedback_field_missing")
    };
    let target = column(&r.property)?.ordinal;
    let mut feature_indices = vec![];
    for v in &r.variables {
        let col = column(&v.name)?;
        let unit = col
            .unit
            .as_deref()
            .ok_or("feedback_variable_unit_missing")?;
        let (s, b) = materials::conversion(&v.unit, unit)?;
        if s != 1. || b != 0. {
            return Err("feedback_variable_unit_must_match_dataset".into());
        }
        feature_indices.push(col.ordinal);
    }
    let enabled: Vec<String> = rules::list(c, ws, &r.project_id)?
        .into_iter()
        .filter(|r| r.rule.enabled)
        .map(|r| r.rule.id)
        .collect();
    let snapshot = rules::compile(c, ws, &r.project_id, r.variables.clone(), &enabled)?;
    let mut features = vec![];
    let mut targets = vec![];
    for row in table.rows {
        let values = feature_indices
            .iter()
            .map(|i| {
                row.get(*i)
                    .ok_or("feedback_row_width")?
                    .parse::<f64>()
                    .map_err(|_| "feedback_non_numeric_value")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let y = row
            .get(target)
            .ok_or("feedback_row_width")?
            .parse::<f64>()
            .map_err(|_| "feedback_non_numeric_target")?;
        if !y.is_finite() || values.iter().any(|v| !v.is_finite()) {
            return Err("feedback_nonfinite_value".into());
        }
        features.push(values);
        targets.push(y);
    }
    if features.len() < 5 {
        return Err("GP next-round recommendation requires at least five measured source rows; select a parent dataset when creating the version".into());
    }
    let mut exclude = vec![];
    let mut offset = 0;
    loop {
        let batch = measurements::list(c, ws, &r.project_id, 100, offset)?;
        for execution in &batch {
            let mut values = BTreeMap::new();
            for v in &r.variables {
                let col = column(&v.name)?;
                let source_name = if execution.actual_variables.contains_key(&v.name) {
                    v.name.as_str()
                } else {
                    col.original_name.as_str()
                };
                if let (Some(value), Some(unit)) = (
                    execution.actual_variables.get(source_name),
                    execution.variable_units.get(source_name),
                ) {
                    values.insert(v.name.clone(), materials::convert(*value, unit, &v.unit)?);
                }
            }
            if values.len() == r.variables.len() {
                exclude.push(values);
            }
        }
        if batch.len() < 100 {
            break;
        }
        offset += 100;
        if exclude.len() > 5000 {
            return Err("feedback_exclusion_limit".into());
        }
    }
    // Pending approved/proposed plans with explicit units also reserve their points.
    let mut page = 0;
    loop {
        let objects = projects::objects(c, ws, &r.project_id, 100, page)?;
        for link in objects.iter().filter(|o| o.kind == "experiment") {
            if let Some(plan) = super::experiments::get(c, ws, &link.id)? {
                if !matches!(plan.state.as_str(), "accepted" | "proposed") {
                    continue;
                }
                let raw: Value =
                    serde_json::from_str(&plan.variables_json).map_err(|e| e.to_string())?;
                let mut values = BTreeMap::new();
                for variable in &r.variables {
                    if let Some(item) = raw
                        .as_array()
                        .and_then(|a| a.iter().find(|i| i["name"] == variable.name))
                    {
                        if let (Some(value), Some(unit)) =
                            (item["value"].as_f64(), item["unit"].as_str())
                        {
                            values.insert(
                                variable.name.clone(),
                                materials::convert(value, unit, &variable.unit)?,
                            );
                        }
                    }
                }
                if values.len() == r.variables.len() {
                    exclude.push(values);
                }
            }
        }
        if objects.len() < 100 {
            break;
        }
        page += 100;
        if exclude.len() > 5000 {
            return Err("pending_candidate_exclusion_limit".into());
        }
    }
    let method = if r.strategy == "exploration" {
        "active_learning"
    } else {
        "bayesian"
    };
    let payload = json!({"_research_project_id":r.project_id,"operation":"design_experiments","payload":{"dataset_id":data.id,"data_version":data.source_sha256,"method":method,"variables":r.variables.iter().map(|v|json!({"name":v.name,"low":v.min,"high":v.max})).collect::<Vec<_>>(),"count":r.budget,"direction":r.direction,"seed":r.seed,"existing":{"feature_names":r.variables.iter().map(|v|v.name.clone()).collect::<Vec<_>>(),"features":features,"targets":targets},"recommendation_policy":{"strategy":r.strategy,"min_distance":r.min_distance,"constraints":materials::worker_constraints(&snapshot.compiled),"exclude":exclude},"material_rule_snapshot":snapshot,"feedback_source":{"measurement_ids":ids,"target":r.property,"uncertainty_source":"GaussianProcessRegressor surrogate fitted to measured dataset; independent of performance model uncertainty"}}});
    artifacts::atomic(c, || {
        let task = crate::app::compute_commands::logic::submit_design_experiments_on_connection(
            c, ws, payload,
        )?;
        artifacts::bind(c, ws, &r.project_id, &task.id.to_string())?;
        projects::audit(
            c,
            ws,
            &r.project_id,
            "next_round_requested",
            json!({"task_id":task.id,"budget":r.budget,"strategy":r.strategy,"dataset_id":data.id}),
        )?;
        Ok(task)
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub property: String,
    pub unit: String,
    pub count: usize,
    pub mean: f64,
    pub stddev: f64,
}
pub fn statistics(
    c: &Connection,
    ws: &str,
    p: &str,
    plan: &str,
) -> Result<Vec<Statistics>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT m.property,m.unit,m.value FROM experiment_measurements m JOIN experiment_executions e ON e.workspace_id=m.workspace_id AND e.id=m.execution_id WHERE e.workspace_id=?1 AND e.project_id=?2 AND e.plan_id=?3 AND e.source_type='measured' AND (SELECT state FROM experiment_reviews r WHERE r.workspace_id=e.workspace_id AND r.execution_id=e.id ORDER BY sequence DESC LIMIT 1)='accepted' ORDER BY m.property,m.unit").map_err(|e|e.to_string())?;
    let values = q
        .query_map(params![ws, p, plan], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut groups: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for (prop, unit, val) in values {
        groups.entry((prop, unit)).or_default().push(val)
    }
    Ok(groups
        .into_iter()
        .map(|((property, unit), v)| {
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let stddev = if v.len() > 1 {
                (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt()
            } else {
                0.
            };
            Statistics {
                property,
                unit,
                count: v.len(),
                mean,
                stddev,
            }
        })
        .collect())
}

pub fn design_plan(
    c: &Connection,
    ws: &str,
    p: &str,
    task: &str,
    index: usize,
    title: &str,
) -> Result<super::experiments::ExperimentRecord, String> {
    artifacts::atomic(c, || {
        projects::require_active(c, ws, p)?;
        let id:String=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND source_key=?3 AND kind='experiment_plan'",params![ws,p,task],|r|r.get(0)).map_err(|e|e.to_string())?;
        let a = artifacts::get(c, ws, &id)?;
        let payload = a.payload.ok_or("design_artifact_missing")?;
        let point = payload["result"]["points"]
            .get(index)
            .ok_or("design_candidate_missing")?;
        let values = point["values"].as_object().ok_or("design_values_missing")?;
        let variables: Vec<Value> = values
            .iter()
            .map(|(name, value)| {
                let unit = payload["request"]["material_rule_snapshot"]["variables"]
                    .as_array()
                    .and_then(|a| a.iter().find(|v| v["name"] == *name))
                    .and_then(|v| v["unit"].as_str());
                json!({"name":name,"value":value,"unit":unit})
            })
            .collect();
        let raw = json!(variables).to_string();
        let recommendation=json!({"design_task_id":task,"design_artifact_id":id,"candidate_index":index,"candidate":point}).to_string();
        let plan = super::experiments::create_on_connection(
            c,
            ws,
            super::experiments::NewExperiment {
                title,
                objective: Some("基于审核实测的下一轮实验"),
                variables_json: &raw,
                recommendation_json: Some(&recommendation),
                state: "proposed",
            },
        )?;
        let snapshot = measurements::snapshot_plan(c, ws, p, &plan.id)?;
        artifacts::edge(c, ws, &id, &snapshot, "recommends_experiment")?;
        Ok(plan)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_cross_workspace_missing_version_and_bad_budget() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            projects::SaveResearchProject {
                id: None,
                name: "fixture".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        let request = |budget| {
            serde_json::from_value::<RecommendRequest>(json!({"projectId":p.id,"datasetId":"unknown","property":"HV","variables":[],"strategy":"balanced","budget":budget,"direction":"maximize","seed":42})).unwrap()
        };
        assert!(submit(&c, "ws", request(21)).is_err());
        assert!(submit(&c, "other", request(3)).is_err());
        assert!(submit(&c, "ws", request(3)).is_err());
    }
    #[test]
    fn explicit_project_submission_preserves_the_current_selection() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let make = |c: &mut Connection, name: &str| {
            projects::save(
                c,
                "ws",
                projects::SaveResearchProject {
                    id: None,
                    name: name.into(),
                    description: "".into(),
                    material_system: None,
                    tags: vec![],
                    expected_updated_at: None,
                },
            )
            .unwrap()
        };
        let a = make(&mut c, "A");
        let b = make(&mut c, "B");
        c.execute(
            "INSERT INTO settings VALUES('ws','suna.research.current_project',?1,'now')",
            [json!(a.id).to_string()],
        )
        .unwrap();
        let task = crate::tasks::repository::create(
            &c,
            crate::tasks::NewTask {
                workspace_id: "ws".into(),
                kind: "compute_design_experiments".into(),
                payload_json: json!({"_research_project_id":b.id,"payload":{}}).to_string(),
                checkpoint_json: None,
                next_run_at: None,
                progress: 0,
            },
        )
        .unwrap();
        assert!(artifacts::runs(&c, "ws", &a.id, 10, 0).unwrap().is_empty());
        assert_eq!(
            artifacts::runs(&c, "ws", &b.id, 10, 0).unwrap()[0].background_task_id,
            Some(task.id.to_string())
        );
        assert_eq!(artifacts::current(&c, "ws").unwrap(), Some(a.id));
    }
}
