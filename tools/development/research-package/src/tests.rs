use crate::*;
use serde_json::json;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, ZipWriter};
const PROJECT: &str = "11111111-1111-4111-8111-111111111111";
const A: &str = "22222222-2222-4222-8222-222222222222";
const B: &str = "33333333-3333-4333-8333-333333333333";
fn projection() -> projection::Projection {
    projection::Projection {
        project_id: PROJECT.into(),
        records: vec![
            json!({"id":PROJECT,"kind":"project","name":"中文合成项目"}),
            json!({"id":A,"kind":"artifact","name":"历史科研成果","source_artifact_sha256":"a".repeat(64)}),
            json!({"id":B,"kind":"model","name":"引用模型","version":1}),
        ],
        dependencies: vec![
            json!({"resource_id":B,"kind":"model","status":"missing","expected_sha256":"b".repeat(64)}),
        ],
        edges: vec![json!({"from_id":A,"to_id":B,"relation":"uses_model"})],
        app_version: "synthetic-test".into(),
    }
}
fn raw_package(
    records: Vec<(String, Vec<u8>)>,
    edges: Vec<Value>,
    extra: Option<(&str, &[u8])>,
    manifest_override: Option<Vec<u8>>,
) -> Vec<u8> {
    let entries = records
        .iter()
        .map(|(name, bytes)| json!({"name":name,"bytes":bytes.len(),"sha256":digest(bytes)}))
        .collect::<Vec<_>>();
    let manifest=manifest_override.unwrap_or_else(||serde_json::to_vec(&json!({"schema_version":VERSION,"scope":"metadata_only","source_project_id":PROJECT,"entries":entries,"external_dependencies":[],"edges":edges,"app_version":"synthetic-test"})).unwrap());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    writer.start_file("manifest.json", options).unwrap();
    writer.write_all(&manifest).unwrap();
    for (name, body) in records {
        writer.start_file(name, options).unwrap();
        writer.write_all(&body).unwrap();
    }
    if let Some((name, body)) = extra {
        writer.start_file(name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn bodies(p: projection::Projection) -> Vec<(String, Vec<u8>)> {
    p.records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                format!("records/{i:04}.json"),
                serde_json::to_vec(r).unwrap(),
            )
        })
        .collect()
}
#[test]
fn original_ids_hashes_units_and_chinese_labels_survive_metadata_roundtrip() {
    let bytes = archive::export(projection()).unwrap();
    let checked = archive::inspect(&bytes).unwrap();
    assert_eq!(checked.objects.len(), 3);
    assert_eq!(checked.edges.len(), 1);
    assert_eq!(checked.source_project_id, PROJECT);
    assert_eq!(
        checked.objects.iter().find(|(id, _, _)| id == A).unwrap().1["source_artifact_sha256"],
        "a".repeat(64)
    );
    assert!(String::from_utf8_lossy(&bytes).contains("中文合成项目"));
    let result = temporary::simulate_import(&checked).unwrap();
    assert_eq!(result["status"], "metadata_only_missing_assets");
    assert_eq!(result["assetsRestored"], false);
    assert_eq!(result["writesToLiveWorkspace"], false);
    assert_eq!(result["idempotenceVerified"], true);
    assert_ne!(result["idMapping"][PROJECT], PROJECT);
}
#[test]
fn unsafe_archive_names_and_unlisted_assets_are_rejected() {
    for name in [
        "../secret.json",
        "C:/data.json",
        "records/0000.json:stream",
        "records\\0000.json",
        "records/0000.json ",
        "records/CON.json",
        "records/中文.json",
        "records/0000.exe",
        "Records/0003.json",
    ] {
        let bytes = raw_package(bodies(projection()), vec![], Some((name, b"{}")), None);
        assert!(
            archive::inspect(&bytes).is_err(),
            "unsafe canonical entry accepted"
        );
    }
    let bytes = raw_package(
        bodies(projection()),
        vec![],
        Some(("records/0003.json", b"{}")),
        None,
    );
    assert!(archive::inspect(&bytes).is_err());
}
#[test]
fn symlink_zip_entry_is_rejected_before_metadata_is_read() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("manifest.json", SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"{}").unwrap();
    writer
        .add_symlink(
            "records/0000.json",
            "synthetic-secret",
            SimpleFileOptions::default(),
        )
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert_eq!(
        archive::inspect(&bytes).err().unwrap(),
        "package_zip_non_regular"
    );
}
#[test]
fn duplicate_json_keys_nested_depth_and_non_finite_values_cannot_pass() {
    for body in [br#"{"id":"11111111-1111-4111-8111-111111111111","id":"11111111-1111-4111-8111-111111111111","kind":"project"}"#.to_vec(),
        br#"{"id":"11111111-1111-4111-8111-111111111111","kind":"project","name":NaN}"#.to_vec(),
        format!("{}0{}","[".repeat(34),"]".repeat(34)).into_bytes()]{
        let bytes=raw_package(vec![("records/0000.json".into(),body)],vec![],None,None);assert!(archive::inspect(&bytes).is_err());
    }
    assert!(strict_json::decode(br#"{"x":{"y":1,"y":2}}"#).is_err());
    assert!(strict_json::decode(b"1e9999").is_err());
}
#[test]
fn digest_mismatch_and_unknown_schema_fail_before_simulation() {
    let records = bodies(projection());
    let entries = records
        .iter()
        .map(|(name, data)| json!({"name":name,"bytes":data.len(),"sha256":"0".repeat(64)}))
        .collect::<Vec<_>>();
    let manifest = json!({"schema_version":VERSION,"scope":"metadata_only","source_project_id":PROJECT,"entries":entries,"external_dependencies":[],"edges":[],"app_version":"synthetic-test"});
    let bytes = raw_package(
        records.clone(),
        vec![],
        None,
        Some(serde_json::to_vec(&manifest).unwrap()),
    );
    assert_eq!(
        archive::inspect(&bytes).err().unwrap(),
        "package_hash_mismatch"
    );
    let mut changed = manifest;
    changed["schema_version"] = json!("unknown-schema");
    assert!(archive::inspect(&raw_package(
        records,
        vec![],
        None,
        Some(serde_json::to_vec(&changed).unwrap())
    ))
    .is_err());
}
#[test]
fn sensitive_fields_and_binary_metadata_are_not_accepted() {
    for key in [
        "credentials",
        "password",
        "source_path",
        "absolute_path",
        "raw_rows",
        "model_pickle_base64",
        "unknown_nested",
    ] {
        let mut p = projection();
        p.records[0][key] = json!({"value":"synthetic-private-sentinel"});
        assert!(archive::export(p).is_err());
    }
}
#[test]
fn dangling_references_cycles_duplicate_resources_and_duplicate_edges_fail() {
    let mut p = projection();
    p.edges
        .push(json!({"from_id":B,"to_id":A,"relation":"uses_model"}));
    assert!(archive::export(p).is_err());
    let mut p = projection();
    p.records[1]["references"] = json!([B]);
    assert!(archive::export(p).is_err());
    let mut p = projection();
    p.edges[0]["to_id"] = json!("44444444-4444-4444-8444-444444444444");
    assert!(archive::export(p).is_err());
    let mut p = projection();
    p.records.push(p.records[1].clone());
    assert!(archive::export(p).is_err());
    let mut p = projection();
    p.edges.push(p.edges[0].clone());
    assert!(archive::export(p).is_err());
}
#[test]
fn high_compression_and_oversized_central_directory_are_bounded() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("manifest.json", options).unwrap();
    writer.write_all(b"{}").unwrap();
    writer.start_file("records/0000.json", options).unwrap();
    writer.write_all(&vec![b' '; 2 * 1024 * 1024]).unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    assert_eq!(
        archive::inspect(&bytes).err().unwrap(),
        "package_expansion_budget"
    );
    let mut records = Vec::new();
    for i in 0..2048 {
        records.push((format!("records/{i:04}.json"), b"{}".to_vec()));
    }
    assert_eq!(
        archive::inspect(&raw_package(records, vec![], None, None))
            .err()
            .unwrap(),
        "package_zip_envelope_invalid"
    );
}
#[test]
fn actual_thousand_object_project_export_preserves_frozen_hashes_without_raw_data() {
    let mut c = rusqlite::Connection::open_in_memory().unwrap();
    suna::storage::migrations::migrate(&mut c).unwrap();
    let project = suna::storage::repositories::research_projects::save(
        &mut c,
        "local",
        suna::storage::repositories::research_projects::SaveResearchProject {
            id: None,
            name: "千对象合成归档".into(),
            description: "synthetic-private-description".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let mut original = std::collections::BTreeMap::new();
    let mut previous: Option<String> = None;
    for i in 0..1100 {
        let id=suna::storage::repositories::research_artifacts::create(&c,"local",&project.id,"analysis_result","合成成果",&format!("synthetic-{i}"),None,
            json!({"source_path":"C:/synthetic-private/raw.csv","raw_rows":[987.123456],"api_key":"synthetic-secret"})).unwrap();
        if i < 5 {
            if let Some(from) = previous.as_deref() {
                suna::storage::repositories::research_artifacts::edge(
                    &c,
                    "local",
                    from,
                    &id,
                    "generated_by_run",
                )
                .unwrap();
            }
        }
        original.insert(
            id.clone(),
            suna::storage::repositories::research_artifacts::get(&c, "local", &id)
                .unwrap()
                .content_sha256,
        );
        previous = Some(id);
    }
    let before: i64 = c
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let p = projection::from_connection(&c, "local", &project.id).unwrap();
    assert_eq!(p.records.len(), 1101);
    let bytes = archive::export(p).unwrap();
    let checked = archive::inspect(&bytes).unwrap();
    assert_eq!(checked.objects.len(), 1101);
    assert_eq!(checked.edges.len(), 4);
    for (id, body, _) in &checked.objects {
        if let Some(sha) = original.get(id) {
            assert_eq!(body["source_artifact_sha256"], *sha);
        }
    }
    for sentinel in [
        "C:/synthetic-private/raw.csv",
        "synthetic-secret",
        "987.123456",
        "synthetic-private-description",
    ] {
        assert!(!String::from_utf8_lossy(&bytes).contains(sentinel));
    }
    let imported = temporary::simulate_import(&checked).unwrap();
    assert_eq!(imported["metadataRecordCount"], 1101);
    assert_eq!(imported["idempotenceVerified"], true);
    assert_eq!(
        c.query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert!(projection::from_connection(&c, "other-workspace", &project.id).is_err());
}
