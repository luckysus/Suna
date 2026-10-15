use super::logic::{OptimizeSteelProcessRequest, MAX_OPTIMIZATION_TRIALS};
use crate::db::current_workspace_id;
use crate::tasks::{repository, NewTask, TaskRecord, TaskState};
use serde_json::{json, Value};
use std::collections::HashSet;

// Artifacts are fetched from completed local tasks. Never accept pickles from UI.
pub fn build_joint_payload(
    request: &OptimizeSteelProcessRequest,
    artifacts: &std::collections::HashMap<String, Value>,
) -> Result<Value, String> {
    if request.dataset_id.trim().is_empty()
        || !(1..=4).contains(&request.objectives.len())
        || !(1..=MAX_OPTIMIZATION_TRIALS).contains(&request.trials)
    {
        return Err("dataset, 1-4 objectives and 1-500 trials are required".into());
    }
    let algorithm = request
        .algorithm
        .as_deref()
        .unwrap_or(if request.objectives.len() > 1 {
            "nsga2"
        } else {
            "tpe"
        });
    if !["nsga2", "grid", "tpe"].contains(&algorithm)
        || (request.objectives.len() > 1 && algorithm == "tpe")
    {
        return Err("joint optimization requires NSGA-II/grid, or single-objective TPE".into());
    }
    let policy = request.domain_policy.as_deref().unwrap_or("annotate_all");
    if !["annotate_all", "within_domain_only"].contains(&policy) || request.variables.len() > 64 {
        return Err("joint_domain_policy_or_variable_budget_invalid".into());
    }
    let mut names = Vec::new();
    let mut bounds = Vec::new();
    let mut fixed = serde_json::Map::new();
    for v in &request.variables {
        let name = v["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .ok_or("variable name required")?;
        if names.contains(&name) {
            return Err("duplicate variable name".into());
        }
        let low = finite(&v["min"])?;
        let high = finite(&v["max"])?;
        if low > high {
            return Err("variable range is inverted".into());
        }
        if !v["fixedValue"].is_null() {
            let value = finite(&v["fixedValue"])?;
            if value < low || value > high {
                return Err("fixed value is outside bounds".into());
            }
            fixed.insert(name.into(), json!(value));
        }
        names.push(name);
        bounds.push(json!({"min": low, "max": high}));
    }
    if names.is_empty() {
        return Err("optimization variables are required".into());
    }
    let mut objectives = Vec::new();
    let mut labels = HashSet::new();
    for spec in &request.objectives {
        let name = spec["name"]
            .as_str()
            .filter(|n| !n.trim().is_empty())
            .ok_or("objective name required")?;
        if !labels.insert(name) {
            return Err("duplicate objective name".into());
        }
        let direction = spec["direction"].as_str().unwrap_or("");
        if !["maximize", "minimize"].contains(&direction) {
            return Err("objective direction required".into());
        }
        let mut objective = spec.clone();
        match spec["source"].as_str() {
            Some("prediction_model") => {
                let id = spec["modelId"].as_str().ok_or("modelId required")?;
                let artifact = artifacts.get(id).ok_or("model artifact missing")?;
                let features = artifact["feature_names"]
                    .as_array()
                    .ok_or("model has no feature schema")?;
                for feature in features {
                    if !names.contains(&feature.as_str().ok_or("invalid model feature")?) {
                        return Err(format!("model feature {feature} missing from variables"));
                    }
                }
                crate::compute::joint_applicability::units::model_transforms(
                    &mut objective,
                    artifact,
                    &request.variables,
                )?;
                objective["artifact"] = artifact.clone();
                objective["model_id"] = json!(id);
                use sha2::{Digest, Sha256};
                objective["model_sha256"] = json!(format!(
                    "{:x}",
                    Sha256::digest(
                        serde_json::to_vec(artifact)
                            .map_err(|_| "joint_model_serialization_failed")?
                    )
                ));
            }
            Some("formula") => {
                let c = spec["coefficients"]
                    .as_object()
                    .filter(|c| !c.is_empty())
                    .ok_or("formula coefficients required")?;
                for (n, v) in c {
                    if !names.contains(&n.as_str()) {
                        return Err(format!("unknown formula variable {n}"));
                    }
                    finite(v)?;
                }
            }
            _ => return Err("unsupported objective source".into()),
        }
        objectives.push(objective);
    }
    for c in &request.constraints {
        if !["equality", "inequality"].contains(&c["kind"].as_str().unwrap_or("")) {
            return Err("unsupported constraint kind".into());
        }
        let coefficients = c["coefficients"]
            .as_object()
            .ok_or("constraints require named coefficients")?;
        if coefficients.is_empty() {
            return Err("constraint coefficients required".into());
        }
        for (n, v) in coefficients {
            if !names.contains(&n.as_str()) {
                return Err(format!("unknown constraint variable {n}"));
            }
            finite(v)?;
        }
        finite(&c["value"])?;
    }
    // Freeze the units of the *optimization variables*. They may differ from
    // model input units and cannot be reconstructed from a later dataset.
    let variables = request
        .variables
        .iter()
        .map(|v| json!({"name":v["name"],"unit":v["unit"]}))
        .collect::<Vec<_>>();
    Ok(json!({"operation": "optimize_constrained", "payload": {
        "dataset_id": request.dataset_id, "feature_names": names, "bounds": bounds, "domain_policy":policy,
        "variables": variables,
        "fixed_values": fixed, "constraints": request.constraints, "objective_specs": objectives,
        "algorithm": algorithm, "trials": request.trials, "seed": request.seed
    }}))
}

fn finite(v: &Value) -> Result<f64, String> {
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| "finite number required".into())
}

