use crate::permissions::path::{authorize_existing_file_with_handle, authorize_output_path};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::domains::{self, DomainTrustStore};

const FORMAT_VERSION: u32 = 2;
const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const DATABASE_ENTRY: &str = "suna.sqlite3";
const CONTENT_PREFIX: &str = "content/";
const CONTENT_DIRECTORIES: [&str; 5] = [
    "objects",
    "indexes",
    "microstructure",
    "material-datasets",
    "research-datasets",
];
const MAX_FILES: usize = 100_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifest {
    format_version: u32,
    created_at: String,
    database_entry: String,
    content_prefix: String,
    content_file_count: usize,
    #[serde(default)]
    content_hashes: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    managed_datasets: Vec<ManagedDataset>,
    #[serde(default)]
    database_sha256: Option<String>,
    #[serde(default)]
    external_dataset_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManagedDataset {
    workspace_id: String,
    dataset_id: String,
    relative_path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub format_version: u32,
    pub archive_path: String,
    pub database_bytes: u64,
    pub content_file_count: usize,
    pub content_bytes: u64,
    pub external_dataset_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArchiveStats {
    database_bytes: u64,
    content_file_count: usize,
    content_bytes: u64,
    total_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Manifest,
    Database,
    Content,
}

pub fn create_backup(
    connection: &Connection,
    database_path: &Path,
    content_root: &Path,
    archive_path: &Path,
) -> Result<BackupSummary, String> {
    let display_path = archive_path.to_path_buf();
    let authorized_archive = authorize_output_path(archive_path)
        .map_err(|error| format!("backup archive path is not authorized: {error}"))?;
    let archive_path = authorized_archive.canonical_path();
    if !database_path.is_file() {
        return Err("backup database does not exist".to_string());
    }
    if database_path == archive_path {
        return Err("backup destination must differ from the database".to_string());
    }
    if archive_path.exists() {
        return Err("backup destination already exists".to_string());
    }
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create backup directory failed: {error}"))?;
    }

    let files = collect_content_files(content_root, archive_path)?;
    let temporary_database = archive_path.with_extension(format!("sqlite3.tmp-{}", Uuid::new_v4()));
    let result = create_backup_archive(
        connection,
        &temporary_database,
        archive_path,
        &files,
        content_root,
    );
    let _ = fs::remove_file(&temporary_database);
    result.map(|mut summary| {
        summary.archive_path = display_path.to_string_lossy().into_owned();
        summary
    })
}

fn create_backup_archive(
    connection: &Connection,
    temporary_database: &Path,
    archive_path: &Path,
    files: &[(PathBuf, String, u64)],
    content_root: &Path,
) -> Result<BackupSummary, String> {
    connection
        .execute(
            "VACUUM INTO ?1",
            [temporary_database.to_string_lossy().as_ref()],
        )
        .map_err(|error| format!("snapshot database failed: {error}"))?;
    let database_bytes = fs::metadata(temporary_database)
        .map_err(|error| format!("read database snapshot failed: {error}"))?
        .len();
    let content_bytes = files.iter().map(|(_, _, size)| *size).sum::<u64>();
    let content_hashes = files
        .iter()
        .map(|(path, entry, _)| Ok((entry.clone(), file_sha(path)?)))
        .collect::<Result<std::collections::BTreeMap<_, _>, String>>()?;
    let snapshot_connection = Connection::open(temporary_database).map_err(|e| e.to_string())?;
    let (managed_datasets, external_dataset_count) =
        managed_dataset_sources(&snapshot_connection, content_root, &content_hashes)?;
    drop(snapshot_connection);
    let manifest = BackupManifest {
        format_version: FORMAT_VERSION,
        created_at: Utc::now().to_rfc3339(),
        database_entry: DATABASE_ENTRY.to_string(),
        content_prefix: CONTENT_PREFIX.to_string(),
        content_file_count: files.len(),
        content_hashes,
        managed_datasets,
        database_sha256: Some(file_sha(temporary_database)?),
        external_dataset_count: Some(external_dataset_count),
    };
    let archive_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(archive_path)
        .map_err(|error| format!("create backup archive failed: {error}"))?;
    let mut cleanup = BackupOutput {
        path: archive_path,
        completed: false,
    };
    let mut writer = ZipWriter::new(archive_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let manifest_json = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("backup_manifest_byte_limit".into());
    }
    writer
        .start_file("manifest.json", options)
        .map_err(|error| format!("write backup manifest failed: {error}"))?;
    writer
        .write_all(&manifest_json)
        .map_err(|error| format!("write backup manifest failed: {error}"))?;

    writer
        .start_file(DATABASE_ENTRY, options)
        .map_err(|error| format!("write database snapshot failed: {error}"))?;
    copy_file(
        &mut File::open(temporary_database).map_err(|error| error.to_string())?,
        &mut writer,
    )
    .map_err(|error| format!("write database snapshot failed: {error}"))?;
    for (path, entry, _) in files {
        writer
            .start_file(entry, options)
            .map_err(|error| format!("write backup content failed: {error}"))?;
        let (_, sha) = copy_hashed(
            &mut File::open(path).map_err(|error| error.to_string())?,
            &mut writer,
        )?;
        if manifest.content_hashes.get(entry) != Some(&sha) {
            return Err("backup_source_changed_during_export".into());
        }
    }
    writer
        .finish()
        .map_err(|error| format!("finish backup archive failed: {error}"))?;
    cleanup.completed = true;

    Ok(BackupSummary {
        format_version: FORMAT_VERSION,
        archive_path: archive_path.to_string_lossy().into_owned(),
        database_bytes,
        content_file_count: files.len(),
        content_bytes,
        external_dataset_count: manifest.external_dataset_count,
    })
}

pub fn preview_backup(archive_path: &Path) -> Result<BackupSummary, String> {
    let display_path = archive_path.to_path_buf();
    let (_, archive_file) = authorize_existing_file_with_handle(archive_path)
        .map_err(|error| format!("backup archive path is not authorized: {error}"))?;
    preview_backup_from_file(archive_file, &display_path)
}

fn preview_backup_from_file(
    archive_file: File,
    display_path: &Path,
) -> Result<BackupSummary, String> {
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("read backup archive failed: {error}"))?;
    let manifest = read_manifest(&mut archive)?;
    let stats = validate_archive(&mut archive, &manifest)?;
    if stats.total_bytes > MAX_UNCOMPRESSED_BYTES {
        return Err("backup exceeds the uncompressed size limit".to_string());
    }
    if stats.database_bytes == 0 {
        return Err("backup database is empty".to_string());
    }
    Ok(BackupSummary {
        format_version: manifest.format_version,
        archive_path: display_path.to_string_lossy().into_owned(),
        database_bytes: stats.database_bytes,
        content_file_count: stats.content_file_count,
        content_bytes: stats.content_bytes,
        external_dataset_count: manifest.external_dataset_count,
    })
}

