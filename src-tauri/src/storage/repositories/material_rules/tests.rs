use super::super::research_projects::SaveResearchProject;
use super::*;
fn fixture() -> (Connection, String) {
    let mut c = Connection::open_in_memory().unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "ws",
        SaveResearchProject {
            id: None,
            name: "合成材料项目".into(),
            description: "".into(),
            material_system: Some("wear-resistant-steel".into()),
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    (c, p.id)
}
fn rule() -> Rule {
    serde_json::from_value(json!({"id":"equipment","name":"合成设备下限","severity":"hard","enabled":true,"penalty":1,"provenance":{"type":"equipment","sourceRef":"test-device","sourceVersion":"1","reviewed":true},"expression":{"kind":"range","fieldId":"process.quench_temperature","unit":"degC","min":850}})).unwrap()
}
#[test]
fn rule_versions_are_scoped_immutable_and_frozen() {
    let (c, p) = fixture();
    save(&c, "ws", &p, rule(), 0).unwrap();
    let v = Variable {
        name: "temperature".into(),
        field_id: "process.quench_temperature".into(),
        unit: "K".into(),
        min: 1073.15,
        max: 1273.15,
        fixed_value: None,
    };
    let frozen = compile(&c, "ws", &p, vec![v], &["equipment".into()]).unwrap();
    let mut changed = rule();
    changed.enabled = false;
    save(&c, "ws", &p, changed.clone(), 1).unwrap();
    assert_eq!(frozen.rules[0].version, 1);
    assert!(frozen.rules[0].rule.enabled);
    assert!(!list(&c, "ws", &p).unwrap()[0].rule.enabled);
    assert!(save(&c, "ws", &p, changed, 1).is_err());
    assert!(list(&c, "other", &p).is_err());
    assert!(c.execute("DELETE FROM material_rule_versions", []).is_err());
}
#[test]
fn explicit_schema_overrides_and_ambiguous_hardness() {
    let (c, p) = fixture();
    let mut field = materials::base().fields[0].clone();
    field.aliases.push("用户碳含量".into());
    assert!(save_overlay(&c, "ws", &p, vec![field.clone()], vec![]).is_err());
    save_overlay(&c, "ws", &p, vec![field.clone()], vec![field.id.clone()]).unwrap();
    assert_eq!(
        materials::suggest(&registry(&c, "ws", Some(&p)).unwrap(), "用户碳含量").len(),
        1
    );
}
#[test]
fn normalization_writes_new_file_and_mapping_without_touching_original() {
    let (c, p) = fixture();
    let root = std::env::temp_dir().join(format!("suna-material-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("原始.csv");
    std::fs::write(&path, "C,temperature\n0.004,1173.15\n0.005,1273.15\n").unwrap();
    let source = path.to_string_lossy().to_string();
    let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
        source_path: source.clone(),
        sheet: None,
    })
    .unwrap();
    let hash = crate::steel::hash_dataset_source(&source).unwrap();
    let original =
        super::super::steel::save_preview_on_connection(&c, "ws", &source, &hash, &preview, &[])
            .unwrap();
    let normalized = normalize_dataset(
        &c,
        "ws",
        &root,
        NormalizeRequest {
            project_id: p.clone(),
            dataset_id: original.id.clone(),
            mappings: vec![
                ColumnMapping {
                    ordinal: 0,
                    field_id: "chem.c".into(),
                    source_unit: "fraction".into(),
                },
                ColumnMapping {
                    ordinal: 1,
                    field_id: "process.quench_temperature".into(),
                    source_unit: "K".into(),
                },
            ],
        },
    )
    .unwrap();
    assert_ne!(original.id, normalized.id);
    assert_eq!(crate::steel::hash_dataset_source(&source).unwrap(), hash);
    assert_eq!(
        normalized.columns[0].canonical_field.as_deref(),
        Some("chem.c")
    );
    assert_eq!(normalized.columns[0].min, Some(0.4));
    assert!((normalized.columns[1].min.unwrap() - 900.).abs() < 1e-9);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM material_mapping_versions", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_activation_reopens_normalized_training_fields_without_rewriting_frozen_sources() {
    use crate::storage::repositories::steel as datasets;
    let root = std::env::temp_dir().join(format!("suna-activation-中文-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("验收.sqlite3");
    let (mut c, _) = crate::storage::database::open(&database).unwrap();
    let project = projects::save(
        &mut c,
        "ws",
        SaveResearchProject {
            id: None,
            name: "合成激活回归".into(),
            description: "非真实测量".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let path = root.join("原始合成.csv");
    let raw = "C,temp,HV,J,time\n0.004,1173.15,100,50,1\n0.005,1183.15,101,51,2\n0.006,1193.15,102,52,3\n0.007,1203.15,103,53,4\n0.008,1213.15,104,54,5\n0.009,1223.15,105,55,6\n";
    std::fs::write(&path, raw).unwrap();
    let source = path.to_string_lossy().to_string();
    let hash = crate::steel::hash_dataset_source(&source).unwrap();
    let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
        source_path: source.clone(),
        sheet: None,
    })
    .unwrap();
    let original =
        datasets::save_preview_on_connection(&c, "ws", &source, &hash, &preview, &[]).unwrap();
    assert!(datasets::activate(&mut c, "ws", &original.id).is_err());
    let normalized = normalize_dataset(
        &c,
        "ws",
        &root,
        NormalizeRequest {
            project_id: project.id.clone(),
            dataset_id: original.id.clone(),
            mappings: [
                (0, "chem.c", "fraction"),
                (1, "process.quench_temperature", "K"),
                (2, "property.hardness_hv", "HV"),
                (3, "property.impact_energy", "J"),
            ]
            .into_iter()
            .map(|(ordinal, field, unit)| ColumnMapping {
                ordinal,
                field_id: field.into(),
                source_unit: unit.into(),
            })
            .collect(),
        },
    )
    .unwrap();
    assert_eq!(normalized.mapping_state, "draft");
    assert_ne!(original.source_sha256, normalized.source_sha256);
    let frozen: Vec<_> = [&original.id, &normalized.id]
        .into_iter()
        .map(|id| {
            let snapshot = artifacts::dataset_snapshot(&c, "ws", &project.id, id)
                .unwrap()
                .unwrap();
            let body = serde_json::to_value(artifacts::get(&c, "ws", &snapshot).unwrap()).unwrap();
            (snapshot, body)
        })
        .collect();
    assert!(datasets::activate(&mut c, "other", &normalized.id)
        .unwrap()
        .is_none());
    use crate::app::steel_commands::logic::{activate_on_connection, DatasetActivationReview};
    let review = DatasetActivationReview {
        source_sha256: normalized.source_sha256.clone(),
        updated_at: normalized.updated_at.clone(),
    };
    let stale = DatasetActivationReview {
        source_sha256: review.source_sha256.clone(),
        updated_at: "stale".into(),
    };
    assert_eq!(
        activate_on_connection(&mut c, "ws", &normalized.id, Some(&stale)).unwrap_err(),
        "dataset_activation_review_changed"
    );
    // Check again under the same IMMEDIATE transaction that changes readiness.
    assert_eq!(
        datasets::activate_version(
            &mut c,
            "ws",
            &normalized.id,
            Some((&review.source_sha256, "stale"))
        )
        .unwrap_err(),
        "dataset_activation_review_changed"
    );
    let normalized_bytes = std::fs::read(&normalized.source_path).unwrap();
    std::fs::write(&normalized.source_path, "tampered synthetic fixture").unwrap();
    assert!(
        activate_on_connection(&mut c, "ws", &normalized.id, Some(&review))
            .unwrap_err()
            .contains("source changed")
    );
    assert_eq!(
        datasets::get(&c, "ws", &normalized.id)
            .unwrap()
            .unwrap()
            .mapping_state,
        "draft"
    );
    std::fs::write(&normalized.source_path, normalized_bytes).unwrap();
    let activated = activate_on_connection(&mut c, "ws", &normalized.id, Some(&review)).unwrap();
    assert_eq!(activated.mapping_state, "ready");
    assert_eq!(activated.source_sha256, normalized.source_sha256);
    assert_eq!(
        activate_on_connection(&mut c, "ws", &normalized.id, None)
            .unwrap()
            .mapping_state,
        "ready"
    );
    drop(c);
    let (c, _) = crate::storage::database::open(&database).unwrap();
    let reopened = datasets::get(&c, "ws", &normalized.id).unwrap().unwrap();
    assert_eq!(reopened.mapping_state, "ready");
    assert_eq!(
        datasets::get(&c, "ws", &original.id)
            .unwrap()
            .unwrap()
            .mapping_state,
        "draft"
    );
    assert_eq!(std::fs::read(&path).unwrap(), raw.as_bytes());
    assert_eq!(crate::steel::hash_dataset_source(&source).unwrap(), hash);
    for (id, body) in frozen {
        assert_eq!(
            serde_json::to_value(artifacts::get(&c, "ws", &id).unwrap()).unwrap(),
            body
        );
    }
    let table = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
        source_path: reopened.source_path.clone(),
        sheet: None,
    })
    .unwrap();
    let payload = crate::app::compute_commands::logic::build_linear_regression_payload(
        &table,
        &reopened.columns,
        &crate::app::compute_commands::logic::TrainSteelDatasetRequest {
            dataset_id: reopened.id,
            feature_columns: vec![1, 0],
            target_column: 2,
            split_policy: None,
            algorithm: None,
        },
    )
    .unwrap();
    assert_eq!(
        payload["feature_names"],
        json!(["process.quench_temperature", "chem.c"])
    );
    assert!((payload["features"][0][0].as_f64().unwrap() - 900.).abs() < 1e-9);
    assert_eq!(payload["features"][0][1], 0.4);
    assert_eq!(payload["feature_metadata"][0]["unit"], "degC");
    assert_eq!(payload["feature_metadata"][1]["unit"], "wt%");
    assert_eq!(
        payload["target_schema"],
        json!({"name":"property.hardness_hv", "unit":"HV"})
    );
    assert!(reopened.columns[4].canonical_field.is_none());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM material_mapping_versions", [], |r| {
            r.get::<_, u32>(0)
        })
        .unwrap(),
        1
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
