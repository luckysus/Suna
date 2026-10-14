//! Independent compiler/Dry Run contract. Execution is not implemented here.
//! Nodes reference audited existing APIs; no alternate task state machine.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelNode {
    pub name: String,
    pub algorithm: String,
    pub feature_columns: Vec<usize>,
    pub target_column: usize,
    pub direction: String,
    pub unit: Option<String>,
    pub split_policy: Value,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Template {
    pub schema_version: String,
    pub id: String,
    pub revision: u64,
    pub name: String,
    pub models: Vec<ModelNode>,
    pub variables: Vec<Value>,
    pub constraints: Vec<Value>,
    pub trials: u32,
    pub seed: i64,
    pub timeout_seconds: u32,
}

#[derive(Clone)]
pub struct FrozenInput {
    pub project_id: String,
    pub dataset_id: String,
    pub dataset_sha256: String,
}

pub fn dry_run(template: &Template, input: &FrozenInput) -> Result<Value, &'static str> {
    if template.schema_version != "research-workflow-template.v1"
        || template.revision == 0
        || uuid::Uuid::parse_str(&template.id).is_err()
        || uuid::Uuid::parse_str(&input.project_id).is_err()
        || uuid::Uuid::parse_str(&input.dataset_id).is_err()
        || input.dataset_sha256.len() != 64
        || !input.dataset_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || template.models.len() != 2
        || !(1..=500).contains(&template.trials)
        || !(30..=86400).contains(&template.timeout_seconds)
        || template.variables.is_empty()
        || template.variables.len() > 64
        || crate::label(&json!(template.name)).is_none()
    {
        return Err("workflow_template_invalid");
    }
    let targets: HashSet<_> = template
        .models
        .iter()
        .map(|model| model.target_column)
        .collect();
    if targets.len() != 2
        || template.models.iter().any(|model| {
            model
                .feature_columns
                .iter()
                .any(|column| targets.contains(column))
        })
    {
        return Err("workflow_target_feature_leakage");
    }
    let mut names = HashSet::new();
    for variable in &template.variables {
        let name = crate::label(&variable["name"]).ok_or("workflow_variable_invalid")?;
        if !names.insert(name)
            || variable.as_object().is_none_or(|fields| {
                fields.keys().any(|key| {
                    !["name", "min", "max", "fixedValue", "unit", "fieldId"].contains(&key.as_str())
                })
            })
        {
            return Err("workflow_variable_invalid");
        }
        let low = crate::finite(&variable["min"]).ok_or("workflow_variable_invalid")?;
        let high = crate::finite(&variable["max"]).ok_or("workflow_variable_invalid")?;
        if low > high {
            return Err("workflow_variable_invalid");
        }
        if !variable["fixedValue"].is_null()
            && crate::finite(&variable["fixedValue"]).is_none_or(|v| v < low || v > high)
        {
            return Err("workflow_variable_invalid");
        }
        if !variable["unit"].is_null()
            && variable["unit"]
                .as_str()
                .is_none_or(|unit| crate::steel::materials::conversion(unit, unit).is_err())
        {
            return Err("workflow_variable_invalid");
        }
        if !variable["fieldId"].is_null() && crate::label(&variable["fieldId"]).is_none() {
            return Err("workflow_variable_invalid");
        }
    }
    if template.constraints.len() > 128 {
        return Err("workflow_constraint_invalid");
    }
    for constraint in &template.constraints {
        let fields = constraint
            .as_object()
            .ok_or("workflow_constraint_invalid")?;
        if fields
            .keys()
            .any(|key| !["kind", "coefficients", "value", "tolerance"].contains(&key.as_str()))
            || !["equality", "inequality"].contains(&constraint["kind"].as_str().unwrap_or(""))
            || crate::finite(&constraint["value"]).is_none()
        {
            return Err("workflow_constraint_invalid");
        }
        let coefficients = constraint["coefficients"]
            .as_object()
            .ok_or("workflow_constraint_invalid")?;
        if coefficients.is_empty()
            || coefficients.len() > 64
            || coefficients.iter().any(|(name, value)| {
                !names.contains(name.as_str()) || crate::finite(value).is_none()
            })
        {
            return Err("workflow_constraint_invalid");
        }
        if !constraint["tolerance"].is_null()
            && crate::finite(&constraint["tolerance"]).is_none_or(|v| v <= 0.0)
        {
            return Err("workflow_constraint_invalid");
        }
    }
    let mut model_names = HashSet::new();
    let mut nodes = vec![];
    for (index, model) in template.models.iter().enumerate() {
        if crate::label(&json!(model.name)).is_none()
            || !model_names.insert(&model.name)
            || model.feature_columns.is_empty()
            || model.feature_columns.len() > 128
            || model
                .feature_columns
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
                != model.feature_columns.len()
            || model.feature_columns.contains(&model.target_column)
            || !["maximize", "minimize"].contains(&model.direction.as_str())
            || ![
                "linear_regression",
                "elasticnet",
                "random_forest",
                "hist_gradient_boosting",
                "lightgbm",
                "xgboost",
                "svr",
                "mlp",
                "transformer",
            ]
            .contains(&model.algorithm.as_str())
        {
            return Err("workflow_model_invalid");
        }
        if model
            .unit
            .as_ref()
            .is_some_and(|unit| crate::steel::materials::conversion(unit, unit).is_err())
        {
            return Err("workflow_model_unit_unknown");
        }
        let policy = model
            .split_policy
            .as_object()
            .ok_or("workflow_split_policy_invalid")?;
        if policy.keys().any(|key| {
            ![
                "kind",
                "seed",
                "validationFraction",
                "groupColumn",
                "timeColumn",
                "folds",
                "testFraction",
                "calibrationFraction",
                "searchTrials",
                "learningCurve",
            ]
            .contains(&key.as_str())
        }) {
            return Err("workflow_split_policy_invalid");
        }
        for (key, value) in policy {
            let valid = match key.as_str() {
                "kind" => value
                    .as_str()
                    .is_some_and(|v| ["random", "group", "time"].contains(&v)),
                "seed" => value
                    .as_i64()
                    .is_some_and(|v| (0..=u32::MAX as i64).contains(&v)),
                "groupColumn" | "timeColumn" => value.as_u64().is_some_and(|v| {
                    v < 1024
                        && !targets.contains(&(v as usize))
                        && !model.feature_columns.contains(&(v as usize))
                }),
                "folds" => value
                    .as_u64()
                    .is_some_and(|v| v == 0 || (2..=10).contains(&v)),
                "searchTrials" => value.as_u64().is_some_and(|v| v <= 20),
                "learningCurve" => value.is_boolean(),
                "validationFraction" => crate::finite(value).is_some_and(|v| v > 0.0 && v < 1.0),
                "testFraction" | "calibrationFraction" => {
                    crate::finite(value).is_some_and(|v| v >= 0.0 && v < 0.5)
                }
                _ => false,
            };
            if !valid {
                return Err("workflow_split_policy_invalid");
            }
        }
        let kind = policy
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("random");
        if !["random", "group", "time"].contains(&kind)
            || kind == "group" && model.split_policy["groupColumn"].as_u64().is_none()
            || kind == "time" && model.split_policy["timeColumn"].as_u64().is_none()
        {
            return Err("workflow_split_policy_invalid");
        }
        nodes.push(json!({"id":format!("model_{}",index+1),"operation":"train_steel_dataset_on_connection", "dependsOn":[],
            "taskKind":if model.algorithm=="linear_regression" {"compute_train_linear_regression"} else {"compute_train_sklearn_model"},
            "output":"completed_training_task.artifact","timeoutSeconds":template.timeout_seconds,"retry":"existing_scheduler_manual_retry",
            "cancel":"existing_background_task_cancellation","model":model}));
    }
    nodes.push(json!({"id":"joint_optimization","operation":"submit_joint_optimization","dependsOn":["model_1","model_2"],
        "taskKind":"compute_optimize_constrained","output":"optimization_result.pareto_front","algorithm":"nsga2","trials":template.trials,
        "variables":template.variables,"constraints":template.constraints,"seed":template.seed}));
    nodes.push(json!({"id":"proposed_plan","operation":"experiments.create_on_connection","dependsOn":["joint_optimization"],
        "state":"proposed","output":"experiment_plan","confirmationRequiredToAccept":true,"measurementsCreated":false}));
    let mut preview = json!({"schemaVersion":"research-workflow-dry-run.v1","templateId":template.id,"templateRevision":template.revision,
        "projectId":input.project_id,"datasetId":input.dataset_id,"datasetSha256":input.dataset_sha256,
        "nodes":nodes,"networkRequired":false,"externalServices":[],"externalCostEstimate":null,
        "writes":["background_tasks","registered_model_versions","research_artifacts","proposed_experiment_plan"],
        "confirmationRequired":["start_workflow","accept_proposed_plan"],"rawRowsInPreview":false,"timeoutSeconds":template.timeout_seconds,
        "validationStatus":"schema_only_requires_repository_dry_run"});
    preview["dryRunSha256"] = json!(format!(
        "{:x}",
        Sha256::digest(preview.to_string().as_bytes())
    ));
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Template, FrozenInput) {
        let t=serde_json::from_value(json!({"schemaVersion":"research-workflow-template.v1","id":"11111111-1111-4111-8111-111111111111","revision":1,"name":"性能建模与待确认实验计划",
            "models":[{"name":"硬度","algorithm":"random_forest","featureColumns":[0,1],"targetColumn":2,"direction":"maximize","unit":"HV","splitPolicy":{"kind":"group","groupColumn":4}},
            {"name":"韧性","algorithm":"svr","featureColumns":[1,0],"targetColumn":3,"direction":"maximize","unit":"J","splitPolicy":{"kind":"group","groupColumn":4}}],
            "variables":[{"name":"c","min":0.1,"max":0.4}],"constraints":[],"trials":25,"seed":42,"timeoutSeconds":3600})).unwrap();
        (
            t,
            FrozenInput {
                project_id: "22222222-2222-4222-8222-222222222222".into(),
                dataset_id: "33333333-3333-4333-8333-333333333333".into(),
                dataset_sha256: "a".repeat(64),
            },
        )
    }
    #[test]
    fn produces_two_independent_models_and_only_a_proposed_plan_with_confirmation() {
        let (t, i) = fixture();
        let p = dry_run(&t, &i).unwrap();
        assert_eq!(p["nodes"][0]["model"]["algorithm"], "random_forest");
        assert_eq!(p["nodes"][1]["model"]["algorithm"], "svr");
        assert_eq!(p["nodes"][3]["state"], "proposed");
        assert_eq!(p["nodes"][3]["measurementsCreated"], false);
        assert_eq!(p["nodes"][3]["confirmationRequiredToAccept"], true);
    }
    #[test]
    fn source_version_and_project_are_frozen_in_the_dry_run_digest() {
        let (t, mut i) = fixture();
        let p = dry_run(&t, &i).unwrap();
        assert_eq!(p, dry_run(&t, &i).unwrap());
        i.project_id = "44444444-4444-4444-8444-444444444444".into();
        assert_ne!(p["dryRunSha256"], dry_run(&t, &i).unwrap()["dryRunSha256"]);
        i.dataset_sha256 = "b".repeat(64);
        assert_ne!(p["dryRunSha256"], dry_run(&t, &i).unwrap()["dryRunSha256"]);
    }
    #[test]
    fn arbitrary_executables_measurements_and_auto_accept_flags_are_not_valid_template_inputs() {
        let (t, _) = fixture();
        let mut data = serde_json::to_value(t).unwrap();
        data["autoAcceptPlan"] = json!(true);
        assert!(serde_json::from_value::<Template>(data).is_err());
    }
    #[test]
    fn secret_or_path_fields_are_rejected_without_leaking_their_values() {
        let (mut t, i) = fixture();
        t.models[0].split_policy["password"] = json!("synthetic-secret");
        assert_eq!(
            dry_run(&t, &i).unwrap_err(),
            "workflow_split_policy_invalid"
        );
        t.models[0].split_policy = json!({"kind":"random"});
        t.name = "C:/synthetic-private/name".into();
        assert_eq!(dry_run(&t, &i).unwrap_err(), "workflow_template_invalid");
    }
    #[test]
    fn bounded_budget_and_split_conditions_fail_before_execution() {
        let (mut t, i) = fixture();
        t.trials = 501;
        assert!(dry_run(&t, &i).is_err());
        t.trials = 25;
        t.models[0].split_policy = json!({"kind":"group"});
        assert_eq!(
            dry_run(&t, &i).unwrap_err(),
            "workflow_split_policy_invalid"
        );
    }
    #[test]
    fn nested_values_under_allowed_policy_variable_or_constraint_names_are_rejected() {
        let (mut t, i) = fixture();
        t.models[0].split_policy["seed"] = json!({"password":"synthetic-secret"});
        assert_eq!(
            dry_run(&t, &i).unwrap_err(),
            "workflow_split_policy_invalid"
        );
        let (mut t, i) = fixture();
        t.variables[0]["unit"] = json!({"password":"synthetic-secret"});
        assert_eq!(dry_run(&t, &i).unwrap_err(), "workflow_variable_invalid");
        let (mut t, i) = fixture();
        t.constraints.push(json!({"kind":"equality","coefficients":{"c":1},"value":1,"password":"synthetic-secret"}));
        assert_eq!(dry_run(&t, &i).unwrap_err(), "workflow_constraint_invalid");
    }
    #[test]
    fn a_predicted_performance_cannot_be_another_models_input_or_split_label() {
        let (mut template, input) = fixture();
        template.models[1].target_column = template.models[0].target_column;
        assert!(dry_run(&template, &input).is_err());
        let (mut template, input) = fixture();
        let other_target = template.models[0].target_column;
        template.models[1].feature_columns.push(other_target);
        assert!(dry_run(&template, &input).is_err());
        let (mut template, input) = fixture();
        template.models[1].split_policy["groupColumn"] = json!(template.models[0].target_column);
        assert!(dry_run(&template, &input).is_err());
    }
}
