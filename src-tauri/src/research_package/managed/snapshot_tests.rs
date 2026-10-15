use super::*;
use crate::research_package::projection::Projection;
use std::path::{Path, PathBuf};

fn fixture() -> (PathBuf, Vec<u8>, Verified) {
    let (metadata, asset, body) = tests::simple();
    let original = crate::research_package::archive::inspect(&metadata).unwrap();
    let metadata = crate::research_package::archive::export(Projection {
        project_id: original.source_project_id.clone(),
        records: original.objects.iter().map(|(_, value, _)| value.clone()).collect(),
        dependencies: vec![json!({"resource_id":asset.resource_id,"kind":"dataset","status":"missing","expected_sha256":asset.sha256})],
        edges: vec![json!({"from_id":original.source_project_id,"to_id":asset.resource_id,"relation":"generated_by_run"})],
        app_version: "synthetic-snapshot-verification".into(),
    }).unwrap();
    let bytes = archive::prepare(metadata, vec![(asset, body)], true)
        .unwrap()
        .into_bytes();
    let package = archive::inspect(&bytes).unwrap();
    let root = std::env::temp_dir().join(format!(
        "suna-isolation-snapshot-中文-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    staging::import_with_archive(&package, &bytes, &root, &package.package_sha256).unwrap();
    (root, bytes, package)
}
fn folder(root: &Path, package: &Verified) -> PathBuf {
    root.join("research-package-imports")
        .join(&package.package_sha256)
}
fn refresh_receipt(folder: &Path) {
    let path = folder.join("receipt.json");
    let mut receipt: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    receipt["databaseSha256"] = json!(digest(
        &std::fs::read(folder.join("metadata.sqlite3")).unwrap()
    ));
    std::fs::write(path, serde_json::to_vec(&receipt).unwrap()).unwrap();
}
#[test]
fn rewritten_receipt_hash_cannot_authenticate_forged_asset_rows_with_the_same_count() {
    for (column, value) in [
        ("role", "forged-role"),
        ("name", "assets/forged.csv"),
        ("sha256", "0"),
        ("resource_id", "forged-resource"),
        ("bytes", "999"),
    ] {
        let (root, bytes, package) = fixture();
        let folder = folder(&root, &package);
        let c = rusqlite::Connection::open(folder.join("metadata.sqlite3")).unwrap();
        // Simulate an offline editor; production foreign-key enforcement stays enabled.
        c.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        c.execute(&format!("UPDATE package_assets SET {column}=?1"), [value])
            .unwrap();
        c.close().unwrap();
        refresh_receipt(&folder);
        let changed = std::fs::read(folder.join("metadata.sqlite3")).unwrap();
        assert!(
            library::open(&root, &package.package_sha256, 0).is_err(),
            "forged {column} accepted"
        );
        assert!(
            staging::import_with_archive(&package, &bytes, &root, &package.package_sha256).is_err()
        );
        assert_eq!(
            std::fs::read(folder.join("metadata.sqlite3")).unwrap(),
            changed
        );
        assert_eq!(std::fs::read(folder.join("package.zip")).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn rewritten_receipt_hash_cannot_authenticate_forged_lineage_with_the_same_count() {
    for (column, value) in [
        ("relation", "forged_relation"),
        ("from_id", "forged-from"),
        ("to_id", "forged-to"),
    ] {
        let (root, bytes, package) = fixture();
        let folder = folder(&root, &package);
        let c = rusqlite::Connection::open(folder.join("metadata.sqlite3")).unwrap();
        // Simulate an offline editor; production foreign-key enforcement stays enabled.
        c.execute_batch("PRAGMA foreign_keys=OFF;").unwrap();
        c.execute(&format!("UPDATE package_edges SET {column}=?1"), [value])
            .unwrap();
        c.close().unwrap();
        refresh_receipt(&folder);
        assert!(
            library::open(&root, &package.package_sha256, 0).is_err(),
            "forged {column} accepted"
        );
        assert!(
            staging::import_with_archive(&package, &bytes, &root, &package.package_sha256).is_err()
        );
        assert_eq!(std::fs::read(folder.join("package.zip")).unwrap(), bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn verification_uses_the_hashed_database_snapshot_instead_of_reopening_a_replaced_path() {
    let (root, bytes, package) = fixture();
    let directory = folder(&root, &package);
    let result = staging::verify_existing_with(&package, &directory, || {
        std::fs::rename(
            directory.join("metadata.sqlite3"),
            directory.join("held.sqlite3"),
        )
        .unwrap();
        std::fs::write(
            directory.join("metadata.sqlite3"),
            b"synthetic replacement, never a trusted database",
        )
        .unwrap();
    });
    assert!(
        result.is_ok(),
        "the already verified snapshot must be the only SQLite input: {:?}",
        result.err()
    );
    // A future read detects the replacement, and neither file is repaired or overwritten.
    assert!(library::open(&root, &package.package_sha256, 0).is_err());
    assert_eq!(
        std::fs::read(directory.join("metadata.sqlite3")).unwrap(),
        b"synthetic replacement, never a trusted database"
    );
    assert_eq!(std::fs::read(directory.join("package.zip")).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn untrusted_schema_objects_never_execute_or_become_a_verified_isolation() {
    for sql in [
        "CREATE TABLE synthetic_private(value TEXT)",
        "CREATE VIEW synthetic_view AS SELECT * FROM package_objects",
        "CREATE TRIGGER synthetic_trigger BEFORE UPDATE ON package_objects BEGIN SELECT RAISE(ABORT,'not a package schema'); END",
        "ALTER TABLE package_assets RENAME TO held_assets; CREATE VIEW package_assets AS SELECT * FROM held_assets",
    ] {
        let (root, bytes, package)=fixture();let directory=folder(&root,&package);
        let c=rusqlite::Connection::open(directory.join("metadata.sqlite3")).unwrap();
        c.execute_batch(sql).unwrap();c.close().unwrap();refresh_receipt(&directory);
        assert!(library::open(&root,&package.package_sha256,0).is_err());
        assert!(staging::import_with_archive(&package,&bytes,&root,&package.package_sha256).is_err());
        assert_eq!(std::fs::read(directory.join("package.zip")).unwrap(),bytes);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn duplicate_json_keys_cannot_disguise_changed_object_bodies_after_receipt_rehash() {
    let (root, bytes, package) = fixture();
    let directory = folder(&root, &package);
    let c = rusqlite::Connection::open(directory.join("metadata.sqlite3")).unwrap();
    let (id, body): (String, String) = c
        .query_row(
            "SELECT original_id,body_json FROM package_objects LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let changed = format!("{{\"id\":\"forged\",{}", body.strip_prefix('{').unwrap());
    // Ordinary JSON last-key-wins parsing would hide the first identity.
    assert_eq!(
        serde_json::from_str::<Value>(&changed).unwrap(),
        serde_json::from_str::<Value>(&body).unwrap()
    );
    c.execute(
        "UPDATE package_objects SET body_json=?1 WHERE original_id=?2",
        rusqlite::params![changed, id],
    )
    .unwrap();
    c.close().unwrap();
    refresh_receipt(&directory);
    assert!(library::open(&root, &package.package_sha256, 0).is_err());
    assert!(
        staging::import_with_archive(&package, &bytes, &root, &package.package_sha256).is_err()
    );
    assert_eq!(std::fs::read(directory.join("package.zip")).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}
