//! Read-only preflight of the same fixed values and linear system sent to Worker.
use super::schema::Template;
use crate::{steel::materials, storage::repositories::material_rules::RuleSnapshot};
use serde_json::{json, Map, Value};

pub(super) fn preflight(
    template: &Template,
    snapshot: Option<&RuleSnapshot>,
) -> Result<(), String> {
    if template.constraints.is_empty() {
        // Keep the existing large, unconstrained workflow contract. Project
        // rules have their own original typed compilation and feasibility gate.
        return Ok(());
    }
    let mut constraints = template.constraints.clone();
    if let Some(snapshot) = snapshot {
        constraints.extend(materials::worker_constraints(&snapshot.compiled));
    }
    if constraints.len() > 64 {
        return Err("workflow_constraint_budget".into());
    }
    let mut relaxed = Vec::new();
    for constraint in constraints {
        let value = constraint["value"]
            .as_f64()
            .ok_or("workflow_constraint_invalid")?;
        let tolerance = constraint["tolerance"].as_f64().unwrap_or(1e-6);
        if !(value - tolerance).is_finite() || !(value + tolerance).is_finite() {
            return Err("workflow_constraint_preflight_overflow".into());
        }
        // Worker feasibility is |sum-value| <= tolerance (equality), or
        // sum >= value-tolerance. Reuse the original materials solver with
        // these explicit inequalities rather than inventing another solver.
        relaxed.push(json!({"kind":"inequality","coefficients":constraint["coefficients"],"value":value-tolerance}));
        if constraint["kind"] == "equality" {
            let coefficients: Map<String, Value> = constraint["coefficients"]
                .as_object()
                .ok_or("workflow_constraint_invalid")?
                .iter()
                .map(|(name, value)| (name.clone(), json!(-value.as_f64().unwrap())))
                .collect();
            relaxed.push(
                json!({"kind":"inequality","coefficients":coefficients,"value":-value-tolerance}),
            );
        }
    }
    let fixed: Map<String, Value> = template
        .variables
        .iter()
        .filter_map(|variable| {
            variable["fixedValue"]
                .as_f64()
                .map(|value| (variable["name"].as_str().unwrap().into(), json!(value)))
        })
        .collect();
    let system = json!({
        "feature_names":template.variables.iter().map(|v|&v["name"]).collect::<Vec<_>>(),
        "bounds":template.variables.iter().map(|v|json!({"min":v["min"],"max":v["max"]})).collect::<Vec<_>>(),
        "fixed_values":fixed,"constraints":relaxed,
    });
    materials::verify_worker_system(&system).map_err(|error| match error.as_str() {
        "material_preflight_supports_up_to_24_variables" => {
            "workflow_constraint_preflight_variable_budget".into()
        }
        e if e.starts_with("material_preflight_complexity_limit") => {
            "workflow_constraint_preflight_complexity_budget".into()
        }
        "material_preflight_overflow" => "workflow_constraint_preflight_overflow".into(),
        _ => "workflow_constraint_system_infeasible".into(),
    })
}
