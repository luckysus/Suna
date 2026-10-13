use super::*;

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
            .map(|(name, value)| {
                let unit = payload["request"]["material_rule_snapshot"]["variables"]
                    .as_array()
                    .and_then(|a| a.iter().find(|v| v["name"] == *name))
                    .and_then(|v| v["unit"].as_str());
                json!({"name":name,"value":value,"unit":unit})
            })
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
