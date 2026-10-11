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
    assert_eq!(PathBuf::from(path), target.join("objects/nested/paper.txt"));
    let json: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        json["path"],
        target
            .join("objects/nested/paper.txt")
            .to_string_lossy()
            .as_ref()
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