pub fn restore_backup(
    archive_path: &Path,
    database_path: &Path,
    content_root: &Path,
) -> Result<BackupSummary, String> {
    restore_backup_internal(
        archive_path,
        database_path,
        content_root,
        None,
        env!("CARGO_PKG_VERSION"),
        &DomainTrustStore::default(),
    )
}

pub fn restore_backup_with_domain_validation(
    archive_path: &Path,
    database_path: &Path,
    content_root: &Path,
    domains_root: &Path,
    app_version: &str,
    trust_store: &DomainTrustStore,
) -> Result<BackupSummary, String> {
    restore_backup_internal(
        archive_path,
        database_path,
        content_root,
        Some(domains_root),
        app_version,
        trust_store,
    )
}

fn restore_backup_internal(
    archive_path: &Path,
    database_path: &Path,
    content_root: &Path,
    domains_root: Option<&Path>,
    app_version: &str,
    trust_store: &DomainTrustStore,
) -> Result<BackupSummary, String> {
    let display_path = archive_path.to_path_buf();
    let (_, archive_file) = authorize_existing_file_with_handle(archive_path)
        .map_err(|error| format!("backup archive path is not authorized: {error}"))?;
    if database_path.is_dir() {
        return Err("restore database destination must be a file".to_string());
    }
    if content_root.is_file() {
        return Err("restore content destination must be a directory".to_string());
    }

    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| format!("read backup archive failed: {error}"))?;
    let manifest = read_manifest(&mut archive)?;
    let stats = validate_archive(&mut archive, &manifest)?;
    if stats.total_bytes > MAX_UNCOMPRESSED_BYTES {
        return Err("backup exceeds the uncompressed size limit".to_string());
    }

    let database_parent = database_path
        .parent()
        .ok_or_else(|| "restore database parent is required".to_string())?;
    let content_parent = content_root
        .parent()
        .ok_or_else(|| "restore content parent is required".to_string())?;
    fs::create_dir_all(database_parent)
        .map_err(|error| format!("create restore database directory failed: {error}"))?;
    fs::create_dir_all(content_parent)
        .map_err(|error| format!("create restore content directory failed: {error}"))?;

    let staging_database =
        database_parent.join(format!(".suna-restore-{}.sqlite3", Uuid::new_v4()));
    let staging_content = content_parent.join(format!(".suna-content-restore-{}", Uuid::new_v4()));
    let result = extract_backup(
        &mut archive,
        &manifest,
        &staging_database,
        &staging_content,
        database_path,
        content_root,
        domains_root,
        app_version,
        trust_store,
    );
    if result.is_err() {
        let _ = fs::remove_file(&staging_database);
        let _ = fs::remove_dir_all(&staging_content);
    }
    result.map(|(database_bytes, content_bytes)| BackupSummary {
        format_version: manifest.format_version,
        archive_path: display_path.to_string_lossy().into_owned(),
        database_bytes,
        content_file_count: stats.content_file_count,
        content_bytes,
        external_dataset_count: manifest.external_dataset_count,
    })
}

