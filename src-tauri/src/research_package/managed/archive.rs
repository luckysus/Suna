use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn allowed_name(name: &str) -> bool {
    if matches!(name, "manifest.json" | "metadata.zip") {
        return true;
    }
    name.strip_prefix("assets/")
        .and_then(|v| v.split_once('.'))
        .is_some_and(|(sha, ext)| {
            valid_sha(sha)
                && matches!(
                    ext,
                    "csv" | "json" | "png" | "jpg" | "jpeg" | "tif" | "tiff"
                )
        })
}

pub(super) fn validate_bytes(asset: &Asset, bytes: &[u8]) -> Result<(), String> {
    asset.validate()?;
    if bytes.len() != asset.bytes || digest(bytes) != asset.sha256 {
        return Err("package_asset_hash_mismatch".into());
    }
    match asset.role.as_str() {
        "image_original" => {
            let expected = match asset.extension.as_str() {
                "png" => image::ImageFormat::Png,
                "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                _ => image::ImageFormat::Tiff,
            };
            if image::guess_format(bytes).ok() != Some(expected) {
                return Err("package_asset_type_mismatch".into());
            }
        }
        "dataset_source" => {
            let text = std::str::from_utf8(bytes).map_err(|_| "package_asset_text_encoding")?;
            if text.contains('\0') {
                return Err("package_asset_text_encoding".into());
            }
            if asset.extension == "json" {
                crate::research_package::strict_json::decode(bytes)?;
            }
        }
        "model_artifact" => {
            let model = crate::research_package::strict_json::decode(bytes)?;
            if !model.is_object()
                || model["model_type"].as_str().is_none_or(|a| {
                    ![
                        "linear_regression",
                        "elasticnet",
                        "random_forest",
                        "hist_gradient_boosting",
                        "lightgbm",
                        "xgboost",
                        "svr",
                        "mlp",
                        "transformer",
                    ]
                    .contains(&a)
                })
            {
                return Err("package_model_schema_invalid".into());
            }
            reject_private_model_fields(&model)?;
        }
        _ => return Err("package_asset_schema_invalid".into()),
    }
    Ok(())
}
fn reject_private_model_fields(value: &Value) -> Result<(), String> {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let normalized: String = key
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect();
                if [
                    "apikey",
                    "password",
                    "authorization",
                    "secret",
                    "secretref",
                    "credential",
                    "credentialname",
                    "accesstoken",
                    "refreshtoken",
                    "connectionstring",
                    "sourcepath",
                    "filepath",
                    "directory",
                ]
                .contains(&normalized.as_str())
                {
                    return Err("package_asset_private_fields".into());
                }
                reject_private_model_fields(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_private_model_fields(value)?;
            }
        }
        Value::String(value)
            if value.trim().starts_with(['/', '\\'])
                || (value.trim().len() > 1 && value.trim().as_bytes()[1] == b':') =>
        {
            return Err("package_asset_private_fields".into())
        }
        _ => {}
    }
    Ok(())
}

