//! Versioned scientific workflows with read-only preflight and original task orchestration.
use crate::steel;
mod constraints;
pub mod entry;
pub mod lifecycle;
pub mod provenance;
pub mod rules;
pub mod runtime;
pub mod schema;
pub mod status;
pub mod templates;
pub mod worker_validation;
use crate::app::compute_commands::logic::{
    build_linear_regression_payload, TrainSteelDatasetRequest,
};
use crate::storage::repositories::{research_projects as projects, steel as datasets};
use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn label(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        let normalized = s.trim();
        !normalized.is_empty()
            && s.chars().count() <= 128
            && !s.chars().any(char::is_control)
            && !normalized.starts_with(['/', '\\'])
            && !s.contains("://")
            && !(normalized.len() >= 2 && normalized.as_bytes()[1] == b':')
    })
}
fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite())
}

pub fn dry_run_on_connection(
    connection: &Connection,
    workspace: &str,
    template: &schema::Template,
    input: &schema::FrozenInput,
) -> Result<Value, String> {
    let mut preview = schema::dry_run(template, input).map_err(str::to_owned)?;
    projects::require_active(connection, workspace, &input.project_id)?;
    let dataset = datasets::get(connection, workspace, &input.dataset_id)?
        .ok_or("workflow_dataset_missing")?;
    if dataset.mapping_state != "ready" || dataset.source_sha256 != input.dataset_sha256 {
        return Err("workflow_dataset_changed_or_not_ready".into());
    }
    if steel::hash_dataset_source(&dataset.source_path)
        .map_err(|_| "workflow_source_unavailable")?
        != dataset.source_sha256
    {
        return Err("workflow_source_changed".into());
    }
    let table = steel::read_dataset_table(&steel::DatasetPreviewRequest {
        source_path: dataset.source_path.clone(),
        sheet: Some(dataset.selected_sheet.clone()),
    })
    .map_err(|_| "workflow_source_unavailable")?;
    let mut validated = Vec::new();
    for model in &template.models {
        let request = TrainSteelDatasetRequest {
            dataset_id: dataset.id.clone(),
            target_column: model.target_column,
            feature_columns: model.feature_columns.clone(),
            split_policy: Some(model.split_policy.clone()),
            algorithm: Some(model.algorithm.clone()),
        };
        let mut payload = build_linear_regression_payload(&table, &dataset.columns, &request)
            .map_err(|_| "workflow_training_configuration_invalid")?;
        if payload.get("evaluation").is_some()
            && payload["features"]
                .as_array()
                .is_none_or(|rows| rows.len() < 10)
        {
            return Err("workflow_scientific_evaluation_requires_10_valid_rows".into());
        }
        payload["data_version"] = json!(dataset.source_sha256);
        payload["algorithm"] = json!(model.algorithm);
        for name in payload["feature_names"]
            .as_array()
            .ok_or("workflow_feature_schema_missing")?
        {
            let variable = template
                .variables
                .iter()
                .find(|v| v["name"] == *name)
                .ok_or("workflow_model_variable_missing")?;
            let feature = payload["feature_metadata"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == *name)
                .ok_or("workflow_feature_schema_missing")?;
            if let Some(from) = variable["unit"].as_str() {
                let to = feature["unit"]
                    .as_str()
                    .ok_or("workflow_training_unit_unknown")?;
                for key in ["min", "max", "fixedValue"] {
                    if let Some(value) = variable[key].as_f64() {
                        steel::materials::convert(value, from, to)?;
                    }
                }
            }
        }
        if let Some(to) = model.unit.as_deref() {
            let from = payload["target_schema"]["unit"]
                .as_str()
                .ok_or("workflow_target_unit_unknown")?;
            steel::materials::conversion(from, to)?;
        }
        let bytes = serde_json::to_vec(&payload).map_err(|_| "workflow_payload_invalid")?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("workflow_payload_budget".into());
        }
        validated.push(json!({"name":model.name,"algorithm":model.algorithm,"featureNames":payload["feature_names"],
            "targetSchema":payload["target_schema"],"validRowCount":payload["data_summary"]["valid_row_count"],
            "filteredRowCount":payload["data_summary"]["filtered_row_count"],"trainingPayloadSha256":format!("{:x}",Sha256::digest(&bytes))}));
    }
    preview["validationStatus"] = json!("repository_validated_read_only");
    preview["validatedModels"] = json!(validated);
    let snapshot = rules::snapshot(
        connection,
        workspace,
        &input.project_id,
        &input.dataset_id,
        &template.variables,
    )?;
    constraints::preflight(template, snapshot.as_ref())?;
    preview["materialRules"] = rules::summary(snapshot.as_ref());
    preview.as_object_mut().unwrap().remove("dryRunSha256");
    preview["dryRunSha256"] = json!(format!(
        "{:x}",
        Sha256::digest(preview.to_string().as_bytes())
    ));
    Ok(preview)
}

#[cfg(test)]
mod constraint_tests;
#[cfg(test)]
mod entry_tests;
#[cfg(test)]
mod migration_tests;
#[cfg(test)]
mod runtime_guard_tests;
#[cfg(test)]
mod runtime_recovery_tests;
#[cfg(test)]
pub(crate) mod runtime_support;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod scientific_evaluation_tests;
#[cfg(test)]
mod testing;