fn validate_archive(
    archive: &mut ZipArchive<File>,
    manifest: &BackupManifest,
) -> Result<ArchiveStats, String> {
    let mut total_bytes = 0_u64;
    let mut content_file_count = 0_usize;
    let mut content_bytes = 0_u64;
    let mut database_bytes = 0_u64;
    let mut names = HashSet::new();
    let mut database_count = 0_usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read backup entry failed: {error}"))?;
        let name = entry.name().to_string();
        if !names.insert(name.clone()) {
            return Err(format!("duplicate backup entry: {name}"));
        }
        let kind = validate_entry_name(&name)?;
        if entry.is_dir() {
            continue;
        }
        if kind == EntryKind::Database {
            database_count += 1;
            database_bytes = entry.size();
            if manifest.format_version == 2 {
                let expected = manifest
                    .database_sha256
                    .as_ref()
                    .ok_or("backup_database_hash_missing")?;
                let (_, actual) = copy_hashed(&mut entry, &mut io::sink())?;
                if &actual != expected {
                    return Err("backup_database_hash_mismatch".into());
                }
            }
        } else if kind == EntryKind::Content {
            if manifest.format_version == 2 {
                let expected = manifest
                    .content_hashes
                    .get(&name)
                    .ok_or("backup_content_hash_missing")?;
                let (_, actual) = copy_hashed(&mut entry, &mut io::sink())?;
                if &actual != expected {
                    return Err("backup_content_hash_mismatch".into());
                }
            }
            content_file_count += 1;
            if content_file_count > MAX_FILES {
                return Err("backup contains too many content files".to_string());
            }
            content_bytes = content_bytes
                .checked_add(entry.size())
                .ok_or_else(|| "backup is too large".to_string())?;
        }
        total_bytes = total_bytes
            .checked_add(entry.size())
            .ok_or_else(|| "backup is too large".to_string())?;
    }
    if database_count != 1 {
        return Err("backup must contain exactly one database".to_string());
    }
    if content_file_count != manifest.content_file_count {
        return Err("backup content count does not match its manifest".to_string());
    }
    if manifest.format_version == 2 && manifest.content_hashes.len() != content_file_count {
        return Err("backup_content_hash_count_mismatch".into());
    }
    for dataset in &manifest.managed_datasets {
        let name = format!("{CONTENT_PREFIX}{}", dataset.relative_path);
        if !managed_relative(&dataset.relative_path)
            || manifest.content_hashes.get(&name) != Some(&dataset.sha256)
        {
            return Err("backup_managed_dataset_reference_invalid".into());
        }
    }
    Ok(ArchiveStats {
        database_bytes,
        content_file_count,
        content_bytes,
        total_bytes,
    })
}

