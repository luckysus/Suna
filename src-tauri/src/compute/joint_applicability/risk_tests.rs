use super::*;

fn fixture() -> (Value, Value) {
    let model_id = "a".repeat(24);
    let model_sha = "d".repeat(64);
    let support = "b".repeat(64);
    let data = "c".repeat(64);
    let payload = json!({"objective_specs":[{"source":"prediction_model","model_sha256":model_sha,"artifact":{"model_id":model_id,"applicability_support":{"support_sha256":support,"dataset_sha256":data}}}],"domain_policy":"annotate_all"});
    let candidate = json!({"values":{"C":0.2},"objectives":[200.],"prediction":200.,"feasible":true,
        "model_applicability":[{"objective_index":0,"candidate_index":0,"model_id":model_id,"model_sha256":model_sha,"support_sha256":support,"dataset_sha256":data,
            "schema_version":"applicability-support.v1","method":"marginal_training_ranges","probability":null,"physical_feasibility":"not_assessed","joint_support":"not_assessed","interval_status":"unknown",
            "risk":{"row_index":0,"state":"within_domain","reasons":[]}}]});
    let result = json!({"pareto_front":[candidate.clone()],"recommendations":[candidate],"domain_filter":{"policy":"annotate_all","scope":"returned_pareto_front","before":1,"after":1,"excluded":0,"status":"completed","joint_probability":null,"physical_constraints_changed":false}});
    (payload, result)
}

#[test]
fn correct_binding_is_preserved_and_unknown_fields_are_not_frozen() {
    let (payload, mut result) = fixture();
    for key in ["pareto_front", "recommendations"] {
        result[key][0]["model_applicability"][0]["private_path"] = json!("C:/private/raw.csv");
        result[key][0]["model_applicability"][0]["risk"]["password"] = json!("synthetic-secret");
    }
    validate_joint_result(&mut result, &payload).unwrap();
    assert!(!result.to_string().contains("private"));
    assert!(!result.to_string().contains("synthetic-secret"));
}
#[test]
fn model_support_dataset_and_objective_identity_tampering_are_rejected() {
    let (payload, result) = fixture();
    for (key, value) in [
        ("model_id", json!("d".repeat(24))),
        ("model_sha256", json!("e".repeat(64))),
        ("support_sha256", json!("d".repeat(64))),
        ("dataset_sha256", json!("d".repeat(64))),
        ("objective_index", json!(1)),
        ("candidate_index", json!(1)),
    ] {
        let mut bad = result.clone();
        for list in ["pareto_front", "recommendations"] {
            bad[list][0]["model_applicability"][0][key] = value.clone();
        }
        assert!(validate_joint_result(&mut bad, &payload).is_err(), "{key}");
    }
}
#[test]
fn empty_front_requires_explicit_filter_and_valid_before_after_evidence() {
    let (mut payload, mut result) = fixture();
    result["pareto_front"] = json!([]);
    result["recommendations"] = json!([]);
    result["domain_filter"]["after"] = json!(0);
    result["domain_filter"]["excluded"] = json!(1);
    result["domain_filter"]["status"] = json!("insufficient_domain_candidates");
    assert!(validate_joint_result(&mut result, &payload).is_err());
    payload["domain_policy"] = json!("within_domain_only");
    result["domain_filter"]["policy"] = json!("within_domain_only");
    validate_joint_result(&mut result, &payload).unwrap();
    result["domain_filter"]["excluded"] = json!(0);
    assert!(validate_joint_result(&mut result, &payload).is_err());
}
#[test]
fn filter_cannot_accept_unknown_or_claim_joint_confidence_or_change_constraints() {
    let (mut payload, mut result) = fixture();
    payload["domain_policy"] = json!("within_domain_only");
    result["domain_filter"]["policy"] = json!("within_domain_only");
    for key in ["pareto_front", "recommendations"] {
        result[key][0]["model_applicability"][0]["risk"]["state"] = json!("unknown");
    }
    assert!(validate_joint_result(&mut result, &payload).is_err());
    let (_, mut result) = fixture();
    result["domain_filter"]["joint_probability"] = json!(0.95);
    assert!(validate_joint_result(&mut result, &payload).is_err());
    let (_, mut result) = fixture();
    result["domain_filter"]["physical_constraints_changed"] = json!(true);
    assert!(validate_joint_result(&mut result, &payload).is_err());
}
#[test]
fn recommendations_must_reference_the_verified_front_and_calibration_conditions() {
    let (payload, mut result) = fixture();
    result["recommendations"][0]["values"]["C"] = json!(0.3);
    assert!(validate_joint_result(&mut result, &payload).is_err());
    let (mut payload, mut result) = fixture();
    payload["objective_specs"][0]["artifact"]["conformal"] = json!({"method":"split_conformal"});
    assert!(validate_joint_result(&mut result, &payload).is_err());
    for key in ["pareto_front", "recommendations"] {
        result[key][0]["model_applicability"][0]["interval_status"] =
            json!("requires_exchangeability_assumption");
    }
    validate_joint_result(&mut result, &payload).unwrap();
}
#[test]
fn legacy_single_model_payload_is_not_reinterpreted_as_joint() {
    validate_joint_result(
        &mut json!({"recommendations":[{}]}),
        &json!({"model_id":"legacy"}),
    )
    .unwrap();
}

#[test]
fn joint_legacy_models_without_support_must_remain_unknown() {
    let (mut payload, mut result) = fixture();
    payload["objective_specs"][0]["artifact"]
        .as_object_mut()
        .unwrap()
        .remove("applicability_support");
    for key in ["pareto_front", "recommendations"] {
        result[key][0]["model_applicability"][0]["support_sha256"] = Value::Null;
        result[key][0]["model_applicability"][0]["dataset_sha256"] = Value::Null;
    }
    assert!(validate_joint_result(&mut result, &payload).is_err());
    for key in ["pareto_front", "recommendations"] {
        result[key][0]["model_applicability"][0]["risk"]["state"] = json!("unknown");
    }
    validate_joint_result(&mut result, &payload).unwrap();
}
