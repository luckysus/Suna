use super::*;

#[test]
fn standalone_scientific_flags_cannot_silently_disappear() {
    let table = DatasetTable {
        source_name: "synthetic.csv".into(),
        format: "csv".into(),
        sheets: vec!["CSV".into()],
        selected_sheet: "CSV".into(),
        headers: vec!["C".into(), "HV".into()],
        rows: (0..20)
            .map(|i| {
                vec![
                    (0.1 + f64::from(i) * 0.01).to_string(),
                    (100 + i).to_string(),
                ]
            })
            .collect(),
        row_count: 20,
        truncated: false,
        warnings: vec![],
    };
    let columns = table
        .headers
        .iter()
        .enumerate()
        .map(|(ordinal, name)| SteelDatasetColumnRecord {
            ordinal,
            original_name: name.clone(),
            duplicate: false,
            inferred_type: "number".into(),
            canonical_field: Some(name.clone()),
            unit: Some(if ordinal == 0 { "wt%" } else { "HV" }.into()),
            non_empty_count: 20,
            missing_count: 0,
            invalid_count: 0,
            min: None,
            max: None,
        })
        .collect::<Vec<_>>();
    for (policy, expected) in [
        (
            json!({"learningCurve":true}),
            json!({"folds":0,"test_fraction":0.0,"calibration_fraction":0.0,"search_trials":0,"learning_curve":true}),
        ),
        (
            json!({"searchTrials":2}),
            json!({"folds":0,"test_fraction":0.0,"calibration_fraction":0.0,"search_trials":2,"learning_curve":false}),
        ),
    ] {
        let mut request = TrainSteelDatasetRequest {
            dataset_id: "synthetic".into(),
            target_column: 1,
            feature_columns: vec![0],
            algorithm: Some("random_forest".into()),
            split_policy: Some(policy),
        };
        assert_eq!(
            build_linear_regression_payload(&table, &columns, &request).unwrap_err(),
            "scientific_evaluation_requires_cross_validation"
        );
        request.split_policy.as_mut().unwrap()["folds"] = json!(2);
        let payload = build_linear_regression_payload(&table, &columns, &request).unwrap();
        let mut expected = expected;
        expected["folds"] = json!(2);
        assert_eq!(payload["evaluation"], expected);
    }
    let request = TrainSteelDatasetRequest {
        dataset_id: "synthetic".into(),
        target_column: 1,
        feature_columns: vec![0],
        algorithm: None,
        split_policy: None,
    };
    assert!(
        build_linear_regression_payload(&table, &columns, &request)
            .unwrap()
            .get("evaluation")
            .is_none(),
        "legacy requests must keep their original split without enabling scientific evaluation"
    );
    let before = build_linear_regression_payload(&table, &columns, &request).unwrap();
    let disabled = TrainSteelDatasetRequest {
        split_policy: Some(json!({"searchTrials":0,"learningCurve":false})),
        ..request
    };
    assert_eq!(
        build_linear_regression_payload(&table, &columns, &disabled).unwrap(),
        before
    );
}