fn extract_backup(
    archive: &mut ZipArchive<File>,
    manifest: &BackupManifest,
    staging_database: &Path,
    staging_content: &Path,
    database_path: &Path,
    content_root: &Path,
    domains_root: Option<&Path>,
    app_version: &str,
    trust_store: &DomainTrustStore,
) -> Result<(u64, u64), String> {
    let mut database_bytes = 0_u64;
    let mut content_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read backup entry failed: {error}"))?;
        if entry.is_dir() {
            continue;
        }
        match validate_entry_name(entry.name())? {
            EntryKind::Manifest => {}
            EntryKind::Database => {
                if entry.name() != manifest.database_entry {
                    return Err("backup database entry does not match its manifest".to_string());
                }
                let mut target = File::create(staging_database)
                    .map_err(|error| format!("create restored database failed: {error}"))?;
                let (_, actual) = copy_hashed(&mut entry, &mut target)?;
                if manifest.format_version == 2
                    && manifest.database_sha256.as_ref() != Some(&actual)
                {
                    return Err("restore_database_hash_mismatch".into());
                }
                database_bytes = fs::metadata(staging_database)
                    .map_err(|error| error.to_string())?
                    .len();
            }
            EntryKind::Content => {
                let relative = entry
                    .name()
                    .strip_prefix(&manifest.content_prefix)
                    .ok_or_else(|| "invalid backup content entry".to_string())?;
                let target =
                    staging_content.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        format!("create restored content directory failed: {error}")
                    })?;
                }
                let mut output = File::create(&target)
                    .map_err(|error| format!("create restored content failed: {error}"))?;
                let name = entry.name().to_string();
                let (bytes, sha) = copy_hashed(&mut entry, &mut output)?;
                if manifest.format_version == 2 && manifest.content_hashes.get(&name) != Some(&sha)
                {
                    return Err("restore_content_hash_mismatch".into());
                }
                content_bytes = content_bytes
                    .checked_add(bytes)
                    .ok_or("restored_content_byte_limit")?;
            }
        }
    }
    if database_bytes == 0 {
        return Err("backup database is empty".to_string());
    }

    validate_and_migrate_staged_database(staging_database)?;
    relocate_managed_datasets(
        staging_database,
        staging_content,
        content_root,
        &manifest.managed_datasets,
    )?;
    if let Some(domains_root) = domains_root {
        validate_staged_domain_packages(staging_database, domains_root, app_version, trust_store)?;
    }
    install_restored_files(
        staging_database,
        staging_content,
        database_path,
        content_root,
    )?;
    Ok((database_bytes, content_bytes))
}

