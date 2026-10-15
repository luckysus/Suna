use super::tests::{rewrite, simple};
use super::*;
use serde_json::json;

#[test]
fn logical_asset_expansion_and_role_or_expected_hash_forgery_are_rejected() {
    let (metadata, asset, body) = simple();
    let prepared = archive::prepare(metadata, vec![(asset, body)], true).unwrap();
    let bytes = prepared.into_bytes();
    let expansion = rewrite(&bytes, |files| {
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        let mut asset = manifest["assets"][0].clone();
        asset["bytes"] = json!(MAX_ASSET);
        manifest["assets"] = json!(vec![asset; MAX_ASSETS]);
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
    });
    assert_eq!(
        archive::inspect(&expansion).err().unwrap(),
        "package_size_budget"
    );
    let wrong_role = rewrite(&bytes, |files| {
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["assets"][0]["role"] = json!("image_original");
        manifest["assets"][0]["extension"] = json!("png");
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
    });
    assert_eq!(
        archive::inspect(&wrong_role).err().unwrap(),
        "package_asset_role_mismatch"
    );
    let wrong_sha = rewrite(&bytes, |files| {
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["assets"][0]["sha256"] = json!("0".repeat(64));
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
    });
    assert_eq!(
        archive::inspect(&wrong_sha).err().unwrap(),
        "package_asset_expected_digest_conflict"
    );
    let sensitive_header = rewrite(&bytes, |files| {
        let mut manifest: Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["credential"] = json!("synthetic-private");
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
    });
    assert!(archive::inspect(&sensitive_header).is_err());
}

#[cfg(unix)]
#[test]
fn external_sources_and_isolation_symlinks_cannot_copy_or_overwrite_other_files() {
    use crate::storage::repositories::{research_projects, steel};
    let mut f = crate::research_workflow::testing::fixture();
    research_projects::attach(
        &mut f.connection,
        "local",
        &f.input.project_id,
        "dataset",
        &f.input.dataset_id,
        "attached",
    )
    .unwrap();
    // Even an app directory source must be in an explicit managed-data namespace.
    assert_eq!(
        catalog::inventory(&f.connection, "local", &f.root, &f.input.project_id).unwrap()["assets"]
            [0]["availability"],
        "unavailable_or_external"
    );
    let source = steel::get(&f.connection, "local", &f.input.dataset_id)
        .unwrap()
        .unwrap();
    let owned = f.root.join("objects/material-data");
    std::fs::create_dir_all(&owned).unwrap();
    let link = owned.join("outside.csv");
    std::os::unix::fs::symlink(&source.source_path, &link).unwrap();
    // A symlink to an unmanaged namespace does not authorize that namespace.
    f.connection
        .execute(
            "UPDATE steel_datasets SET source_path=?1 WHERE id=?2",
            rusqlite::params![link.to_string_lossy(), source.id],
        )
        .unwrap();
    assert!(catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[source.id],
        true
    )
    .is_err());
    let (metadata, asset, body) = simple();
    let verified = archive::inspect(
        &archive::prepare(metadata, vec![(asset, body)], true)
            .unwrap()
            .into_bytes(),
    )
    .unwrap();
    let outside = f.root.join("untouched");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep.txt"), "preserved").unwrap();
    std::os::unix::fs::symlink(&outside, f.root.join("research-package-imports")).unwrap();
    assert!(staging::import(&verified, &f.root, &verified.package_sha256).is_err());
    assert_eq!(
        std::fs::read_to_string(outside.join("keep.txt")).unwrap(),
        "preserved"
    );
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 1);
}
