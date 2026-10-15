use super::*;
use crate::research_package::projection::Projection;
use crate::storage::repositories::{research_artifacts, research_projects, steel};
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub(super) fn simple() -> (Vec<u8>, Asset, Vec<u8>) {
    let project = uuid::Uuid::new_v4().to_string();
    let data = uuid::Uuid::new_v4().to_string();
    let bytes = b"C,HV\n0.3,150\n".to_vec();
    let sha = digest(&bytes);
    let metadata = crate::research_package::archive::export(Projection {
        project_id: project.clone(),
        records: vec![
            json!({"id":project,"kind":"project"}),
            json!({"id":data,"kind":"dataset"}),
        ],
        dependencies: vec![
            json!({"resource_id":data,"kind":"dataset","status":"missing","expected_sha256":sha}),
        ],
        edges: vec![],
        app_version: "synthetic-test".into(),
    })
    .unwrap();
    let asset = Asset {
        resource_id: data,
        role: "dataset_source".into(),
        extension: "csv".into(),
        sha256: sha,
        bytes: bytes.len(),
    };
    (metadata, asset, bytes)
}
pub(super) fn rewrite(bytes: &[u8], edit: impl FnOnce(&mut BTreeMap<String, Vec<u8>>)) -> Vec<u8> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut files = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        files.insert(f.name().to_string(), b);
    }
    edit(&mut files);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(&data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
#[test]
fn selected_assets_are_hash_bound_deduplicated_and_never_imply_live_restoration() {
    let (metadata, asset, body) = simple();
    assert_eq!(
        archive::prepare(metadata.clone(), vec![(asset.clone(), body.clone())], false)
            .err()
            .unwrap(),
        "package_asset_rights_confirmation_required"
    );
    let prepared = archive::prepare(metadata, vec![(asset.clone(), body.clone())], true).unwrap();
    let summary = prepared.summary().clone();
    let bytes = prepared.into_bytes();
    let verified = archive::inspect(&bytes).unwrap();
    assert_eq!(verified.assets[0].1, body);
    assert_eq!(summary["packageSha256"], digest(&bytes));
    assert_eq!(summary["missingDependencies"], json!([]));
    for key in [
        "writesToLiveWorkspace",
        "modelsActivated",
        "measurementsCreated",
        "rightsIndependentlyVerified",
    ] {
        assert_eq!(summary[key], false);
    }
    let tampered = rewrite(&bytes, |f| {
        f.get_mut(&asset.name()).unwrap()[0] ^= 1;
    });
    assert_eq!(
        archive::inspect(&tampered).err().unwrap(),
        "package_asset_hash_mismatch"
    );
    let other = rewrite(&bytes, |f| {
        f.insert("assets/../../private.sqlite3".into(), b"bad".to_vec());
    });
    assert_eq!(
        archive::inspect(&other).err().unwrap(),
        "package_zip_entry_invalid"
    );
    let unknown = rewrite(&bytes, |f| {
        f.insert("settings.json".into(), b"{}".to_vec());
    });
    assert!(archive::inspect(&unknown).is_err());
    let duplicate = rewrite(&bytes, |f| {
        let mut m: Value = serde_json::from_slice(&f["manifest.json"]).unwrap();
        let a = m["assets"][0].clone();
        m["assets"].as_array_mut().unwrap().push(a);
        f.insert("manifest.json".into(), serde_json::to_vec(&m).unwrap());
    });
    assert_eq!(
        archive::inspect(&duplicate).err().unwrap(),
        "package_asset_identity_conflict"
    );
}

#[test]
fn malformed_roles_unlisted_files_and_private_model_fields_cannot_be_packaged() {
    let (metadata, mut asset, body) = simple();
    asset.role = "model_artifact".into();
    asset.extension = "json".into();
    let model = serde_json::to_vec(
        &json!({"model_type":"linear_regression","api_key":"synthetic-private"}),
    )
    .unwrap();
    asset.bytes = model.len();
    asset.sha256 = digest(&model);
    assert_eq!(
        archive::prepare(metadata.clone(), vec![(asset.clone(), model)], true)
            .err()
            .unwrap(),
        "package_asset_private_fields"
    );
    let model =
        br#"{"model_type":"linear_regression","source_path":"C:/synthetic-private.csv"}"#.to_vec();
    asset.bytes = model.len();
    asset.sha256 = digest(&model);
    assert_eq!(
        archive::prepare(metadata.clone(), vec![(asset.clone(), model)], true)
            .err()
            .unwrap(),
        "package_asset_private_fields"
    );
    asset.extension = "exe".into();
    asset.bytes = body.len();
    asset.sha256 = digest(&body);
    assert_eq!(
        archive::prepare(metadata, vec![(asset, body)], true)
            .err()
            .unwrap(),
        "package_asset_schema_invalid"
    );
}

fn managed_fixture() -> crate::research_workflow::testing::Fixture {
    let mut f = crate::research_workflow::testing::fixture();
    let source = steel::get(&f.connection, "local", &f.input.dataset_id)
        .unwrap()
        .unwrap();
    let folder = f.root.join("objects/material-data");
    std::fs::create_dir_all(&folder).unwrap();
    let managed = folder.join("合成 规范化.csv");
    std::fs::copy(&source.source_path, &managed).unwrap();
    f.connection
        .execute(
            "UPDATE steel_datasets SET source_path=?1 WHERE id=?2",
            rusqlite::params![managed.to_string_lossy(), f.input.dataset_id],
        )
        .unwrap();
    research_projects::attach(
        &mut f.connection,
        "local",
        &f.input.project_id,
        "dataset",
        &f.input.dataset_id,
        "attached",
    )
    .unwrap();
    f
}
fn prepared_fixture(f: &crate::research_workflow::testing::Fixture) -> Prepared {
    catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[f.input.dataset_id.clone()],
        true,
    )
    .unwrap()
    .prepare()
    .unwrap()
}
#[test]
fn source_selection_is_scoped_read_only_and_changed_bytes_or_stale_previews_fail() {
    let f = managed_fixture();
    let changes: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let inventory =
        catalog::inventory(&f.connection, "local", &f.root, &f.input.project_id).unwrap();
    assert_eq!(inventory["assets"][0]["resourceId"], f.input.dataset_id);
    assert!(!inventory
        .to_string()
        .contains(f.root.to_string_lossy().as_ref()));
    assert!(catalog::inventory(&f.connection, "other", &f.root, &f.input.project_id).is_err());
    assert!(catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[uuid::Uuid::new_v4().to_string()],
        true
    )
    .is_err());
    assert!(catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[f.input.dataset_id.clone()],
        false
    )
    .is_err());
    let captured = catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[f.input.dataset_id.clone()],
        true,
    )
    .unwrap();
    let current = prepared_fixture(&f);
    let sha = current.summary()["packageSha256"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        f.connection
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        changes
    );
    research_artifacts::create(
        &f.connection,
        "local",
        &f.input.project_id,
        "analysis_result",
        "合成新增成果",
        "synthetic-archive-change",
        None,
        json!({"value":1}),
    )
    .unwrap();
    assert_eq!(
        prepared_fixture(&f).confirm(&sha).err().unwrap(),
        "package_preview_changed"
    );
    let file = f.root.join("objects/material-data/合成 规范化.csv");
    std::fs::write(&file, "changed").unwrap();
    assert_eq!(
        captured.prepare().err().unwrap(),
        "package_asset_hash_mismatch"
    );
    assert!(prepared_fixture_checked(&f).is_err());
    assert!(current.confirm(&"0".repeat(64)).is_err());
    assert_ne!(sha, "0".repeat(64));
}
fn prepared_fixture_checked(
    f: &crate::research_workflow::testing::Fixture,
) -> Result<Prepared, String> {
    catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &[f.input.dataset_id.clone()],
        true,
    )?
    .prepare()
}