fn validate_staged_domain_packages(
    database_path: &Path,
    domains_root: &Path,
    app_version: &str,
    trust_store: &DomainTrustStore,
) -> Result<(), String> {
    let connection = Connection::open(database_path)
        .map_err(|error| format!("open restored database for domain validation failed: {error}"))?;
    let records = crate::storage::repositories::domains::list_all(&connection)?;
    let expected_root =
        fs::canonicalize(domains_root).unwrap_or_else(|_| domains_root.to_path_buf());

    for record in records {
        for (field, value) in [
            ("id", record.id.as_str()),
            ("version", record.version.as_str()),
        ] {
            let path = Path::new(value);
            if value.is_empty()
                || path.components().count() != 1
                || !matches!(path.components().next(), Some(Component::Normal(_)))
            {
                return Err(format!(
                    "restored domain package {field} is unsafe: {value}"
                ));
            }
        }
        let stored_path = PathBuf::from(&record.path);
        if !stored_path.is_absolute()
            || stored_path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(format!(
                "restored domain package path is unsafe: {}",
                record.path
            ));
        }
        if !stored_path.is_dir() {
            return Err(format!(
                "restored domain package is missing: {}",
                stored_path.display()
            ));
        }
        let actual_path = fs::canonicalize(&stored_path).map_err(|error| {
            format!(
                "canonicalize restored domain package failed: {}: {error}",
                stored_path.display()
            )
        })?;
        let expected_path = fs::canonicalize(expected_root.join(&record.id).join(&record.version))
            .map_err(|error| {
                format!(
                    "canonicalize expected domain package path failed: {}: {error}",
                    record.path
                )
            })?;
        if actual_path != expected_path {
            return Err(format!(
                "restored domain package path is outside the managed domain directory: {}",
                record.path
            ));
        }
        let (manifest, package_sha256, trust) =
            domains::verify_installed_package(&actual_path, app_version, trust_store).map_err(
                |error| {
                    format!(
                        "restored domain package verification failed: {}: {error}",
                        actual_path.display()
                    )
                },
            )?;
        let manifest_matches = serde_json::to_value(&manifest)
            .map_err(|error| format!("serialize restored domain manifest failed: {error}"))?
            == serde_json::to_value(&record.manifest)
                .map_err(|error| format!("serialize backup domain manifest failed: {error}"))?;
        if !manifest_matches
            || !package_sha256.eq_ignore_ascii_case(&record.package_sha256)
            || trust != record.trust
        {
            return Err(format!(
                "restored domain package metadata does not match installed contents: {}",
                actual_path.display()
            ));
        }
    }
    Ok(())
}

fn validate_and_migrate_staged_database(path: &Path) -> Result<(), String> {
    let connection = Connection::open(path)
        .map_err(|error| format!("open restored database failed: {error}"))?;
    let quick_check: String = connection
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(|error| format!("restored database check failed: {error}"))?;
    if quick_check != "ok" {
        return Err("restored database failed integrity check".to_string());
    }
    let core_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_schema
             WHERE type = 'table'
               AND name IN ('schema_migrations', 'conversations', 'settings')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("inspect restored database schema failed: {error}"))?;
    if core_table_count != 3 {
        return Err("restored database is not a Suna database".to_string());
    }
    let migration_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM sqlite_schema
             WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("inspect restored database schema failed: {error}"))?;
    if migration_table_count != 1 {
        return Err("restored database is not a Suna database".to_string());
    }
    let migration_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|error| format!("inspect restored database migrations failed: {error}"))?;
    if migration_count == 0 {
        return Err("restored database is not a Suna database".to_string());
    }
    drop(connection);

    let (migrated, _) = crate::storage::database::open(path)
        .map_err(|error| format!("migrate restored Suna database failed: {error}"))?;
    migrated
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .map_err(|error| format!("checkpoint restored database failed: {error}"))?;
    drop(migrated);
    remove_database_sidecars(path)
}

