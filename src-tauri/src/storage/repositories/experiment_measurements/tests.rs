use super::super::research_projects::SaveResearchProject;
use super::*;
fn setup() -> (Connection, String, String) {
    let mut c = Connection::open_in_memory().unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "ws",
        SaveResearchProject {
            id: None,
            name: "合成测试".into(),
            description: "".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let plan = experiments::create(
        &mut c,
        "ws",
        experiments::NewExperiment {
            title: "测试实验",
            objective: None,
            variables_json: r#"[{"name":"C","value":0.4}]"#,
            recommendation_json: None,
            state: "proposed",
        },
    )
    .unwrap();
    snapshot_plan(&c, "ws", &p.id, &plan.id).unwrap();
    experiments::set_state(&mut c, "ws", &plan.id, "accepted").unwrap();
    (c, p.id, plan.id)
}
fn input(p: &str, plan: &str, value: f64) -> RecordExecution {
    RecordExecution {
        project_id: p.into(),
        plan_id: plan.into(),
        sample_id: "合成测试-1".into(),
        heat_id: Some("炉次-1".into()),
        batch_id: None,
        actual_variables: BTreeMap::from([("C".into(), 0.4)]),
        variable_units: BTreeMap::from([("C".into(), "wt%".into())]),
        performed_at: "2026-01-01T12:00:00Z".into(),
        method: "测试设备".into(),
        source_type: "measured".into(),
        note: "自动化合成夹具，不代表真实实验".into(),
        measurements: vec![MeasurementInput {
            property: "HV".into(),
            value,
            unit: "HV".into(),
            method: "HV10".into(),
            uncertainty: Some(2.),
            repeat_count: 1,
            measured_at: "2026-01-01T12:00:00Z".into(),
            note: "fixture".into(),
        }],
    }
}
fn accept(c: &Connection, id: &str) {
    review(c, "ws", id, "submitted", "提交", "draft").unwrap();
    review(c, "ws", id, "accepted", "确认夹具", "submitted").unwrap();
}
#[test]
fn independent_replicates_and_append_only_review() {
    let (c, p, plan) = setup();
    let a = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
    let b = record(&c, "ws", input(&p, &plan, 410.)).unwrap();
    assert_eq!(b.replicate, a.replicate + 1);
    assert_ne!(a.measurements[0].id, b.measurements[0].id);
    assert!(reviewed_table(&c, "ws", &p, &[a.id.clone()], &["C".into()], &["HV".into()]).is_err());
    assert!(review(&c, "ws", &a.id, "accepted", "", "draft").is_err());
    accept(&c, &a.id);
    assert_eq!(get(&c, "ws", &a.id).unwrap().reviews.len(), 3);
    assert!(c
        .execute("UPDATE experiment_measurements SET value=0", [])
        .is_err());
    assert!(get(&c, "other", &a.id).is_err());
    assert_eq!(
        experiments::get(&c, "ws", &plan).unwrap().unwrap().state,
        "accepted"
    );
}
#[test]
fn rejects_missing_units_nan_wrong_project_and_synthetic_training() {
    let (c, p, plan) = setup();
    let mut invalid = input(&p, &plan, 400.);
    invalid.measurements[0].unit.clear();
    assert!(record(&c, "ws", invalid).is_err());
    let mut invalid = input(&p, &plan, f64::NAN);
    assert!(record(&c, "ws", invalid).is_err());
    invalid = input(&p, &plan, 400.);
    invalid.source_type = "synthetic".into();
    let e = record(&c, "ws", invalid).unwrap();
    accept(&c, &e.id);
    assert!(reviewed_table(&c, "ws", &p, &[e.id], &["C".into()], &["HV".into()]).is_err());
    assert!(record(&c, "other", input(&p, &plan, 400.)).is_err());
}
#[test]
fn accepted_measurements_create_new_persistent_dataset_and_lineage() {
    let (c, p, plan) = setup();
    let a = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
    let b = record(&c, "ws", input(&p, &plan, 410.)).unwrap();
    accept(&c, &a.id);
    accept(&c, &b.id);
    let root = std::env::temp_dir().join(format!("suna-实验-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let d = create_dataset_version(
        &c,
        "ws",
        &root,
        DatasetVersionRequest {
            project_id: p.clone(),
            execution_ids: vec![a.id, b.id],
            features: vec!["C".into()],
            properties: vec!["HV".into()],
            parent_dataset_id: None,
        },
    )
    .unwrap();
    assert_eq!(d.row_count, 2);
    assert_eq!(d.mapping_state, "draft");
    assert!(std::path::Path::new(&d.source_path).exists());
    assert_eq!(
        super::super::steel::get(&c, "ws", &d.id)
            .unwrap()
            .unwrap()
            .columns[1]
            .unit
            .as_deref(),
        Some("HV")
    );
    assert_eq!(
        artifacts::edges(&c, "ws", &p, 100, 0)
            .unwrap()
            .iter()
            .filter(|e| e.relation == "uses_dataset")
            .count(),
        2
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn reviewed_replicate_statistics_keep_runs_separate() {
    let (c, p, plan) = setup();
    for value in [400., 410., 420.] {
        let e = record(&c, "ws", input(&p, &plan, value)).unwrap();
        accept(&c, &e.id);
    }
    let s = super::super::research_feedback::statistics(&c, "ws", &p, &plan).unwrap();
    assert_eq!(s[0].count, 3);
    assert_eq!(s[0].mean, 410.);
    assert_eq!(s[0].stddev, 10.);
}
#[test]
fn comparison_checks_units_and_defines_prediction_minus_actual() {
    let (c, p, plan) = setup();
    let e = record(&c, "ws", input(&p, &plan, 400.)).unwrap();
    let a=artifacts::create(&c,"ws",&p,"optimization_result","测试优化","fixture",None,json!({"result":{"objective_specs":[{"name":"HV","unit":"HV"}],"recommendations":[{"objectives":[415.],"values":{"C":0.4}}]}})).unwrap();
    assert_eq!(
        compare(&c, "ws", &e.id, &a, 0).unwrap()[0].prediction_minus_actual,
        15.
    );
    let candidate = candidate_plan(&c, "ws", &p, &a, 0, "来源候选").unwrap();
    assert_eq!(candidate.state, "proposed");
    assert!(artifacts::edges(&c, "ws", &p, 100, 0)
        .unwrap()
        .iter()
        .any(|e| e.relation == "recommends_experiment"));
}
