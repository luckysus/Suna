use super::*;

fn root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("suna-directory-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn validates_same_nested_nonempty_relative_and_file_targets() {
    let root = root();
    let source = root.join("source");
    fs::create_dir_all(source.join("nested")).unwrap();
    assert!(validate_target(&source, &source).is_err());
    assert!(validate_target(&source, &source.join("nested")).is_err());
    assert!(validate_target(&source, &root).is_err());
    assert!(validate_target(&source, Path::new("relative")).is_err());
    let occupied = root.join("occupied");
    fs::create_dir_all(&occupied).unwrap();
    fs::write(occupied.join("valuable"), b"keep").unwrap();
    assert!(validate_target(&source, &occupied).is_err());
    let file = root.join("file");
    fs::write(&file, b"keep").unwrap();
    assert!(validate_target(&source, &file).is_err());
    assert!(validate_target(&source, &root.join("empty")).is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn migration_preserves_wal_records_all_files_and_relocates_owned_paths() {
    let root = root();
    let source = root.join("source");
    let target = root.join("target");
    let config_path = root.join("config/data-directory.json");
    fs::create_dir_all(source.join("objects/nested")).unwrap();
    fs::create_dir_all(source.join("datasets")).unwrap();
    fs::create_dir_all(source.join("agent-workspace")).unwrap();
    fs::write(source.join("objects/nested/paper.txt"), "中文文献").unwrap();
    fs::write(source.join("datasets/data.csv"), "a,b\n1,2").unwrap();
    let db = Connection::open(source.join("suna.sqlite3")).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE paths(source_path TEXT, value_json TEXT); INSERT INTO paths VALUES ('external', '{}');").unwrap();
    let old_file = source.join("objects/nested/paper.txt");
    let json = serde_json::json!({"path": old_file, "external": root.join("external.csv")});
    db.execute(
        "INSERT INTO paths VALUES (?1, ?2)",
        rusqlite::params![old_file.to_string_lossy(), json.to_string()],
    )
    .unwrap();
    let mut config = DirectoryConfig {
        path: Some(source.clone()),
        pending: Some(target.clone()),
        ..Default::default()
    };
    write_config(&config_path, &config).unwrap();
    let mut active = source.clone();
    complete_pending(&config_path, &mut config, &mut active).unwrap();
    assert_eq!(active, target);
    assert!(config.pending.is_none());
    assert_eq!(
        read_config(&config_path).unwrap().path,
        Some(target.clone())
    );
    assert_eq!(
        fs::read_to_string(target.join("objects/nested/paper.txt")).unwrap(),
        "中文文献"
    );
    assert!(target.join("agent-workspace").is_dir());
    assert!(source.join("datasets/data.csv").exists());
    let copied = Connection::open(target.join("suna.sqlite3")).unwrap();
    assert_eq!(
        copied
            .query_row("SELECT count(*) FROM paths", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
    let (path, json): (String, String) = copied
        .query_row(
            "SELECT source_path,value_json FROM paths WHERE source_path != 'external'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let expected = fs::canonicalize(target.join("objects/nested/paper.txt")).unwrap();
    assert_eq!(fs::canonicalize(&path).unwrap(), expected);
    assert_ne!(
        fs::canonicalize(&path).unwrap(),
        fs::canonicalize(&old_file).unwrap()
    );
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        fs::canonicalize(json["path"].as_str().unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        json["external"],
        root.join("external.csv").to_string_lossy().as_ref()
    );
    drop(copied);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_migration_keeps_original_config_and_data() {
    let root = root();
    let source = root.join("source");
    let target = root.join("occupied");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&target).unwrap();
    fs::write(source.join("valuable.txt"), "old").unwrap();
    fs::write(target.join("valuable.txt"), "target").unwrap();
    let config_path = root.join("config.json");
    let mut config = DirectoryConfig {
        path: Some(source.clone()),
        pending: Some(target.clone()),
        ..Default::default()
    };
    let mut active = source.clone();
    complete_pending(&config_path, &mut config, &mut active).unwrap();
    assert_eq!(active, source);
    assert_eq!(
        read_config(&config_path).unwrap().path,
        Some(source.clone())
    );
    assert!(config.message.unwrap().contains("迁移失败"));
    assert_eq!(
        fs::read_to_string(source.join("valuable.txt")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(target.join("valuable.txt")).unwrap(),
        "target"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn relocation_matches_boundaries_and_supports_multiple_moves() {
    let root = root();
    let source = root.join("source");
    let target = root.join("target");
    assert_eq!(
        relocate_path(&source.join("objects/paper"), &source, &target),
        Some(target.join("objects/paper"))
    );
    assert!(relocate_path(&root.join("source-neighbor/paper"), &source, &target).is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn relocation_keeps_real_frozen_artifact_hashes_and_is_idempotent() {
    use crate::storage::repositories::{
        research_artifacts as artifacts, research_projects as projects,
    };
    let base = root();
    let source = base.join("中文 source");
    let target = base.join("中文 target");
    fs::create_dir_all(source.join("objects")).unwrap();
    let file = source.join("objects/paper.txt");
    fs::write(&file, "unchanged source").unwrap();
    let mut c = Connection::open(source.join("suna.sqlite3")).unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    let project = projects::save(
        &mut c,
        "ws",
        projects::SaveResearchProject {
            id: None,
            name: "合成迁移夹具".into(),
            description: "fixture only".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let payload = serde_json::json!({"historicalPath":file,"fixture":"immutable evidence"});
    let id = artifacts::create(
        &c,
        "ws",
        &project.id,
        "report",
        "历史记录",
        "original",
        None,
        payload.clone(),
    )
    .unwrap();
    let before = artifacts::get(&c, "ws", &id).unwrap().content_sha256;
    c.execute_batch(
        "CREATE TABLE operational_paths(source_path TEXT,value_json TEXT,title_path TEXT);",
    )
    .unwrap();
    let external = base.join("source-neighbor/private.txt");
    c.execute(
        "INSERT INTO operational_paths VALUES(?1,?2,?3)",
        rusqlite::params![
            file.to_string_lossy(),
            serde_json::json!({"items":[file, external],"sourceDirectory":source}).to_string(),
            file.to_string_lossy()
        ],
    )
    .unwrap();
    relocate_database(&mut c, &source, &target).unwrap();
    let after = artifacts::get(&c, "ws", &id).unwrap();
    assert_eq!(after.content_sha256, before);
    assert_eq!(after.payload, Some(payload));
    let row: (String, String, String) = c
        .query_row("SELECT * FROM operational_paths", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(PathBuf::from(&row.0), target.join("objects/paper.txt"));
    assert_eq!(PathBuf::from(&row.2), file);
    let json: serde_json::Value = serde_json::from_str(&row.1).unwrap();
    // JSON contains local filesystem paths, whose separator spelling may differ
    // from PathBuf::join's input. Compare components, not serialized strings.
    assert_eq!(
        Path::new(json["items"][0].as_str().unwrap()),
        target.join("objects/paper.txt")
    );
    assert_ne!(Path::new(json["items"][0].as_str().unwrap()), file);
    assert_eq!(json["items"][1], external.to_string_lossy().as_ref());
    assert_eq!(Path::new(json["sourceDirectory"].as_str().unwrap()), target);
    relocate_database(&mut c, &source, &target).unwrap();
    let repeated: (String, String, String) = c
        .query_row("SELECT * FROM operational_paths", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(row, repeated);
    let repeated_artifact = artifacts::get(&c, "ws", &id).unwrap();
    assert_eq!(repeated_artifact.content_sha256, before);
    assert_eq!(repeated_artifact.payload, after.payload);
    assert_eq!(fs::read_to_string(&file).unwrap(), "unchanged source");
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    drop(c);
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn relocated_json_round_trip_preserves_path_semantics_and_external_spelling() {
    let base = root();
    let source = base.join("中文 source");
    let target = base.join("中文 target");
    fs::create_dir_all(source.join("objects")).unwrap();
    let file = source.join("objects/paper.txt");
    fs::write(&file, "original").unwrap();
    let external = base.join("中文 source-neighbor/objects/paper.txt");
    let external_text = external.to_string_lossy().into_owned();
    let expected = target.join("objects").join("paper.txt");
    let spellings = vec![file.clone()];
    #[cfg(windows)]
    let spellings = {
        let mut spellings = spellings;
        spellings.push(PathBuf::from(file.to_string_lossy().replace('\\', "/")));
        spellings.push(source.join("objects").join("paper.txt"));
        spellings
    };
    for spelling in spellings {
        let mut json = serde_json::json!({
            "items": [{"path": spelling}, external_text],
            "directory": source,
            "label": "fixture, not a path",
        });
        relocate_json(&mut json, &source, &target);
        let serialized = serde_json::to_string(&json).unwrap();
        let round_trip: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        let relocated = Path::new(round_trip["items"][0]["path"].as_str().unwrap());
        assert_eq!(relocated, expected);
        assert_ne!(relocated, file);
        assert_eq!(Path::new(round_trip["directory"].as_str().unwrap()), target);
        // External strings must remain byte-for-byte unchanged, including slash
        // spelling; only owned paths are eligible for relocation.
        assert_eq!(round_trip["items"][1], external_text);
        assert_eq!(round_trip["label"], "fixture, not a path");
        relocate_json(&mut json, &source, &target);
        assert_eq!(serde_json::to_string(&json).unwrap(), serialized);
    }
    assert_eq!(fs::read_to_string(file).unwrap(), "original");
    fs::remove_dir_all(base).unwrap();
}

#[cfg(windows)]
#[test]
fn relocation_handles_canonical_drive_case_slashes_and_short_aliases() {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let base = root();
    let source = base.join("研究 source with spaces");
    let target = base.join("target");
    fs::create_dir_all(source.join("objects")).unwrap();
    let file = source.join("objects/记录.txt");
    fs::write(&file, "original").unwrap();
    let canonical = fs::canonicalize(&file).unwrap();
    assert_eq!(
        relocate_path(&canonical, &source, &target),
        Some(target.join("objects/记录.txt"))
    );
    let spelling = source
        .to_string_lossy()
        .to_ascii_uppercase()
        .replace('\\', "/");
    assert_eq!(
        relocate_path(&file, Path::new(&spelling), &target),
        Some(target.join("objects/记录.txt"))
    );
    let wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buffer = vec![0u16; 32768];
    let length =
        unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) }
            as usize;
    assert!(length > 0 && length < buffer.len());
    let short = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length]));
    if short == source {
        eprintln!("8.3 aliases unavailable on this filesystem; canonical/case/separator assertions still executed");
    }
    assert_eq!(
        relocate_path(&short.join("objects/记录.txt"), &source, &target),
        Some(target.join("objects/记录.txt"))
    );
    assert_eq!(
        relocate_path(&short.join("objects/missing.csv"), &source, &target),
        Some(target.join("objects/missing.csv"))
    );
    assert!(relocate_path(Path::new(r"\\?\UNC\server\share\file"), &source, &target).is_none());
    assert!(relocate_path(&file.with_extension("txt:secret"), &source, &target).is_none());
    assert!(relocate_path(&source.join("objects/../../outside"), &source, &target).is_none());
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn configuration_write_failure_keeps_original_directory_active() {
    let root = root();
    let source = root.join("source");
    let target = root.join("target");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("valuable.txt"), "original").unwrap();
    let blocked_parent = root.join("not-a-directory");
    fs::write(&blocked_parent, "keep").unwrap();
    let mut config = DirectoryConfig {
        path: Some(source.clone()),
        pending: Some(target.clone()),
        ..Default::default()
    };
    let mut active = source.clone();
    complete_pending(
        &blocked_parent.join("config.json"),
        &mut config,
        &mut active,
    )
    .unwrap();
    assert_eq!(active, source);
    assert_eq!(config.path, Some(source));
    assert!(config.message.unwrap().contains("保存配置失败"));
    assert_eq!(
        fs::read_to_string(target.join("valuable.txt")).unwrap(),
        "original"
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn rejects_windows_junctions_without_following_them() {
    let root = root();
    let source = root.join("source");
    let outside = root.join("outside");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let script = format!(
        "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
        source.join("link").display(),
        outside.display()
    );
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(collect_tree(&source, &mut Vec::new()).is_err());
    assert!(validate_target(&source, &source.join("link")).is_err());
    fs::remove_dir(source.join("link")).unwrap();
    fs::remove_dir_all(root).unwrap();
}