fn install_restored_files(
    staging_database: &Path,
    staging_content: &Path,
    database_path: &Path,
    content_root: &Path,
) -> Result<(), String> {
    let parent = database_path
        .parent()
        .ok_or_else(|| "restore database parent is required".to_string())?;
    let rollback_root = parent.join(format!(".suna-rollback-{}", Uuid::new_v4()));
    let rollback_content = rollback_root.join("content");
    fs::create_dir_all(&rollback_content)
        .map_err(|error| format!("create restore rollback directory failed: {error}"))?;
    fs::create_dir_all(content_root)
        .map_err(|error| format!("create restore content directory failed: {error}"))?;

    let mut moved_files = Vec::new();
    let mut moved_content = Vec::new();
    let mut installed_content = Vec::new();
    let mut installed_database = false;
    let result = (|| {
        for name in database_sidecars(database_path) {
            move_existing_file(&name, &rollback_root, &mut moved_files)?;
        }
        move_existing_file(database_path, &rollback_root, &mut moved_files)?;
        for directory in CONTENT_DIRECTORIES {
            let target = content_root.join(directory);
            if target.exists() {
                let old = rollback_content.join(directory);
                fs::rename(&target, &old)
                    .map_err(|error| format!("stage existing restore content failed: {error}"))?;
                moved_content.push((old, target));
            }
        }
        fs::rename(staging_database, database_path)
            .map_err(|error| format!("install restored database failed: {error}"))?;
        installed_database = true;
        for directory in CONTENT_DIRECTORIES {
            let staged = staging_content.join(directory);
            if staged.exists() {
                let target = content_root.join(directory);
                fs::rename(&staged, &target)
                    .map_err(|error| format!("install restored content failed: {error}"))?;
                installed_content.push(target);
            }
        }
        Ok::<(), String>(())
    })();

    if let Err(error) = result {
        for path in installed_content {
            let _ = fs::remove_dir_all(path);
        }
        if installed_database {
            let _ = fs::remove_file(database_path);
        }
        for (old, target) in moved_content.into_iter().rev() {
            let _ = fs::rename(old, target);
        }
        for (old, target) in moved_files.into_iter().rev() {
            let _ = fs::rename(old, target);
        }
        let _ = fs::remove_dir_all(staging_content);
        let _ = fs::remove_file(staging_database);
        let _ = fs::remove_dir_all(&rollback_root);
        return Err(error);
    }

    let _ = fs::remove_dir_all(staging_content);
    fs::remove_dir_all(&rollback_root)
        .map_err(|error| format!("remove restore rollback directory failed: {error}"))?;
    Ok(())
}

fn move_existing_file(
    target: &Path,
    rollback_root: &Path,
    moved_files: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), String> {
    if !target.exists() {
        return Ok(());
    }
    let name = target
        .file_name()
        .ok_or_else(|| "restore file name is required".to_string())?;
    let old = rollback_root.join(name);
    fs::rename(target, &old)
        .map_err(|error| format!("stage existing restore file failed: {error}"))?;
    moved_files.push((old, target.to_path_buf()));
    Ok(())
}

fn database_sidecars(database_path: &Path) -> [PathBuf; 3] {
    [
        PathBuf::from(format!("{}-wal", database_path.display())),
        PathBuf::from(format!("{}-shm", database_path.display())),
        PathBuf::from(format!("{}-journal", database_path.display())),
    ]
}

fn remove_database_sidecars(database_path: &Path) -> Result<(), String> {
    for sidecar in database_sidecars(database_path) {
        match fs::remove_file(&sidecar) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "remove restored database sidecar failed: {}: {error}",
                    sidecar.display()
                ))
            }
        }
    }
    Ok(())
}

fn read_manifest(archive: &mut ZipArchive<File>) -> Result<BackupManifest, String> {
    let entry = archive
        .by_name("manifest.json")
        .map_err(|_| "backup manifest is missing".to_string())?;
    let mut bytes = Vec::new();
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err("backup_manifest_byte_limit".into());
    }
    entry
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read backup manifest failed: {error}"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("backup_manifest_byte_limit".into());
    }
    let manifest: BackupManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("parse backup manifest failed: {error}"))?;
    if !matches!(manifest.format_version, 1 | 2)
        || manifest.database_entry != DATABASE_ENTRY
        || manifest.content_prefix != CONTENT_PREFIX
    {
        return Err("unsupported backup format".to_string());
    }
    Ok(manifest)
}