#[test]
fn isolation_persists_new_ids_reopens_and_repeats_without_touching_existing_data() {
    let f = managed_fixture();
    let prepared = prepared_fixture(&f);
    let bytes = prepared.into_bytes();
    let verified = archive::inspect(&bytes).unwrap();
    let before = research_artifacts::list(&f.connection, "local", &f.input.project_id, 100, 0)
        .unwrap()
        .iter()
        .map(|a| (a.id.clone(), a.content_sha256.clone()))
        .collect::<Vec<_>>();
    std::fs::write(f.root.join("keep.txt"), b"synthetic-existing").unwrap();
    assert_eq!(
        staging::import_failing_after_assets(&verified, &f.root).unwrap_err(),
        "synthetic_import_failure"
    );
    assert!(!f
        .root
        .join("research-package-imports")
        .join(&verified.package_sha256)
        .exists());
    let result = staging::import(&verified, &f.root, &verified.package_sha256).unwrap();
    let again = staging::import(&verified, &f.root, &verified.package_sha256).unwrap();
    assert_eq!(again, result);
    assert_eq!(result["status"], "isolated_assets_stored");
    assert_eq!(result["reopenVerified"], true);
    assert_eq!(result["assetsRestoredToLiveWorkspace"], false);
    assert_eq!(result["readyForComputation"], false);
    for (old, new) in result["idMapping"].as_object().unwrap() {
        assert_ne!(old, new.as_str().unwrap());
    }
    assert_eq!(
        std::fs::read(f.root.join("keep.txt")).unwrap(),
        b"synthetic-existing"
    );
    let after = research_artifacts::list(&f.connection, "local", &f.input.project_id, 100, 0)
        .unwrap()
        .iter()
        .map(|a| (a.id.clone(), a.content_sha256.clone()))
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    let dest = f
        .root
        .join("research-package-imports")
        .join(&verified.package_sha256)
        .join(verified.assets[0].0.name());
    std::fs::write(dest, "tampered").unwrap();
    assert!(staging::import(&verified, &f.root, &verified.package_sha256).is_err());
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn actual_registered_models_dataset_and_original_image_survive_managed_package_isolation() {
    use crate::research_workflow::runtime_support::*;
    let (mut f, path) = setup();
    let root = submit(&mut f, uuid::Uuid::new_v4());
    let mut scheduler = new_scheduler(&path);
    completed(&mut scheduler, &f.connection, root.id);
    drain(&mut scheduler);
    drop(scheduler);
    let data = steel::get(&f.connection, "local", &f.input.dataset_id)
        .unwrap()
        .unwrap();
    let directory = f.root.join("objects/material-data");
    std::fs::create_dir_all(&directory).unwrap();
    let dest = directory.join("实际合成输入.csv");
    std::fs::copy(&data.source_path, &dest).unwrap();
    f.connection
        .execute(
            "UPDATE steel_datasets SET source_path=?1 WHERE id=?2",
            rusqlite::params![dest.to_string_lossy(), data.id],
        )
        .unwrap();
    let image = f.root.join("原始合成图.png");
    image::GrayImage::from_fn(4, 4, |x, y| image::Luma([(x + y) as u8 * 20]))
        .save(&image)
        .unwrap();
    crate::storage::repositories::microstructure::import(
        &f.connection,
        "local",
        &f.root,
        crate::storage::repositories::microstructure::ImportImage {
            source_path: image.to_string_lossy().into(),
            project_id: f.input.project_id.clone(),
            sample_id: "synthetic".into(),
            modality: "OM".into(),
            source_type: "synthetic".into(),
            magnification: None,
            acquired_at: None,
            notes: "".into(),
            gray16_mapping: None,
        },
    )
    .unwrap();
    let inventory =
        catalog::inventory(&f.connection, "local", &f.root, &f.input.project_id).unwrap();
    let ids = inventory["assets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["resourceId"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 4);
    let prepared = catalog::capture(
        &f.connection,
        "local",
        &f.root,
        &f.input.project_id,
        &ids,
        true,
    )
    .unwrap()
    .prepare()
    .unwrap();
    let bytes = prepared.into_bytes();
    let verified = archive::inspect(&bytes).unwrap();
    assert_eq!(verified.assets.len(), 4);
    assert_eq!(
        verified
            .assets
            .iter()
            .filter(|(a, _)| a.role == "model_artifact")
            .count(),
        2
    );
    let imported = staging::import(&verified, &f.root, &verified.package_sha256).unwrap();
    assert_eq!(imported["assetCount"], 4);
    for (asset, body) in &verified.assets {
        assert_eq!(
            std::fs::read(
                f.root
                    .join("research-package-imports")
                    .join(&verified.package_sha256)
                    .join(asset.name())
            )
            .unwrap(),
            *body
        );
    }
    assert_eq!(
        std::fs::read(image).unwrap().as_slice(),
        verified
            .assets
            .iter()
            .find(|(a, _)| a.role == "image_original")
            .unwrap()
            .1
            .as_slice()
    );
    assert_eq!(
        staging::import(&verified, &f.root, &verified.package_sha256).unwrap(),
        imported
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM steel_models WHERE is_active=1",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
}
