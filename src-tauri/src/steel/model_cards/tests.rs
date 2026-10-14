use super::*;
fn fixture() -> Value {
    json!({"model_type":"random_forest","data_version":"a".repeat(64),"feature_names":["C","temperature"],"feature_schema":{"fields":[{"name":"C","unit":"wt%"},{"name":"temperature","unit":"degC"}]},"target_schema":{"name":"HV","unit":"HV"},"parameters":{"n_estimators":100,"password":"synthetic-secret"},"split":{"kind":"time","seed":42,"train_indices":[0,1,2],"validation_indices":[3]},"metrics":{"validation":{"sample_count":1,"rmse":2,"mae":2,"r2":null}},"preprocessing":{"fit_scope":"train_only","imputation":"train_mean"}})
}
fn extract(a: &Value) -> Value {
    let body = a.to_string();
    cards(
        &body,
        &format!("{:x}", Sha256::digest(body.as_bytes())),
        &json!({"id":"11111111-1111-4111-8111-111111111111","version":1}),
    )
    .unwrap()
}
#[test]
fn extracts_actual_units_split_and_metrics() {
    let c = extract(&fixture());
    assert_eq!(c["model_card"]["features"][0]["unit"], "wt%");
    assert_eq!(c["model_card"]["split_counts"]["train"], 3);
    assert_eq!(
        c["model_card"]["metrics"]["validation"]["rmse"].as_f64(),
        Some(2.0)
    );
    assert_eq!(c["data_card"]["target"]["name"], "HV");
}
#[test]
fn legacy_missing_fields_remain_unknown() {
    let c = extract(&json!({"feature_names":["old"]}));
    assert!(c["model_card"]["algorithm"].is_null());
    assert!(c["model_card"]["features"][0]["unit"].is_null());
    assert!(c["data_card"]["dataset_sha256"].is_null());
    assert_eq!(c["data_card"]["quality_status"], "unknown");
}
#[test]
fn sensitive_fields_and_raw_rows_are_not_exported() {
    let mut a = fixture();
    a["source_path"] = json!("C:/private/raw.csv");
    a["raw_rows"] = json!([[123.123456]]);
    a["model_pickle_base64"] = json!("private-model-blob");
    let body = extract(&a).to_string();
    for forbidden in [
        "synthetic-secret",
        "C:/private",
        "123.123456",
        "private-model-blob",
        "train_indices",
    ] {
        assert!(!body.contains(forbidden));
    }
}
#[test]
fn tampering_changes_registry_sha_and_is_rejected() {
    let mut a = fixture();
    let original = a.to_string();
    let sha = format!("{:x}", Sha256::digest(original.as_bytes()));
    a["metrics"]["validation"]["rmse"] = json!(1);
    assert_eq!(
        cards(&a.to_string(), &sha, &json!({})).unwrap_err(),
        "model_card_hash_mismatch"
    );
}
#[test]
fn review_and_coverage_are_never_inferred() {
    let c = extract(&fixture());
    assert_eq!(c["model_card"]["review_status"], "unreviewed");
    assert_eq!(c["model_card"]["interval"]["status"], "unknown");
    assert!(c["model_card"]["interval"]["nominal_coverage"].is_null());
    assert_eq!(
        c["model_card"]["interval"]["exchangeability_verified"],
        false
    );
}
#[test]
fn recorded_coverage_retains_calibration_limitations() {
    let mut a = fixture();
    a["conformal"] = json!({"method":"split_conformal","confidence":0.95});
    a["evaluation"] = json!({"test_interval_coverage":0.9});
    a["metrics"]["test"] = json!({"sample_count":10});
    let c = extract(&a);
    assert_eq!(c["model_card"]["interval"]["observed_test_coverage"], 0.9);
    assert_eq!(
        c["model_card"]["interval"]["exchangeability_verified"],
        false
    );
}
#[test]
fn field_order_does_not_borrow_unrelated_units() {
    let mut a = fixture();
    a["feature_schema"]["fields"][0]["name"] = json!("other");
    assert!(extract(&a)["model_card"]["features"][0]["unit"].is_null());
}
#[test]
fn units_use_real_materials_aliases_and_leave_unknown_units_unknown() {
    let mut a = fixture();
    a["feature_schema"]["fields"][1]["unit"] = json!("°C");
    assert_eq!(extract(&a)["model_card"]["features"][1]["unit"], "°C");
    a["feature_schema"]["fields"][1]["unit"] = json!("synthetic-private-unit");
    assert!(extract(&a)["model_card"]["features"][1]["unit"].is_null());
}
#[test]
fn captures_recorded_svr_and_transformer_parameters_without_guessing_defaults() {
    let mut a = fixture();
    a["parameters"] =
        json!({"c":10.0,"epsilon":0.1,"hidden_layer_size":32,"train_subsampled":true,"d_model":16});
    let c = extract(&a);
    assert_eq!(c["model_card"]["parameters"]["c"], 10.0);
    assert_eq!(c["model_card"]["parameters"]["train_subsampled"], true);
    assert!(c["model_card"]["parameters"].get("kernel").is_none());
}
#[test]
fn card_hash_binds_both_versions_and_does_not_modify_source_model() {
    let a = fixture();
    let body = a.to_string();
    let model_sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let first = cards(&body, &model_sha, &json!({"version":1})).unwrap();
    let second = cards(&body, &model_sha, &json!({"version":2})).unwrap();
    assert_ne!(first["cards_sha256"], second["cards_sha256"]);
    assert_eq!(
        first["model_card"]["model_sha256"],
        second["model_card"]["model_sha256"]
    );
    assert_eq!(a.to_string(), body);
}
#[test]
fn runtime_versions_are_allowlisted_and_partial_metadata_is_explicit() {
    let mut a = fixture();
    a["runtime_dependencies"] =
        json!({"scikit-learn":"1.7.2","lightgbm":"C:/private/secret","api_key":"synthetic-secret"});
    let c = extract(&a);
    assert_eq!(
        c["model_card"]["runtime_dependencies"]["scikit-learn"],
        "1.7.2"
    );
    assert!(c["model_card"]["runtime_dependencies"]
        .get("lightgbm")
        .is_none());
    assert_eq!(c["model_card"]["environment_status"], "partially_recorded");
}
#[test]
fn fake_identity_and_paths_are_not_exposed_as_source_fields() {
    let a = fixture();
    let body = a.to_string();
    let c=cards(&body,&format!("{:x}",Sha256::digest(body.as_bytes())),&json!({"id":"------------------------------------","grouping_field":"C:/private/furnace","time_field":"https://secret.invalid/"})).unwrap();
    assert!(c["model_card"]["registry_id"].is_null());
    assert!(c["data_card"]["grouping_field"].is_null());
    assert!(c["data_card"]["time_field"].is_null());
}
#[test]
fn leading_whitespace_does_not_disguise_private_absolute_paths_as_labels() {
    assert!(label(&json!(" C:/synthetic-private/column")).is_none());
    assert!(label(&json!(" /synthetic-private/column")).is_none());
    assert!(label(&json!(" \\synthetic-private\\column")).is_none());
    assert!(known_unit(&json!(format!("{}HV", " ".repeat(1000)))).is_none());
}
#[test]
fn registration_preserves_model_bytes_and_read_rejects_card_tampering() {
    let a = fixture();
    let body = a.to_string();
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let context = json!({"version":3});
    let mut manifest = registration_manifest(&body, &sha, &body, &context).unwrap();
    assert_eq!(manifest["parameters"], a["parameters"]);
    let card = read_registered_cards(&body, &sha, &manifest.to_string(), &context).unwrap();
    assert_eq!(card["status"], "recorded");
    assert!(!card.to_string().contains("synthetic-secret"));
    manifest["scientific_cards"]["model_card"]["review_status"] = json!("reviewed");
    assert_eq!(
        read_registered_cards(&body, &sha, &manifest.to_string(), &context).unwrap_err(),
        "model_card_snapshot_hash_mismatch"
    );
    assert_eq!(a.to_string(), body);
}
#[test]
fn recomputed_digest_cannot_forge_metrics_against_source_artifact() {
    let a = fixture();
    let body = a.to_string();
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let mut manifest = registration_manifest(&body, &sha, &body, &json!({})).unwrap();
    let saved = &mut manifest["scientific_cards"];
    saved["model_card"]["metrics"]["test"]["rmse"] = json!(0.0);
    saved.as_object_mut().unwrap().remove("cards_sha256");
    saved["cards_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(saved.to_string().as_bytes())
    ));
    assert_eq!(
        read_registered_cards(&body, &sha, &manifest.to_string(), &json!({})).unwrap_err(),
        "model_card_snapshot_source_mismatch"
    );
}
#[test]
fn legacy_card_derivation_is_read_only_and_explicitly_not_persisted() {
    let a = fixture();
    let body = a.to_string();
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let result = read_registered_cards(&body, &sha, &body, &json!({})).unwrap();
    assert_eq!(result["status"], "legacy_derived");
    assert_eq!(result["persisted"], false);
    assert_eq!(a.to_string(), body);
}
#[test]
fn coverage_without_an_actual_recorded_test_count_remains_unknown() {
    let mut a = fixture();
    a["conformal"] = json!({"method":"split_conformal","confidence":0.95});
    a["evaluation"] = json!({"test_interval_coverage":1.0});
    assert!(extract(&a)["model_card"]["interval"]["observed_test_coverage"].is_null());
}
#[test]
fn frozen_training_counts_do_not_infer_unknown_filters_or_group_names() {
    let a = fixture();
    let payload = json!({"data_version":"a".repeat(64),"features":[[1],[2]],"targets":[3,4],"source_rows":[2,4]});
    let c = training_registration_context(&a, &payload).unwrap();
    assert_eq!(c["valid_row_count"], 2);
    assert!(c["filtered_row_count"].is_null());
    assert!(c["time_field"].is_null());
    let mut wrong = payload.clone();
    wrong["data_version"] = json!("b".repeat(64));
    assert_eq!(
        training_registration_context(&a, &wrong).unwrap_err(),
        "model_card_dataset_hash_mismatch"
    );
}
#[test]
fn explicit_frozen_summary_retains_counts_and_split_fields_without_source_rows() {
    let a = fixture();
    let payload = json!({"features":[[1],[2]],"targets":[3,4],"source_rows":[2,4],
            "data_summary":{"schema_version":"training-data-summary.v1","source_row_count":3,"valid_row_count":2,"filtered_row_count":1,"filter_policy":"exclude_missing_target_reject_invalid_numbers"},
            "split_field_metadata":{"schema_version":"training-split-field.v1","name":"source_time"}});
    let c = training_registration_context(&a, &payload).unwrap();
    assert_eq!(c["filtered_row_count"], 1);
    assert_eq!(c["time_field"], "source_time");
    assert!(c.get("source_rows").is_none());
}
