#![cfg(windows)]

use std::{fs, path::Path};
use suna::permissions::path::{AuthorizedRoots, PathAuthorizationError};

#[test]
fn filesystem_canonical_result_is_reusable_only_through_authorized_token() {
    let root = std::env::temp_dir().join(format!("suna-中文 空格-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("研究.csv"), "original").unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    let authorized = roots.authorize(&root.join("研究.csv")).unwrap();
    assert!(authorized
        .canonical_path()
        .to_string_lossy()
        .starts_with(r"\\?\"));
    assert_eq!(
        roots.authorize(authorized.canonical_path()),
        Err(PathAuthorizationError::DevicePath)
    );
    assert!(AuthorizedRoots::new(vec![fs::canonicalize(&root).unwrap()]).is_err());
    let local = authorized.local_path();
    let again = roots.authorize(&local).unwrap();
    assert_eq!(again.canonical_path(), authorized.canonical_path());
    let file = fs::File::open(local).unwrap();
    roots.verify_opened_file(&again, &file).unwrap();
    drop(file);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn all_external_device_and_network_spellings_and_ads_stay_rejected() {
    let root = std::env::temp_dir().join(format!("suna-raw-path-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    for path in [
        r"\\?\C:\data.csv",
        r"\\.\C:\data.csv",
        r"\\?\GLOBALROOT\Device\HarddiskVolume1\data.csv",
        "//?/C:/data.csv",
        "//./C:/data.csv",
        r"\\?\UNC\server\share\data.csv",
        r"\Device\HarddiskVolume1\data.csv",
    ] {
        assert!(
            roots.authorize(Path::new(path)).is_err(),
            "raw namespace accepted: {path}"
        );
    }
    for path in [r"\\server\share\data.csv", "//server/share/data.csv"] {
        assert!(roots.authorize(Path::new(path)).is_err());
    }
    for path in [root.join("data.csv:secret"), root.join("data.csv::$DATA")] {
        assert_eq!(
            roots.authorize(&path),
            Err(PathAuthorizationError::AlternateDataStream)
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn long_local_path_drive_case_and_missing_tail_are_supported() {
    let root = std::env::temp_dir().join(format!("suna-long-{}", uuid::Uuid::new_v4()));
    let mut directory = root.clone();
    for i in 0..12 {
        directory.push(format!("中文 空格-long-directory-{i:02}"));
    }
    fs::create_dir_all(&directory).unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    let missing = directory.join("新结果.csv");
    assert!(missing.to_string_lossy().len() > 260);
    let authorized = roots.authorize(&missing).unwrap();
    fs::write(authorized.canonical_path(), "long path").unwrap();
    let mut spelling = authorized.local_path().to_string_lossy().into_owned();
    spelling.replace_range(..1, &spelling[..1].to_ascii_lowercase());
    let rechecked = roots
        .authorize(Path::new(&spelling.replace('\\', "/")))
        .unwrap();
    assert_eq!(authorized.canonical_path(), rechecked.canonical_path());
    roots.verify_target(&authorized, &missing).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn moved_handle_and_replaced_junction_parent_are_rejected_before_writes() {
    let base = std::env::temp_dir().join(format!("suna-handle-{}", uuid::Uuid::new_v4()));
    let root = base.join("root");
    let outside = base.join("outside");
    fs::create_dir_all(root.join("nested")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    let path = root.join("nested/inside.csv");
    fs::write(&path, "original").unwrap();
    let authorized = roots.authorize(&path).unwrap();
    let file = fs::File::open(&path).unwrap();
    fs::rename(&path, root.join("nested/moved.csv")).unwrap();
    fs::write(&path, "replacement").unwrap();
    assert_eq!(
        roots.verify_opened_file(&authorized, &file),
        Err(PathAuthorizationError::TargetChanged)
    );
    drop(file);
    let missing = root.join("nested/new.csv");
    let output = roots.authorize(&missing).unwrap();
    fs::rename(root.join("nested"), root.join("old-nested")).unwrap();
    let status = std::process::Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(root.join("nested"))
        .arg(&outside)
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        roots.authorize(&missing),
        Err(PathAuthorizationError::OutsideRoots)
    );
    assert_eq!(
        roots.verify_target(&output, &missing),
        Err(PathAuthorizationError::TargetChanged)
    );
    let outside_file = fs::File::create(&missing).unwrap();
    assert_eq!(
        roots.verify_opened_file(&output, &outside_file),
        Err(PathAuthorizationError::TargetChanged)
    );
    drop(outside_file);
    assert_eq!(fs::read(outside.join("new.csv")).unwrap(), b"");
    fs::remove_dir(root.join("nested")).unwrap();
    fs::remove_dir_all(base).unwrap();
}
