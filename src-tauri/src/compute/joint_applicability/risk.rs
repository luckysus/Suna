//! Validate and project per-model evidence before persisting Worker results.
use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_FRONT: usize = 200;

pub fn project_risk_entries(value: &Value) -> Result<Value, &'static str> {
    let entries = value.as_array().ok_or("joint_risk_entries_invalid")?;
    if entries.len() > 4 {
        return Err("joint_risk_entries_budget");
    }
    let mut indices = HashSet::new();
    let mut projected = Vec::new();
    for entry in entries {
        let objective_index = entry["objective_index"]
            .as_u64()
            .filter(|i| *i < 4)
            .ok_or("joint_risk_identity")?;
        let candidate_index = entry["candidate_index"]
            .as_u64()
            .filter(|i| *i < MAX_FRONT as u64)
            .ok_or("joint_risk_identity")?;
        if !indices.insert(objective_index)
            || !matches!(
                entry["interval_status"].as_str(),
                Some("unknown" | "requires_exchangeability_assumption")
            )
        {
            return Err("joint_risk_identity");
        }
        if !entry["model_id"].is_null()
            && !entry["model_id"]
                .as_str()
                .is_some_and(|s| s.len() == 24 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("joint_risk_model_identity_invalid");
        }
        for field in ["model_sha256", "support_sha256", "dataset_sha256"] {
            if !entry[field].is_null()
                && !entry[field]
                    .as_str()
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err(match field {
                    "model_sha256" => "joint_risk_model_hash_invalid",
                    "support_sha256" => "joint_risk_support_hash_invalid",
                    _ => "joint_risk_dataset_hash_invalid",
                });
            }
        }
        let wrapper = json!({"schema_version":entry["schema_version"],"method":entry["method"],
            "support_sha256":entry["support_sha256"],"model_id":entry["model_id"],"probability":entry["probability"],
            "physical_feasibility":entry["physical_feasibility"],"joint_support":entry["joint_support"],"rows":[entry["risk"].clone()]});
        let snapshot = crate::compute::applicability::risk::risk_snapshot(&wrapper, 1)?;
        projected.push(json!({"objective_index":objective_index,"candidate_index":candidate_index,
            "model_id":snapshot["model_id"],"model_sha256":entry["model_sha256"],"schema_version":snapshot["schema_version"],"method":snapshot["method"],
            "support_sha256":snapshot["support_sha256"],"dataset_sha256":entry["dataset_sha256"],"risk":snapshot["rows"][0],
            "probability":null,"physical_feasibility":"not_assessed","joint_support":"not_assessed","interval_status":entry["interval_status"]}));
    }
    Ok(Value::Array(projected))
}

pub fn domain_filter_snapshot(value: &Value) -> Result<Value, &'static str> {
    if !matches!(
        value["policy"].as_str(),
        Some("annotate_all" | "within_domain_only")
    ) || value["scope"] != "returned_pareto_front"
        || !value["joint_probability"].is_null()
        || value["physical_constraints_changed"] != false
    {
        return Err("joint_filter_contract_invalid");
    }
    let before = value["before"]
        .as_u64()
        .filter(|n| *n <= MAX_FRONT as u64)
        .ok_or("joint_filter_count_invalid")?;
    let after = value["after"]
        .as_u64()
        .filter(|n| *n <= before)
        .ok_or("joint_filter_count_invalid")?;
    if before == 0
        || value["excluded"].as_u64() != Some(before - after)
        || value["status"]
            != if after == 0 {
                "insufficient_domain_candidates"
            } else {
                "completed"
            }
        || (value["policy"] == "annotate_all" && before != after)
    {
        return Err("joint_filter_count_invalid");
    }
    Ok(
        json!({"policy":value["policy"],"scope":"returned_pareto_front","before":before,"after":after,
        "excluded":before-after,"status":value["status"],"joint_probability":null,"physical_constraints_changed":false}),
    )
}

