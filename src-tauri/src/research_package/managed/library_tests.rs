use super::*;
use crate::research_package::projection::Projection;
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "suna-isolation-library-中文-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    root
}
fn archive_bytes() -> Vec<u8> {
    let (metadata, asset, body) = tests::simple();
    let original = archive::prepare(metadata, vec![(asset, body)], true)
        .unwrap()
        .into_bytes();
    // A valid non-canonical ZIP must reopen by its original byte hash; rebuilding
    // a Stored archive would produce a different digest.
    let mut reader = zip::ZipArchive::new(Cursor::new(original)).unwrap();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..reader.len() {
        let mut file = reader.by_index(i).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        writer
            .start_file(
                file.name(),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(&data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn save(root: &std::path::Path, bytes: &[u8]) -> Value {
    staging::import_with_archive(
        &archive::inspect(bytes).unwrap(),
        bytes,
        root,
        &digest(bytes),
    )
    .unwrap()
}

#[test]
fn original_archive_reopens_without_the_external_source_and_list_never_claims_verified() {
    let root = temp_root();
    let bytes = archive_bytes();
    let external = root.join("临时外部 合成.zip");
    std::fs::write(&external, &bytes).unwrap();
    let keep = root.join("existing-user-file.txt");
    std::fs::write(&keep, b"synthetic-unchanged").unwrap();
    assert_eq!(library::list(&root, 0).unwrap()["items"], json!([]));
    assert_eq!(
        staging::import_with_archive(
            &archive::inspect(&bytes).unwrap(),
            b"different archive",
            &root,
            &digest(&bytes)
        )
        .unwrap_err(),
        "package_hash_mismatch"
    );
    assert!(!root.join("research-package-imports").exists());
    assert_eq!(
        staging::import_failing_with_archive(&archive::inspect(&bytes).unwrap(), &bytes, &root)
            .unwrap_err(),
        "synthetic_import_failure"
    );
    assert!(!root
        .join("research-package-imports")
        .join(digest(&bytes))
        .exists());
    assert_eq!(library::list(&root, 0).unwrap()["items"], json!([]));
    let imported = save(
        &root,
        &crate::research_package::selected_file::read_bytes(&external, MAX_TOTAL).unwrap(),
    );
    std::fs::remove_file(external).unwrap();
    let list = library::list(&root, 0).unwrap();
    assert_eq!(list["items"][0]["state"], "stored_unverified");
    assert!(list["items"][0].get("reopenVerified").is_none());
    let result = library::open(&root, &digest(&bytes), 0).unwrap();
    assert_eq!(result["isolation"], imported);
    assert_eq!(result["isolation"]["originalArchiveStored"], true);
    assert_eq!(result["records"].as_array().unwrap().len(), 2);
    for record in result["records"].as_array().unwrap() {
        assert_eq!(
            record["importedId"],
            imported["idMapping"][record["originalId"].as_str().unwrap()]
        );
        assert_eq!(
            digest(record["record"].to_string().as_bytes()),
            record["recordSha256"]
        );
    }
    assert_eq!(library::open(&root, &digest(&bytes), 0).unwrap(), result);
    assert_eq!(std::fs::read(&keep).unwrap(), b"synthetic-unchanged");
    assert!(library::open(&root, "../existing-user-file.txt", 0).is_err());
    assert!(library::open(&root, &digest(&bytes), 1).is_err());
    assert!(library::list(&root, 10025).is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_receipts_remain_byte_identical_and_require_the_original_package() {
    let root = temp_root();
    let bytes = archive_bytes();
    let id = digest(&bytes);
    let package = archive::inspect(&bytes).unwrap();
    staging::import(&package, &root, &id).unwrap();
    let folder = root.join("research-package-imports").join(&id);
    let path = folder.join("receipt.json");
    let mut receipt: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    receipt
        .as_object_mut()
        .unwrap()
        .remove("originalArchiveStored");
    let original = serde_json::to_vec(&receipt).unwrap();
    std::fs::write(&path, &original).unwrap();
    assert_eq!(
        library::list(&root, 0).unwrap()["items"][0]["state"],
        "requires_original_archive"
    );
    assert_eq!(
        library::open(&root, &id, 0).unwrap_err(),
        "package_isolation_original_required"
    );
    let checked = staging::import_with_archive(&package, &bytes, &root, &id).unwrap();
    assert_eq!(checked["originalArchiveStored"], false);
    assert!(!folder.join("package.zip").exists());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupted_archive_asset_database_or_mapping_never_reopens_or_overwrites_evidence() {
    for which in ["archive", "asset", "database", "mapping"] {
        let root = temp_root();
        let bytes = archive_bytes();
        let id = digest(&bytes);
        save(&root, &bytes);
        let package = archive::inspect(&bytes).unwrap();
        let folder = root.join("research-package-imports").join(&id);
        let path = match which {
            "archive" => folder.join("package.zip"),
            "asset" => folder.join(package.assets[0].0.name()),
            "database" => folder.join("metadata.sqlite3"),
            _ => folder.join("receipt.json"),
        };
        let mut changed = std::fs::read(&path).unwrap();
        if which == "mapping" {
            let mut receipt: Value = serde_json::from_slice(&changed).unwrap();
            let key = receipt["idMapping"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            receipt["idMapping"][key] = json!(uuid::Uuid::new_v4());
            changed = serde_json::to_vec(&receipt).unwrap();
        } else {
            changed[0] ^= 1;
        }
        std::fs::write(&path, &changed).unwrap();
        assert!(library::open(&root, &id, 0).is_err(), "{which}");
        assert!(save_error(&root, &bytes).is_some(), "{which}");
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        std::fs::remove_dir_all(root).unwrap();
    }
}
fn save_error(root: &std::path::Path, bytes: &[u8]) -> Option<String> {
    staging::import_with_archive(
        &archive::inspect(bytes).unwrap(),
        bytes,
        root,
        &digest(bytes),
    )
    .err()
}

#[test]
fn independent_metadata_and_isolation_pages_preserve_all_ids_without_truncation() {
    let root = temp_root();
    let (metadata, asset, body) = tests::simple();
    let source = crate::research_package::archive::inspect(&metadata).unwrap();
    let mut records: Vec<_> = source.objects.iter().map(|(_, r, _)| r.clone()).collect();
    for i in 0..26 {
        records.push(
            json!({"id":uuid::Uuid::new_v4(),"kind":"dataset","name":format!("合成引用{i}")}),
        );
    }
    let metadata = crate::research_package::archive::export(Projection {
        project_id: source.source_project_id,
        records,
        dependencies: vec![],
        edges: vec![],
        app_version: "synthetic-pages".into(),
    })
    .unwrap();
    let bytes = archive::prepare(metadata, vec![(asset, body)], true)
        .unwrap()
        .into_bytes();
    save(&root, &bytes);
    let first = library::open(&root, &digest(&bytes), 0).unwrap();
    let next = library::open(&root, &digest(&bytes), 25).unwrap();
    assert_eq!(first["total"], 28);
    assert_eq!(first["hasMore"], true);
    assert_eq!(next["hasMore"], false);
    assert_eq!(first["records"].as_array().unwrap().len(), 25);
    assert_eq!(next["records"].as_array().unwrap().len(), 3);
    let ids: std::collections::BTreeSet<_> = first["records"]
        .as_array()
        .unwrap()
        .iter()
        .chain(next["records"].as_array().unwrap())
        .map(|r| r["originalId"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 28);
    let parent = root.join("research-package-imports");
    for i in 0..26 {
        std::fs::create_dir(parent.join(format!("{i:064x}"))).unwrap();
    }
    let first = library::list(&root, 0).unwrap();
    let next = library::list(&root, 25).unwrap();
    assert_eq!(first["total"], 27);
    assert_eq!(first["hasMore"], true);
    assert_eq!(next["hasMore"], false);
    assert_eq!(first["items"].as_array().unwrap().len(), 25);
    assert_eq!(next["items"].as_array().unwrap().len(), 2);
    assert!(first["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["state"] == "incomplete_or_damaged"));
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn linked_isolations_or_archives_cannot_read_outside_the_managed_namespace() {
    use std::os::unix::fs::symlink;
    let root = temp_root();
    let outside = temp_root();
    let bytes = archive_bytes();
    let id = digest(&bytes);
    save(&root, &bytes);
    let parent = root.join("research-package-imports");
    let folder = parent.join(&id);
    let original = outside.join("original.zip");
    std::fs::rename(folder.join("package.zip"), &original).unwrap();
    symlink(&original, folder.join("package.zip")).unwrap();
    assert!(library::open(&root, &id, 0).is_err());
    std::fs::remove_file(folder.join("package.zip")).unwrap();
    std::fs::rename(&original, folder.join("package.zip")).unwrap();
    std::fs::rename(&folder, outside.join("held")).unwrap();
    symlink(outside.join("held"), &folder).unwrap();
    assert_eq!(
        library::list(&root, 0).unwrap()["items"][0]["state"],
        "incomplete_or_damaged"
    );
    assert!(library::open(&root, &id, 0).is_err());
    std::fs::remove_file(&folder).unwrap();
    std::fs::remove_dir(&parent).unwrap();
    symlink(&outside, &parent).unwrap();
    assert!(library::list(&root, 0).is_err());
    std::fs::remove_file(parent).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_dir_all(outside).unwrap();
}

#[test]
fn shared_physical_bytes_preserve_two_logical_asset_links_on_import_and_reopen() {
    let (metadata, first, body) = tests::simple();
    let original = crate::research_package::archive::inspect(&metadata).unwrap();
    let mut second = first.clone();
    second.resource_id = uuid::Uuid::new_v4().to_string();
    let metadata = crate::research_package::archive::export(Projection {
        project_id: original.source_project_id.clone(),
        records: vec![
            json!({"id":original.source_project_id,"kind":"project"}),
            json!({"id":first.resource_id,"kind":"dataset"}),
            json!({"id":second.resource_id,"kind":"dataset"}),
        ],
        dependencies: vec![first.clone(), second.clone()].into_iter().map(|a|json!({"resource_id":a.resource_id,"kind":"dataset","status":"missing","expected_sha256":a.sha256})).collect(),
        edges: vec![],app_version:"synthetic-shared-file".into(),
    }).unwrap();
    let bytes = archive::prepare(
        metadata,
        vec![
            (first.clone(), body.clone()),
            (second.clone(), body.clone()),
        ],
        true,
    )
    .unwrap()
    .into_bytes();
    let package = archive::inspect(&bytes).unwrap();
    assert_eq!(package.summary()["assetCount"], 2);
    let root =
        std::env::temp_dir().join(format!("suna-shared-assets-中文-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let imported = staging::import(&package, &root, &digest(&bytes)).unwrap();
    let again = staging::import(&package, &root, &digest(&bytes)).unwrap();
    assert_eq!(imported, again);
    assert_ne!(
        imported["idMapping"][&first.resource_id],
        imported["idMapping"][&second.resource_id]
    );
    let folder = root.join("research-package-imports").join(digest(&bytes));
    assert_eq!(std::fs::read_dir(folder.join("assets")).unwrap().count(), 1);
    assert_eq!(std::fs::read(folder.join(first.name())).unwrap(), body);
    let c = rusqlite::Connection::open_with_flags(
        folder.join("metadata.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM package_assets", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 2);
    c.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
