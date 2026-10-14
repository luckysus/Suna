use serde_json::{json, Map, Value};

fn known_state(value: &Value) -> bool {
    matches!(
        value.as_str(),
        Some("within_domain" | "warning" | "outside_domain" | "unknown")
    )
}

/// Validate worker risk rows and project only scientific evidence. Raw inputs,
/// paths and unexpected properties cannot enter the research metadata snapshot.
pub fn risk_snapshot(result: &Value, row_count: usize) -> Result<Value, &'static str> {
    if result["schema_version"] != "applicability-support.v1"
        || result["method"] != "marginal_training_ranges"
        || !result["probability"].is_null()
        || result["physical_feasibility"] != "not_assessed"
        || result["joint_support"] != "not_assessed"
    {
        return Err("invalid_applicability_contract");
    }
    let rows = result["rows"]
        .as_array()
        .ok_or("invalid_applicability_rows")?;
    if row_count == 0 || row_count > 10_000 || rows.len() != row_count {
        return Err("invalid_applicability_rows");
    }
    let mut safe_rows = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row["row_index"].as_u64() != Some(index as u64) || !known_state(&row["state"]) {
            return Err("invalid_applicability_row_identity");
        }
        let reasons = row["reasons"]
            .as_array()
            .ok_or("invalid_applicability_reasons")?;
        if reasons.len() > 140 {
            return Err("invalid_applicability_reasons");
        }
        let mut safe_reasons = Vec::new();
        for reason in reasons {
            if !known_state(&reason["state"])
                || !reason["code"].as_str().is_some_and(|code| {
                    matches!(
                        code,
                        "outside_training_range"
                            | "missing_unit"
                            | "unit_conversion_required"
                            | "missing_dataset_sha256"
                            | "legacy_model_without_support"
                            | "imputed_with_training_mean"
                            | "unverified_training_scope"
                            | "missing_training_limits"
                            | "missing_input_feature"
                            | "missing_input_value"
                            | "unsupported_artifact_contract"
                            | "missing_model_identity"
                            | "missing_training_support"
                    )
                })
            {
                return Err("invalid_applicability_reason");
            }
            let mut projected = Map::new();
            for key in ["state", "code", "feature"] {
                if let Some(value) = reason.get(key) {
                    projected.insert(key.into(), value.clone());
                }
            }
            if let Some(threshold) = reason.get("threshold") {
                let mut bounded = Map::new();
                for key in ["min", "max", "unit", "source", "support_sha256"] {
                    if let Some(value) = threshold.get(key) {
                        bounded.insert(key.into(), value.clone());
                    }
                }
                projected.insert("threshold".into(), Value::Object(bounded));
            }
            safe_reasons.push(Value::Object(projected));
        }
        safe_rows.push(json!({"row_index":index,"state":row["state"],"reasons":safe_reasons}));
    }
    Ok(
        json!({"schema_version":result["schema_version"],"method":result["method"],
        "support_sha256":result["support_sha256"],"model_id":result["model_id"],
        "probability":null,"physical_feasibility":"not_assessed","joint_support":"not_assessed","rows":safe_rows}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn training_statistics_keep_exact_float_bits_across_json_round_trips() {
        let original = 11.180339887498949_f64;
        let worker = r#"{"scale":11.180339887498949}"#;
        let received: Value = serde_json::from_str(worker).unwrap();
        assert_eq!(
            received["scale"].as_f64().unwrap().to_bits(),
            original.to_bits()
        );
        let persisted = serde_json::to_string(&received).unwrap();
        let reloaded: Value = serde_json::from_str(&persisted).unwrap();
        assert_eq!(
            reloaded["scale"].as_f64().unwrap().to_bits(),
            original.to_bits()
        );
    }
    fn example() -> Value {
        json!({"schema_version":"applicability-support.v1","method":"marginal_training_ranges","probability":null,"physical_feasibility":"not_assessed","joint_support":"not_assessed","rows":[{"row_index":0,"state":"outside_domain","reasons":[{"state":"outside_domain","code":"outside_training_range","feature":"C","value":999,"credentials":"synthetic-secret","threshold":{"min":0,"max":1,"unit":"wt%","source":"training_split","path":"C:/private/raw.csv"}}]}]})
    }
    #[test]
    fn strips_raw_inputs_and_private_fields_but_keeps_threshold_evidence() {
        let value = risk_snapshot(&example(), 1).unwrap();
        let text = value.to_string();
        for sensitive in ["999", "synthetic-secret", "C:/private"] {
            assert!(!text.contains(sensitive));
        }
        assert_eq!(value["rows"][0]["reasons"][0]["threshold"]["max"], 1);
    }
    #[test]
    fn refuses_probabilities_and_physical_claims() {
        let mut value = example();
        value["probability"] = json!(0.95);
        assert!(risk_snapshot(&value, 1).is_err());
        value["probability"] = Value::Null;
        value["physical_feasibility"] = json!("feasible");
        assert!(risk_snapshot(&value, 1).is_err());
    }
    #[test]
    fn row_identity_must_match_entire_batch() {
        assert!(risk_snapshot(&example(), 2).is_err());
        let mut value = example();
        value["rows"][0]["row_index"] = json!(1);
        assert!(risk_snapshot(&value, 1).is_err());
    }
    #[test]
    fn missing_dataset_sha256_remains_an_unknown_reason() {
        let mut value = example();
        value["rows"][0]["state"] = json!("unknown");
        value["rows"][0]["reasons"] = json!([{"state":"unknown","code":"missing_dataset_sha256"}]);
        assert_eq!(
            risk_snapshot(&value, 1).unwrap()["rows"][0]["state"],
            "unknown"
        );
    }
}