pub fn validate_joint_result(result: &mut Value, payload: &Value) -> Result<(), &'static str> {
    let Some(specs) = payload["objective_specs"].as_array() else {
        return Ok(());
    };
    let filter = domain_filter_snapshot(&result["domain_filter"])?;
    if filter["policy"].as_str()
        != Some(payload["domain_policy"].as_str().unwrap_or("annotate_all"))
    {
        return Err("joint_filter_policy_mismatch");
    }
    let after = filter["after"].as_u64().unwrap() as usize;
    let before = filter["before"].as_u64().unwrap();
    let models: Vec<_> = specs
        .iter()
        .enumerate()
        .filter(|(_, s)| s["source"] == "prediction_model")
        .collect();
    let front = result["pareto_front"]
        .as_array()
        .filter(|f| f.len() == after)
        .ok_or("joint_front_count_invalid")?;
    let recommendations = result["recommendations"]
        .as_array()
        .filter(|r| r.len() <= after)
        .ok_or("joint_recommendations_invalid")?;
    if (after > 0 && recommendations.is_empty())
        || recommendations.iter().any(|r| !front.contains(r))
    {
        return Err("joint_recommendations_invalid");
    }
    for key in ["pareto_front", "recommendations"] {
        let candidates = result[key]
            .as_array_mut()
            .ok_or("joint_front_count_invalid")?;
        let mut candidate_indices = HashSet::new();
        for candidate in candidates {
            if candidate["feasible"] != true
                || !candidate["values"].is_object()
                || !candidate["objectives"].as_array().is_some_and(|o| {
                    o.len() == specs.len()
                        && o.iter().all(|v| v.as_f64().is_some_and(f64::is_finite))
                })
                || !candidate["prediction"].as_f64().is_some_and(f64::is_finite)
            {
                return Err("joint_candidate_invalid");
            }
            let evidence = project_risk_entries(&candidate["model_applicability"])?;
            let entries = evidence.as_array().unwrap();
            if entries.len() != models.len()
                || (filter["policy"] == "within_domain_only"
                    && (entries.is_empty()
                        || entries
                            .iter()
                            .any(|e| e["risk"]["state"] != "within_domain")))
            {
                return Err("joint_candidate_domain_invalid");
            }
            let mut identity = None;
            for (entry, (objective_index, spec)) in entries.iter().zip(&models) {
                let artifact = &spec["artifact"];
                if entry["objective_index"].as_u64() != Some(*objective_index as u64)
                    || entry["model_id"] != artifact["model_id"]
                    || entry["model_sha256"] != spec["model_sha256"]
                    || entry["support_sha256"]
                        != artifact["applicability_support"]["support_sha256"]
                    || entry["dataset_sha256"]
                        != artifact["applicability_support"]["dataset_sha256"]
                    || entry["candidate_index"]
                        .as_u64()
                        .is_none_or(|i| i >= before)
                    || entry["interval_status"]
                        != if artifact["conformal"]["method"] == "split_conformal" {
                            "requires_exchangeability_assumption"
                        } else {
                            "unknown"
                        }
                {
                    return Err("joint_candidate_model_binding");
                }
                if artifact["applicability_support"].is_null()
                    && entry["risk"]["state"] != "unknown"
                {
                    return Err("joint_legacy_support_claim");
                }
                let index = entry["candidate_index"].as_u64().unwrap();
                if identity.is_some_and(|i| i != index) {
                    return Err("joint_candidate_model_binding");
                }
                identity = Some(index);
            }
            if identity.is_some_and(|i| !candidate_indices.insert(i)) {
                return Err("joint_candidate_duplicate_identity");
            }
            candidate["model_applicability"] = evidence;
        }
    }
    result["domain_filter"] = filter;
    if serde_json::to_vec(result)
        .map_err(|_| "joint_result_invalid")?
        .len()
        > 2 * 1024 * 1024
    {
        return Err("joint_result_budget");
    }
    Ok(())
}

#[cfg(test)]
#[path = "risk_tests.rs"]
mod tests;