pub fn submit_joint_optimization(
    connection: &rusqlite::Connection,
    request: &OptimizeSteelProcessRequest,
) -> Result<TaskRecord, String> {
    submit_joint_optimization_in_workspace(connection, current_workspace_id(), request)
}

pub fn submit_joint_optimization_in_workspace(
    connection: &rusqlite::Connection,
    workspace: &str,
    request: &OptimizeSteelProcessRequest,
) -> Result<TaskRecord, String> {
    let mut artifacts = std::collections::HashMap::new();
    for spec in &request.objectives {
        if spec["source"] != "prediction_model" {
            continue;
        }
        let id = spec["modelId"].as_str().ok_or("modelId required")?;
        let task = repository::get(
            connection,
            workspace,
            uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?
        .ok_or("training task not found")?;
        if !crate::compute::handler::is_training_task_kind(&task.kind)
            || task.state != TaskState::Completed
        {
            return Err("objective model must be a completed training task".into());
        }
        let training_payload: Value =
            serde_json::from_str(&task.payload_json).map_err(|e| e.to_string())?;
        if training_payload["payload"]["dataset_id"].as_str() != Some(request.dataset_id.as_str()) {
            return Err("objective model dataset does not match the optimization dataset".into());
        }
        let checkpoint: Value = serde_json::from_str(
            task.checkpoint_json
                .as_deref()
                .ok_or("training result missing")?,
        )
        .map_err(|e| e.to_string())?;
        artifacts.insert(id.to_string(), checkpoint["result"]["artifact"].clone());
    }
    let mut effective = request.clone();
    let snapshot = crate::storage::repositories::material_rules::optimization_rules(
        connection,
        workspace,
        &mut effective,
    )?;
    let mut payload = build_joint_payload(&effective, &artifacts)?;
    if let Some(snapshot) = snapshot {
        crate::steel::materials::verify_worker_system(&payload["payload"])?;
        payload["payload"]["material_rule_snapshot"] =
            serde_json::to_value(snapshot).map_err(|e| e.to_string())?;
    }

    let payload_json = super::logic::serialize_compute_task_payload(&payload)?;
    let task = repository::create(
        connection,
        NewTask {
            workspace_id: workspace.into(),
            kind: crate::compute::handler::COMPUTE_OPTIMIZE_CONSTRAINED_KIND.into(),
            payload_json,
            checkpoint_json: Some(json!({"stage":"queued"}).to_string()),
            next_run_at: None,
            progress: 0,
        },
    )
    .map_err(|e| e.to_string())?;
    crate::storage::repositories::compute_tasks::create_optimization(
        connection,
        workspace,
        crate::storage::repositories::compute_tasks::NewOptimizationTask {
            background_task_id: &task.id.to_string(),
            dataset_id: Some(&request.dataset_id),
            model_id: None,
            direction: request.objectives[0]["direction"]
                .as_str()
                .unwrap_or("maximize"),
            method: request.algorithm.as_deref(),
            trials_requested: request.trials.into(),
            request_json: &serde_json::to_string(request).map_err(|e| e.to_string())?,
        },
    )?;
    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_variables_accept_noncontiguous_dataset_columns_and_independent_models() {
        let request: OptimizeSteelProcessRequest = serde_json::from_value(json!({
            "datasetId":"dataset", "variables":[{"name":"C","min":0,"max":1},{"name":"temperature","min":500,"max":1000,"fixedValue":900}],
            "objectives":[{"name":"HV","source":"prediction_model","modelId":"hardness","direction":"maximize","unit":"HV"},{"name":"cost","source":"formula","direction":"minimize","unit":"CNY","coefficients":{"C":2}}],
            "constraints":[{"kind":"inequality","coefficients":{"C":1},"value":0.1}], "trials":30,"seed":42
        })).unwrap();
        let artifacts = std::collections::HashMap::from([(
            "hardness".into(),
            json!({"feature_names":["temperature","C"]}),
        )]);
        let payload = build_joint_payload(&request, &artifacts).unwrap();
        assert_eq!(
            payload["payload"]["feature_names"],
            json!(["C", "temperature"])
        );
        assert_eq!(
            payload["payload"]["objective_specs"][0]["artifact"]["feature_names"],
            json!(["temperature", "C"])
        );
        assert_eq!(
            payload["payload"]["fixed_values"]["temperature"],
            json!(900.0)
        );
        assert_eq!(
            payload["payload"]["objective_specs"][1]["direction"],
            "minimize"
        );
        let mut invalid = request.clone();
        invalid.variables[1]["name"] = json!("wrong");
        assert!(build_joint_payload(&invalid, &artifacts).is_err());
    }
    #[test]
    fn formula_cannot_reference_unknown_variables_or_execute_expressions() {
        let request: OptimizeSteelProcessRequest = serde_json::from_value(json!({"datasetId":"dataset","variables":[{"name":"C","min":0,"max":1}],"objectives":[{"name":"cost","source":"formula","direction":"minimize","unit":"CNY","coefficients":{"unknown":1}}],"trials":20})).unwrap();
        assert!(build_joint_payload(&request, &std::collections::HashMap::new()).is_err());
    }
    #[test]
    fn actual_request_shape_compiles_two_frozen_unit_schemas_and_only_explicit_filter() {
        let request:OptimizeSteelProcessRequest=serde_json::from_value(json!({"datasetId":"dataset","domainPolicy":"within_domain_only",
            "variables":[{"name":"C","unit":"wt%","min":0.1,"max":0.3},{"name":"temperature","unit":"degC","min":700,"max":730}],
            "objectives":[{"name":"strength","source":"prediction_model","modelId":"first","direction":"maximize","unit":"GPa","feature_transforms":{"C":{"scale":10000}}},
                {"name":"toughness","source":"prediction_model","modelId":"second","direction":"minimize","unit":"J"}],"trials":25,"seed":42})).unwrap();
        let models = std::collections::HashMap::from([
            (
                "first".into(),
                json!({"feature_names":["C","temperature"],"feature_schema":{"fields":[{"name":"C","unit":"wt%"},{"name":"temperature","unit":"degC"}]},"target_schema":{"unit":"MPa"}}),
            ),
            (
                "second".into(),
                json!({"feature_names":["temperature","C"],"feature_schema":{"fields":[{"name":"temperature","unit":"K"},{"name":"C","unit":"fraction"}]},"target_schema":{"unit":"J"}}),
            ),
        ]);
        let original = models.clone();
        let payload = build_joint_payload(&request, &models).unwrap();
        let specs = &payload["payload"]["objective_specs"];
        assert_eq!(payload["payload"]["domain_policy"], "within_domain_only");
        assert_eq!(specs[0]["feature_transforms"]["C"]["scale"], 1.);
        assert_eq!(specs[0]["prediction_transform"]["scale"], 0.001);
        assert_eq!(specs[1]["feature_transforms"]["C"]["scale"], 0.01);
        assert_eq!(
            specs[1]["feature_transforms"]["temperature"]["shift"],
            273.15
        );
        assert_eq!(models, original);
        let mut default = request.clone();
        default.domain_policy = None;
        assert_eq!(
            build_joint_payload(&default, &models).unwrap()["payload"]["domain_policy"],
            "annotate_all"
        );
    }

    #[test]
    fn invalid_policy_or_guessed_model_units_fails_before_worker_submission() {
        let mut request:OptimizeSteelProcessRequest=serde_json::from_value(json!({"datasetId":"dataset","domainPolicy":"fake_physical_constraint", "variables":[{"name":"C","unit":"fraction","min":0.1,"max":0.3}],"objectives":[{"name":"HV","source":"prediction_model","modelId":"first","direction":"maximize","unit":"HV"}],"trials":25})).unwrap();
        let models =
            std::collections::HashMap::from([("first".into(), json!({"feature_names":["C"]}))]);
        assert!(build_joint_payload(&request, &models).is_err());
        request.domain_policy = None;
        assert_eq!(
            build_joint_payload(&request, &models).unwrap_err(),
            "joint_model_units_unknown"
        );
    }
}
