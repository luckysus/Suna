use std::{fs, path::Path};
use suna::permissions::path::{authorize_existing_path, AuthorizedRoots, PathAuthorizationError};

#[test]
fn unicode_spaces_missing_tail_and_component_boundaries() {
    let base = std::env::temp_dir().join(format!("suna-path-{}", uuid::Uuid::new_v4()));
    let root = base.join("研究 空间");
    let neighbor = base.join("研究 空间-other");
    fs::create_dir_all(root.join("数据")).unwrap();
    fs::create_dir_all(&neighbor).unwrap();
    fs::write(neighbor.join("private.csv"), "outside").unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    let missing = root.join("数据/新版本/结果.csv");
    let output = roots.authorize(&missing).unwrap();
    assert!(output.local_path().ends_with(Path::new("新版本/结果.csv")));
    assert_eq!(
        roots.authorize(&neighbor.join("private.csv")),
        Err(PathAuthorizationError::OutsideRoots)
    );
    assert_eq!(
        roots.authorize(Path::new("relative.csv")),
        Err(PathAuthorizationError::RelativePath)
    );
    fs::create_dir_all(missing.parent().unwrap()).unwrap();
    fs::write(&missing, "original").unwrap();
    roots.verify_target(&output, &missing).unwrap();
    let resolved = authorize_existing_path(&missing).unwrap();
    let authorized_again = roots.authorize(&resolved.local_path()).unwrap();
    assert_eq!(resolved.canonical_path(), authorized_again.canonical_path());
    fs::remove_dir_all(base).unwrap();
}

#[cfg(unix)]
#[test]
fn case_sensitive_roots_and_symlink_escape_remain_distinct() {
    let base = std::env::temp_dir().join(format!("suna-path-case-{}", uuid::Uuid::new_v4()));
    let root = base.join("Research");
    let outside = base.join("research");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("private"), "outside").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
    let roots = AuthorizedRoots::new(vec![root.clone()]).unwrap();
    assert_eq!(
        roots.authorize(&outside.join("private")),
        Err(PathAuthorizationError::OutsideRoots)
    );
    assert_eq!(
        roots.authorize(&root.join("link/private")),
        Err(PathAuthorizationError::OutsideRoots)
    );
    fs::remove_dir_all(base).unwrap();
}