pub fn inspect(bytes: &[u8]) -> Result<Verified, String> {
    let contents = crate::research_package::archive::read_entries(
        bytes,
        MAX_TOTAL,
        super::super::MAX_TOTAL,
        allowed_name,
    )?;
    let manifest: Manifest = serde_json::from_value(crate::research_package::strict_json::decode(
        contents
            .get("manifest.json")
            .ok_or("package_manifest_missing")?,
    )?)
    .map_err(|_| "package_manifest_invalid")?;
    if manifest.schema_version != VERSION
        || manifest.rights_status != "user_declared_authorized"
        || manifest.assets.is_empty()
        || manifest.assets.len() > MAX_ASSETS
    {
        return Err("unsupported_managed_package_schema".into());
    }
    // Deduplicated file names must not permit hundreds of logical references
    // to expand into unbounded in-memory copies during validation/staging.
    manifest.assets.iter().try_fold(0usize, |sum, a| {
        sum.checked_add(a.bytes)
            .filter(|n| *n <= MAX_TOTAL)
            .ok_or("package_size_budget")
    })?;
    let metadata_bytes = contents
        .get("metadata.zip")
        .ok_or("package_metadata_missing")?;
    if !valid_sha(&manifest.metadata_sha256) || digest(metadata_bytes) != manifest.metadata_sha256 {
        return Err("package_hash_mismatch".into());
    }
    let metadata = crate::research_package::archive::inspect(metadata_bytes)?;
    let mut files = BTreeSet::from(["manifest.json".to_string(), "metadata.zip".into()]);
    let mut resources = BTreeSet::new();
    let mut assets = Vec::new();
    for asset in manifest.assets {
        asset.validate()?;
        if !resources.insert(asset.resource_id.clone()) {
            return Err("package_asset_identity_conflict".into());
        }
        let record = metadata
            .objects
            .iter()
            .find(|(id, _, _)| id == &asset.resource_id)
            .ok_or("package_asset_reference_missing")?;
        let valid = match asset.role.as_str() {
            "dataset_source" => record.1["kind"] == "dataset",
            "model_artifact" => record.1["kind"] == "model",
            "image_original" => {
                record.1["kind"] == "artifact" && record.1["origin_kind"] == "analysis_result"
            }
            _ => false,
        };
        if !valid {
            return Err("package_asset_role_mismatch".into());
        }
        if metadata
            .missing_dependencies
            .iter()
            .find(|d| d["resource_id"] == asset.resource_id)
            .and_then(|d| d["expected_sha256"].as_str())
            .is_some_and(|s| s != asset.sha256)
        {
            return Err("package_asset_expected_digest_conflict".into());
        }
        let name = asset.name();
        let data = contents.get(&name).ok_or("package_asset_missing")?;
        validate_bytes(&asset, data)?;
        files.insert(name);
        assets.push((asset, data.clone()));
    }
    if files.len() != contents.len() {
        return Err("package_entry_set_mismatch".into());
    }
    Ok(Verified {
        package_sha256: digest(bytes),
        metadata_bytes: metadata_bytes.clone(),
        metadata,
        assets,
    })
}

pub(super) fn prepare(
    metadata_bytes: Vec<u8>,
    mut assets: Vec<(Asset, Vec<u8>)>,
    rights: bool,
) -> Result<Prepared, String> {
    if !rights {
        return Err("package_asset_rights_confirmation_required".into());
    }
    assets.sort_by(|a, b| a.0.resource_id.cmp(&b.0.resource_id));
    if assets.is_empty() || assets.len() > MAX_ASSETS {
        return Err("package_asset_selection_invalid".into());
    }
    let manifest = Manifest {
        schema_version: VERSION.into(),
        metadata_sha256: digest(&metadata_bytes),
        assets: assets.iter().map(|(a, _)| a.clone()).collect(),
        rights_status: "user_declared_authorized".into(),
    };
    let mut files = BTreeMap::from([
        ("metadata.zip".to_string(), metadata_bytes),
        (
            "manifest.json".into(),
            serde_json::to_vec(&manifest).map_err(|_| "package_json_invalid")?,
        ),
    ]);
    for (asset, data) in assets {
        validate_bytes(&asset, &data)?;
        if let Some(previous) = files.insert(asset.name(), data.clone()) {
            if previous != data {
                return Err("package_asset_hash_mismatch".into());
            }
        }
    }
    let total = files
        .values()
        .try_fold(0usize, |n, b| n.checked_add(b.len()))
        .ok_or("package_size_budget")?;
    if total > MAX_TOTAL {
        return Err("package_size_budget".into());
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, data) in files {
        writer
            .start_file(name, options)
            .map_err(|_| "package_zip_write_failed")?;
        writer
            .write_all(&data)
            .map_err(|_| "package_zip_write_failed")?;
    }
    let bytes = writer
        .finish()
        .map_err(|_| "package_zip_write_failed")?
        .into_inner();
    let checked = inspect(&bytes)?;
    let mut summary = checked.summary();
    summary["bytes"] = json!(bytes.len());
    Ok(Prepared { bytes, summary })
}