fn validate_entry_name(name: &str) -> Result<EntryKind, String> {
    let is_safe = |value: &str| {
        !value.is_empty()
            && !value.contains('\\')
            && !value.contains(':')
            && !Path::new(value).is_absolute()
            && Path::new(value)
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    };
    if name == "manifest.json" {
        return Ok(EntryKind::Manifest);
    }
    if name == DATABASE_ENTRY {
        return Ok(EntryKind::Database);
    }
    if let Some(relative) = name.strip_prefix(CONTENT_PREFIX) {
        let relative = relative.trim_end_matches('/');
        if is_safe(relative) {
            return Ok(EntryKind::Content);
        }
    }
    Err(format!("unsafe backup entry: {name}"))
}

fn collect_content_files(
    root: &Path,
    archive_path: &Path,
) -> Result<Vec<(PathBuf, String, u64)>, String> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for directory in CONTENT_DIRECTORIES {
        let path = root.join(directory);
        if path.exists() {
            collect_content_files_from(root, &path, archive_path, &mut files)?;
        }
    }
    if files.len() > MAX_FILES {
        return Err("backup contains too many content files".to_string());
    }
    let total = files.iter().map(|(_, _, size)| *size).sum::<u64>();
    if total > MAX_UNCOMPRESSED_BYTES {
        return Err("backup content exceeds the uncompressed size limit".to_string());
    }
    Ok(files)
}

