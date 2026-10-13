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
