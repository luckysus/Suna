//! A separate on-disk isolation store. Never accepts a live application connection.
use super::*;
use crate::permissions::path::AuthorizedRoots;
use rusqlite::{params, Connection};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) fn ordinary_directory(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
fn create_private(path: &Path) -> std::io::Result<()> {
    #[allow(unused_mut)]
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}
struct Rollback {
    path: PathBuf,
    committed: bool,
}
impl Drop for Rollback {
    fn drop(&mut self) {
        if !self.committed && ordinary_directory(&self.path) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
fn write_new(root: &Path, relative: &str, bytes: &[u8]) -> Result<(), String> {
    let roots = AuthorizedRoots::new(vec![root.to_path_buf()])
        .map_err(|_| "package_isolation_path_invalid")?;
    let path = root.join(relative);
    let authorized = roots
        .authorize(&path)
        .map_err(|_| "package_isolation_path_invalid")?;
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(authorized.canonical_path())
        .map_err(|_| "package_isolation_write_failed")?;
    #[cfg(windows)]
    roots
        .verify_opened_file(&authorized, &file)
        .map_err(|_| "package_isolation_path_invalid")?;
    roots
        .verify_target(&authorized, &path)
        .map_err(|_| "package_isolation_path_invalid")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "package_isolation_write_failed".to_string())
}
pub(super) fn read_owned(root: &Path, relative: &str, budget: usize) -> Result<Vec<u8>, String> {
    let roots = AuthorizedRoots::new(vec![root.to_path_buf()])
        .map_err(|_| "package_isolation_path_invalid")?;
    let path = root.join(relative);
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| "package_isolation_incomplete")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > budget as u64 {
        return Err("package_isolation_path_invalid".into());
    }
    let authorized = roots
        .authorize(&path)
        .map_err(|_| "package_isolation_path_invalid")?;
    let mut file =
        File::open(authorized.canonical_path()).map_err(|_| "package_isolation_read_failed")?;
    #[cfg(windows)]
    roots
        .verify_opened_file(&authorized, &file)
        .map_err(|_| "package_isolation_path_invalid")?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(budget as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "package_isolation_read_failed")?;
    if bytes.len() > budget {
        return Err("package_size_budget".into());
    }
    Ok(bytes)
}

pub(super) fn verify_existing(package: &Verified, root: &Path) -> Result<Value, String> {
    verify_existing_with(package, root, || {})
}
pub(super) fn verify_existing_with(
    package: &Verified,
    root: &Path,
    after_database_read: impl FnOnce(),
) -> Result<Value, String> {
    if !ordinary_directory(root) || !ordinary_directory(&root.join("assets")) {
        return Err("package_isolation_path_invalid".into());
    }
    let receipt = crate::research_package::strict_json::decode(&read_owned(
        root,
        "receipt.json",
        super::super::MAX_FILE,
    )?)?;
    if receipt["schemaVersion"] != "research-managed-isolation.v1"
        || receipt["packageSha256"] != package.package_sha256
        || receipt["sourceProjectId"] != package.metadata.source_project_id
    {
        return Err("package_isolation_conflict".into());
    }
    let archive_stored = match receipt.get("originalArchiveStored") {
        None => false, // Existing isolation receipts remain unchanged.
        Some(value) => value.as_bool().ok_or("package_isolation_conflict")?,
    };
    if archive_stored
        && digest(&read_owned(root, "package.zip", MAX_TOTAL)?) != package.package_sha256
    {
        return Err("package_isolation_conflict".into());
    }
    if read_owned(root, "metadata.zip", super::super::MAX_TOTAL)? != package.metadata_bytes {
        return Err("package_isolation_conflict".into());
    }
    for (asset, _) in &package.assets {
        archive::validate_bytes(asset, &read_owned(root, &asset.name(), MAX_ASSET)?)?;
    }
    let db = read_owned(root, "metadata.sqlite3", MAX_TOTAL)?;
    if receipt["databaseSha256"] != digest(&db) {
        return Err("package_isolation_conflict".into());
    }
    after_database_read();
    let mapping = snapshot::verify(package, &db)?;
    if receipt["idMapping"] != json!(mapping) {
        return Err("package_isolation_conflict".into());
    }
    let mut result = package.summary();
    result["isolationId"] = json!(package.package_sha256);
    result["idMapping"] = json!(mapping);
    result["status"] = json!("isolated_assets_stored");
    result["assetsRestoredToIsolation"] = json!(true);
    result["assetsRestoredToLiveWorkspace"] = json!(false);
    result["readyForComputation"] = json!(false);
    result["reopenVerified"] = json!(true);
    result["originalArchiveStored"] = json!(archive_stored);
    Ok(result)
}

pub fn import(
    package: &Verified,
    data_root: &Path,
    expected_sha256: &str,
) -> Result<Value, String> {
    import_with(package, None, data_root, expected_sha256, || Ok(()))
}
/// Retain the host-selected archive for independent read-only reopening later.
pub fn import_with_archive(
    package: &Verified,
    bytes: &[u8],
    data_root: &Path,
    expected_sha256: &str,
) -> Result<Value, String> {
    import_with(package, Some(bytes), data_root, expected_sha256, || Ok(()))
}
fn import_with(
    package: &Verified,
    original_archive: Option<&[u8]>,
    data_root: &Path,
    expected_sha256: &str,
    after_assets: impl FnOnce() -> Result<(), String>,
) -> Result<Value, String> {
    if !valid_sha(expected_sha256) || package.package_sha256 != expected_sha256 {
        return Err("package_preview_changed".into());
    }
    if original_archive
        .is_some_and(|bytes| bytes.len() > MAX_TOTAL || digest(bytes) != package.package_sha256)
    {
        return Err("package_hash_mismatch".into());
    }
    let guard = AuthorizedRoots::new(vec![data_root.to_path_buf()])
        .map_err(|_| "package_isolation_path_invalid")?;
    let parent = data_root.join("research-package-imports");
    guard
        .authorize(&parent)
        .map_err(|_| "package_isolation_path_invalid")?;
    if let Err(error) = create_private(&parent) {
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err("package_isolation_write_failed".into());
        }
    }
    if !ordinary_directory(&parent) {
        return Err("package_isolation_path_invalid".into());
    }
    guard
        .authorize(&parent)
        .map_err(|_| "package_isolation_path_invalid")?;
    let root = parent.join(&package.package_sha256);
    if let Err(error) = create_private(&root) {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            return verify_existing(package, &root);
        }
        return Err("package_isolation_write_failed".into());
    }
    let mut rollback = Rollback {
        path: root.clone(),
        committed: false,
    };
    create_private(&root.join("assets")).map_err(|_| "package_isolation_write_failed")?;
    if let Some(bytes) = original_archive {
        write_new(&root, "package.zip", bytes)?;
    }
    write_new(&root, "metadata.zip", &package.metadata_bytes)?;
    let mut written = BTreeSet::new();
    for (asset, bytes) in &package.assets {
        if written.insert(asset.name()) {
            write_new(&root, &asset.name(), bytes)?;
        }
    }
    after_assets()?;
    let mut c = Connection::open(root.join("metadata.sqlite3"))
        .map_err(|_| "package_isolation_storage_error")?;
    c.execute_batch("PRAGMA foreign_keys=ON;")
        .map_err(|_| "package_isolation_storage_error")?;
    let tx = c
        .transaction()
        .map_err(|_| "package_isolation_storage_error")?;
    tx.execute_batch("CREATE TABLE package_objects(original_id TEXT PRIMARY KEY,id TEXT UNIQUE NOT NULL,body_json TEXT NOT NULL,source_sha256 TEXT NOT NULL);CREATE TABLE package_assets(resource_id TEXT PRIMARY KEY REFERENCES package_objects(original_id),name TEXT NOT NULL,sha256 TEXT NOT NULL,bytes INTEGER NOT NULL,role TEXT NOT NULL);CREATE TABLE package_edges(from_id TEXT REFERENCES package_objects(id),to_id TEXT REFERENCES package_objects(id),relation TEXT,PRIMARY KEY(from_id,to_id,relation));").map_err(|_|"package_isolation_storage_error")?;
    let mut mapping = BTreeMap::new();
    for (original, body, sha) in &package.metadata.objects {
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO package_objects VALUES(?1,?2,?3,?4)",
            params![original, id, body.to_string(), sha],
        )
        .map_err(|_| "package_isolation_storage_error")?;
        mapping.insert(original.clone(), id);
    }
    for (asset, _) in &package.assets {
        tx.execute(
            "INSERT INTO package_assets VALUES(?1,?2,?3,?4,?5)",
            params![
                asset.resource_id,
                asset.name(),
                asset.sha256,
                asset.bytes,
                asset.role
            ],
        )
        .map_err(|_| "package_isolation_storage_error")?;
    }
    for edge in &package.metadata.edges {
        let from = edge["from_id"].as_str().ok_or("package_edge_invalid")?;
        let to = edge["to_id"].as_str().ok_or("package_edge_invalid")?;
        tx.execute(
            "INSERT INTO package_edges VALUES(?1,?2,?3)",
            params![mapping[from], mapping[to], edge["relation"].as_str()],
        )
        .map_err(|_| "package_isolation_storage_error")?;
    }
    tx.commit().map_err(|_| "package_isolation_storage_error")?;
    c.close().map_err(|_| "package_isolation_storage_error")?;
    let db = read_owned(&root, "metadata.sqlite3", MAX_TOTAL)?;
    let receipt = json!({"schemaVersion":"research-managed-isolation.v1","packageSha256":package.package_sha256,"sourceProjectId":package.metadata.source_project_id,"databaseSha256":digest(&db),"idMapping":mapping,"originalArchiveStored":original_archive.is_some()});
    write_new(
        &root,
        "receipt.json",
        &serde_json::to_vec(&receipt).map_err(|_| "package_json_invalid")?,
    )?;
    let result = verify_existing(package, &root)?;
    rollback.committed = true;
    Ok(result)
}

#[cfg(test)]
pub(super) fn import_failing_after_assets(
    package: &Verified,
    root: &Path,
) -> Result<Value, String> {
    import_with(package, None, root, &package.package_sha256, || {
        Err("synthetic_import_failure".into())
    })
}

#[cfg(test)]
pub(super) fn import_failing_with_archive(
    package: &Verified,
    bytes: &[u8],
    root: &Path,
) -> Result<Value, String> {
    import_with(package, Some(bytes), root, &package.package_sha256, || {
        Err("synthetic_import_failure".into())
    })
}