fn collect_content_files_from(
    root: &Path,
    directory: &Path,
    archive_path: &Path,
    files: &mut Vec<(PathBuf, String, u64)>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(directory).map_err(|error| format!("read backup content failed: {error}"))?
    {
        let entry = entry.map_err(|error| format!("read backup content entry failed: {error}"))?;
        let path = entry.path();
        if path == archive_path || entry.file_name() == ".staging" {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect backup content failed: {error}"))?;
        if file_type.is_symlink() {
            return Err(format!(
                "backup content cannot contain symlink: {}",
                path.display()
            ));
        }
        if file_type.is_dir() {
            collect_content_files_from(root, &path, archive_path, files)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;
        let relative = relative
            .to_str()
            .ok_or_else(|| "backup content path must be valid UTF-8".to_string())?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let size = fs::metadata(&path)
            .map_err(|error| format!("read backup content metadata failed: {error}"))?
            .len();
        files.push((path, format!("{CONTENT_PREFIX}{relative}"), size));
    }
    Ok(())
}

// Guard only an archive file created by this operation. Existing destinations
// are rejected by create_new and never removed on a failed attempt.
struct BackupOutput<'a> {
    path: &'a Path,
    completed: bool,
}
impl Drop for BackupOutput<'_> {
    fn drop(&mut self) {
        if !self.completed {
            let _ = fs::remove_file(self.path);
        }
    }
}
fn copy_hashed(
    source: &mut impl Read,
    destination: &mut impl Write,
) -> Result<(u64, String), String> {
    let mut sha = Sha256::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        if bytes > MAX_UNCOMPRESSED_BYTES {
            return Err("backup_entry_byte_limit".into());
        }
        destination
            .write_all(&buffer[..n])
            .map_err(|e| e.to_string())?;
        sha.update(&buffer[..n]);
    }
    Ok((bytes, format!("{:x}", sha.finalize())))
}
fn file_sha(path: &Path) -> Result<String, String> {
    Ok(copy_hashed(
        &mut File::open(path).map_err(|e| e.to_string())?,
        &mut io::sink(),
    )?
    .1)
}
fn managed_relative(relative: &str) -> bool {
    relative.starts_with("objects/material-data/")
        || relative.starts_with("objects/literature-data/")
        || relative.starts_with("material-datasets/")
        || relative.starts_with("research-datasets/")
}
fn managed_dataset_sources(
    c: &Connection,
    root: &Path,
    hashes: &std::collections::BTreeMap<String, String>,
) -> Result<(Vec<ManagedDataset>, usize), String> {
    if !root.is_dir() {
        let count: usize = c
            .query_row("SELECT COUNT(*) FROM steel_datasets", [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        return Ok((vec![], count));
    }
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    let mut stmt = c
        .prepare("SELECT workspace_id,id,source_path,source_sha256 FROM steel_datasets")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut result = vec![];
    let mut total = 0;
    for row in rows {
        total += 1;
        let (ws, id, source, sha) = row.map_err(|e| e.to_string())?;
        let raw = Path::new(&source);
        let lexical = raw
            .strip_prefix(root)
            .ok()
            .map(|p| p.to_string_lossy().replace('\\', "/"));
        let actual = raw.canonicalize().ok().and_then(|p| {
            p.strip_prefix(&canonical)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        });
        let relative = actual.or(lexical);
        if let Some(relative) = relative.filter(|r| managed_relative(r)) {
            if hashes.get(&format!("{CONTENT_PREFIX}{relative}")) != Some(&sha) {
                return Err("backup_managed_dataset_missing_or_changed".into());
            }
            result.push(ManagedDataset {
                workspace_id: ws,
                dataset_id: id,
                relative_path: relative,
                sha256: sha,
            });
        }
    }
    let external = total - result.len();
    Ok((result, external))
}
fn relocate_managed_datasets(
    db: &Path,
    staged_root: &Path,
    target_root: &Path,
    datasets: &[ManagedDataset],
) -> Result<(), String> {
    let mut c = Connection::open(db).map_err(|e| e.to_string())?;
    let tx = c.transaction().map_err(|e| e.to_string())?;
    for d in datasets {
        // validate_entry_name was already checked by the archive validator.
        validate_entry_name(&format!("{CONTENT_PREFIX}{}", d.relative_path))?;
        if !managed_relative(&d.relative_path)
            || file_sha(&staged_root.join(&d.relative_path))? != d.sha256
        {
            return Err("restore_managed_dataset_hash_mismatch".into());
        }
        let changed=tx.execute("UPDATE steel_datasets SET source_path=?1 WHERE workspace_id=?2 AND id=?3 AND source_sha256=?4",params![target_root.join(&d.relative_path).to_string_lossy(),d.workspace_id,d.dataset_id,d.sha256]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("restore_managed_dataset_not_found".into());
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

fn copy_file(source: &mut File, destination: &mut impl Write) -> io::Result<u64> {
    io::copy(source, destination)
}

#[cfg(test)]
mod tests {
    use super::{create_backup, preview_backup_from_file};
    use crate::storage::migrations::migrate;
    use rusqlite::Connection;
    use std::fs;
    use std::path::Path;
    use uuid::Uuid;

    #[test]
    fn preview_uses_the_supplied_archive_handle_not_the_path() {
        let root = std::env::temp_dir().join(format!("suna-backup-handle-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create fixture root");
        let database = root.join("source.sqlite3");
        let mut connection = Connection::open(&database).expect("open database");
        migrate(&mut connection).expect("migrate database");
        let archive = root.join("source.suna-backup");
        create_backup(&connection, &database, &root.join("content"), &archive)
            .expect("create backup");

        let archive_file = fs::File::open(&archive).expect("open authorized archive");
        let summary = preview_backup_from_file(archive_file, Path::new("path-is-not-used"))
            .expect("preview backup from authorized handle");

        assert_eq!(summary.archive_path, "path-is-not-used");
        assert!(summary.database_bytes > 0);
        drop(connection);
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
