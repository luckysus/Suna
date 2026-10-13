use super::*;
fn fixture() -> (Connection, std::path::PathBuf, String, String) {
    let root = std::env::temp_dir().join(format!("suna-工业合成-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "ws",
        projects::SaveResearchProject {
            id: None,
            name: "工业合成验证".into(),
            description: "".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let id = Uuid::new_v4();
    connections::save(
        &mut c,
        "ws",
        &connections::DatabaseConnectionRecord {
            id,
            display_name: "隔离连接夹具（未联网）".into(),
            host: "127.0.0.1".into(),
            port: 1433,
            database_name: "fixture".into(),
            username: "readonly_fixture".into(),
            timeout_ms: 1000,
            enabled: true,
            last_checked_at: None,
            last_latency_ms: None,
            last_version: None,
            last_error: None,
        },
    )
    .unwrap();
    (c, root, p.id, id.to_string())
}
fn profile(connection: &str) -> ReadProfile {
    ReadProfile {
        connection_id: connection.into(),
        database: "fixture".into(),
        table: "dbo.heats".into(),
        columns: vec!["C".into(), "HV".into(), "batch".into()],
        batch_column: "batch".into(),
        row_limit: 100,
    }
}
#[test]
fn templates_are_parameterized_whitelisted_immutable_and_workspace_scoped() {
    let (c, root, p, connection) = fixture();
    let profile_id = save_profile(&c, "ws", profile(&connection)).unwrap();
    let injection = "B'; DROP TABLE heats--";
    let task = submit_read(&c, "ws", &p, &profile_id, injection).unwrap();
    let payload: Value = serde_json::from_str(&task.payload_json).unwrap();
    assert_eq!(
        payload["sql"],
        "SELECT [C],[HV],[batch] FROM [dbo].[heats] WHERE [batch] = @P1"
    );
    assert!(!payload["sql"].as_str().unwrap().contains(injection));
    assert_eq!(payload["parameters"][0], injection);
    assert!(submit_read(&c, "other", &p, &profile_id, "b").is_err());
    assert!(c
        .execute("UPDATE database_read_profiles SET profile_json='{}'", [])
        .is_err());
    let mut bad = profile(&connection);
    bad.table = "dbo.heats;DROP TABLE x".into();
    assert!(prepare(&bad).is_err());
    bad = profile(&connection);
    bad.batch_column = "not_whitelisted".into();
    assert!(prepare(&bad).is_err());
    bad = profile(&connection);
    bad.row_limit = 10001;
    assert!(prepare(&bad).is_err());
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn file_exchange_creates_real_snapshot_with_simulation_identity_without_software_license() {
    let (c, root, p, _) = fixture();
    let path = root.join("合成软件导出.json");
    let bytes=b"[{\"temperature\":800,\"phase_fraction\":0.2},{\"temperature\":900,\"phase_fraction\":0.3}]";
    fs::write(&path, bytes).unwrap();
    let request = FileImport {
        project_id: p.clone(),
        source_path: path.to_string_lossy().into(),
        adapter_id: "custom_file".into(),
        software_version: "synthetic-fixture-v1".into(),
        rights_basis: "synthetic local test data".into(),
        rights_confirmed: true,
        batch: "synthetic-A".into(),
        input_conditions: json!({"chem.C":{"value":0.3,"unit":"wt%"}}),
        output_units: std::collections::BTreeMap::from([
            ("temperature".into(), "degC".into()),
            ("phase_fraction".into(), "fraction".into()),
        ]),
        mappings: vec![],
    };
    let d = file_import(&c, "ws", &root, request).unwrap();
    assert_eq!(d.row_count, 2);
    assert_eq!(d.mapping_state, "draft");
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read(&d.source_path).unwrap(), bytes);
    let snapshot: String = c
        .query_row(
            "SELECT metadata_json FROM material_source_snapshots",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(snapshot.contains("external_simulation"));
    assert!(!snapshot.contains(root.to_str().unwrap()));
    assert_eq!(artifacts::edges(&c, "ws", &p, 25, 0).unwrap().len(), 1);
    assert!(c
        .execute("DELETE FROM material_source_snapshots", [])
        .is_err());
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn completed_query_cache_creates_new_dataset_and_preserves_query_parameters_without_credentials() {
    let (c, root, p, connection) = fixture();
    let spec = profile(&connection);
    let profile_id = save_profile(&c, "ws", spec).unwrap();
    let task = submit_read(&c, "ws", &p, &profile_id, "synthetic-batch").unwrap();
    let payload: Value = serde_json::from_str(&task.payload_json).unwrap();
    let record = results::QueryResultRecord {
        task_id: task.id,
        connection_id: Uuid::parse_str(&connection).unwrap(),
        database_name: "fixture".into(),
        query_text: payload["sql"].as_str().unwrap().into(),
        row_count: 2,
        truncated: false,
        duration_ms: 1,
        csv_path: "unused fixture path".into(),
        columns: vec!["C".into(), "HV".into(), "batch".into()],
        rows: vec![
            vec![
                Some("0.3".into()),
                Some("500".into()),
                Some("synthetic-batch".into()),
            ],
            vec![
                Some("0.4".into()),
                Some("550".into()),
                Some("synthetic-batch".into()),
            ],
        ],
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    results::insert(&c, "ws", &record).unwrap();
    c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json='{\"stage\":\"completed\"}' WHERE id=?1",[task.id.to_string()]).unwrap();
    artifacts::reconcile_task(&c, "ws", &task.id.to_string()).unwrap();
    let d = import_query(&c, "ws", &p, &root, &task.id.to_string()).unwrap();
    assert_eq!(d.row_count, 2);
    let text = c
        .query_row(
            "SELECT metadata_json FROM material_source_snapshots",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap();
    assert!(text.contains("queryTemplateSha256"));
    assert!(text.contains("synthetic-batch"));
    assert!(!text.contains("readonly_fixture"));
    assert!(import_query(&c, "other", &p, &root, &task.id.to_string()).is_err());
    assert!(import_query(&c, "ws", &p, &root, &task.id.to_string()).is_err());
    assert_eq!(steel::list(&c, "ws").unwrap().len(), 1);
    drop(c);
    let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    assert_eq!(steel::list(&c, "ws").unwrap().len(), 1);
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
