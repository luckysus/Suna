//! Workspace-scoped inventory. Only explicitly selected managed sources are copied.
use super::*;
use crate::storage::repositories::{research_artifacts, steel, steel_models};
use rusqlite::{params, Connection};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    resource_id: String,
    role: String,
    name: String,
    sha256: String,
    bytes: Option<usize>,
    availability: &'static str,
}
enum Source {
    File(File),
    Model(String),
    Bytes(Vec<u8>),
}
pub struct Capture {
    projection: crate::research_package::projection::Projection,
    candidates: BTreeMap<String, (Candidate, Option<Source>, String)>,
}
fn managed_dataset(relative: &Path) -> bool {
    [
        "objects/material-data",
        "objects/literature-data",
        "material-datasets",
        "research-datasets",
    ]
    .iter()
    .any(|p| relative.starts_with(p))
}
fn open_managed(root: &Path, path: &Path, dataset: bool) -> Option<File> {
    let roots = crate::permissions::path::AuthorizedRoots::new(vec![root.to_path_buf()]).ok()?;
    let authorized = roots.authorize(path).ok()?;
    let relative = authorized
        .canonical_path()
        .strip_prefix(root.canonicalize().ok()?)
        .ok()?
        .to_path_buf();
    if dataset && !managed_dataset(&relative) {
        return None;
    }
    if !dataset && !relative.starts_with("microstructure") {
        return None;
    }
    let before = std::fs::symlink_metadata(authorized.canonical_path()).ok()?;
    if !before.is_file() || before.file_type().is_symlink() {
        return None;
    }
    let file = File::open(authorized.canonical_path()).ok()?;
    #[cfg(windows)]
    roots.verify_opened_file(&authorized, &file).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = file.metadata().ok()?;
        if (before.dev(), before.ino()) != (actual.dev(), actual.ino()) {
            return None;
        }
    }
    Some(file)
}
fn file_candidate(
    root: &Path,
    id: &str,
    role: &str,
    name: &str,
    path: PathBuf,
    sha: &str,
) -> (Candidate, Option<Source>, String) {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let file = open_managed(root, &path, role == "dataset_source");
    let size = file
        .as_ref()
        .and_then(|f| f.metadata().ok())
        .and_then(|m| usize::try_from(m.len()).ok());
    let schema = Asset {
        resource_id: id.into(),
        role: role.into(),
        extension: extension.clone(),
        sha256: sha.into(),
        bytes: size.unwrap_or(0),
    };
    let availability = if file.is_none() {
        "unavailable_or_external"
    } else if schema.validate().is_err() {
        "unsupported_or_over_budget"
    } else {
        "eligible_unverified"
    };
    let source = if availability == "eligible_unverified" {
        file.map(Source::File)
    } else {
        None
    };
    (
        Candidate {
            resource_id: id.into(),
            role: role.into(),
            name: super::super::label(Some(name)),
            sha256: sha.into(),
            bytes: size,
            availability,
        },
        source,
        extension,
    )
}

fn snapshot(c: &Connection, ws: &str, root: &Path, project: &str) -> Result<Capture, String> {
    let mut projection = crate::research_package::projection::from_connection(c, ws, project)?;
    // Workflow inputs can be frozen sources without a separate user attachment.
    // Collect their real identities/hashes from verified project artifacts;
    // never infer them from a current selection or a model's display name.
    let frozen: Vec<_> = projection
        .records
        .iter()
        .filter(|r| r["origin_kind"] == "dataset_snapshot")
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect();
    for artifact_id in frozen {
        let artifact = research_artifacts::get(c, ws, &artifact_id)?;
        let payload = artifact.payload.ok_or("package_artifact_schema_invalid")?;
        let Some(id) = payload["dataset_id"]
            .as_str()
            .filter(|id| identity(id).is_ok())
        else {
            continue;
        };
        let Some(sha) = payload["source_sha256"]
            .as_str()
            .filter(|sha| valid_sha(sha))
        else {
            continue;
        };
        if let Some(dependency) = projection
            .dependencies
            .iter_mut()
            .find(|d| d["resource_id"] == id)
        {
            if dependency["expected_sha256"]
                .as_str()
                .is_some_and(|expected| expected != sha)
            {
                return Err("package_dependency_digest_conflict".into());
            }
            dependency["expected_sha256"] = json!(sha);
        } else {
            projection.dependencies.push(
                json!({"resource_id":id,"kind":"dataset","status":"missing","expected_sha256":sha}),
            );
        }
        if !projection.records.iter().any(|r| r["id"] == id) {
            projection
                .records
                .push(json!({"id":id,"kind":"dataset","source_sha256":sha}));
            projection
                .edges
                .push(json!({"from_id":id,"to_id":artifact_id,"relation":"uses_dataset"}));
        }
    }
    let mut candidates = BTreeMap::new();
    for record in &projection.records {
        let id = record["id"].as_str().ok_or("package_resource_id_invalid")?;
        match record["kind"].as_str() {
            Some("dataset") => {
                let Some(data) = steel::get(c, ws, id)? else {
                    candidates.insert(
                        id.into(),
                        (
                            Candidate {
                                resource_id: id.into(),
                                role: "dataset_source".into(),
                                name: "来源数据未提供".into(),
                                sha256: record["source_sha256"].as_str().unwrap_or("").into(),
                                bytes: None,
                                availability: "unavailable_or_external",
                            },
                            None,
                            "csv".into(),
                        ),
                    );
                    continue;
                };
                if projection
                    .dependencies
                    .iter()
                    .find(|d| d["resource_id"] == id)
                    .and_then(|d| d["expected_sha256"].as_str())
                    .is_some_and(|sha| sha != data.source_sha256)
                {
                    return Err("package_dependency_digest_conflict".into());
                }
                candidates.insert(
                    id.to_string(),
                    file_candidate(
                        root,
                        id,
                        "dataset_source",
                        &data.source_name,
                        PathBuf::from(data.source_path),
                        &data.source_sha256,
                    ),
                );
            }
            Some("model") => {
                let (kind,sha,size):(String,String,Option<u64>)=c.query_row("SELECT kind,model_sha256,length(artifact_json) FROM steel_models WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|_|"package_model_missing")?;
                let size = size.and_then(|v| usize::try_from(v).ok());
                let eligible = matches!(
                    kind.as_str(),
                    "linear_artifact" | "sklearn_artifact" | "transformer_artifact"
                ) && valid_sha(&sha)
                    && size.is_some_and(|v| v > 0 && v <= super::super::MAX_FILE);
                candidates.insert(
                    id.into(),
                    (
                        Candidate {
                            resource_id: id.into(),
                            role: "model_artifact".into(),
                            name: super::super::label(record["name"].as_str()),
                            sha256: sha,
                            bytes: size,
                            availability: if eligible {
                                "eligible_unverified"
                            } else {
                                "unsupported_or_over_budget"
                            },
                        },
                        eligible.then(|| Source::Model(id.into())),
                        "json".into(),
                    ),
                );
            }
            _ => {}
        }
    }
    let ids: BTreeSet<_> = projection
        .records
        .iter()
        .filter_map(|r| r["id"].as_str())
        .collect();
    let mut query=c.prepare("SELECT source_artifact_id,storage_ref,source_sha256 FROM microstructure_images WHERE workspace_id=?1 AND project_id=?2 ORDER BY id LIMIT 513").map_err(|_|"package_storage_error")?;
    let images = query
        .query_map(params![ws, project], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "package_storage_error")?;
    for image in images {
        let (id, path, sha) = image.map_err(|_| "package_storage_error")?;
        if !ids.contains(id.as_str()) {
            return Err("package_asset_reference_missing".into());
        }
        if candidates
            .insert(
                id.clone(),
                file_candidate(
                    root,
                    &id,
                    "image_original",
                    "原始显微图像",
                    root.join(path),
                    &sha,
                ),
            )
            .is_some()
        {
            return Err("package_asset_identity_conflict".into());
        }
    }
    if candidates.len() > MAX_ASSETS {
        return Err("package_asset_inventory_budget".into());
    }
    Ok(Capture {
        projection,
        candidates,
    })
}
pub fn inventory(c: &Connection, ws: &str, root: &Path, project: &str) -> Result<Value, String> {
    research_artifacts::atomic(c, || {
        let captured = snapshot(c, ws, root, project)?;
        Ok(
            json!({"sourceProjectId":project,"scope":"managed_assets_inventory","assets":captured.candidates.values().map(|(v,_,_)|v).collect::<Vec<_>>(),"rawContentIncluded":false,"rightsIndependentlyVerified":false}),
        )
    })
}
pub fn capture(
    c: &Connection,
    ws: &str,
    root: &Path,
    project: &str,
    selected: &[String],
    rights: bool,
) -> Result<Capture, String> {
    if !rights {
        return Err("package_asset_rights_confirmation_required".into());
    }
    let unique: BTreeSet<String> = selected.iter().cloned().collect();
    if unique.len() != selected.len()
        || selected.is_empty()
        || selected.len() > MAX_ASSETS
        || selected.iter().any(|id| identity(id).is_err())
    {
        return Err("package_asset_selection_invalid".into());
    }
    research_artifacts::atomic(c, || {
        let mut result = snapshot(c, ws, root, project)?;
        if selected
            .iter()
            .any(|id| !result.candidates.contains_key(id))
        {
            return Err("package_asset_not_in_project".into());
        }
        result.candidates.retain(|id, _| unique.contains(id));
        let size = result
            .candidates
            .values()
            .try_fold(0usize, |sum, (item, _, _)| {
                sum.checked_add(item.bytes.unwrap_or(MAX_TOTAL))
            })
            .ok_or("package_size_budget")?;
        if size > MAX_TOTAL {
            return Err("package_size_budget".into());
        }
        for (item, source, _) in result.candidates.values_mut() {
            if item.availability != "eligible_unverified" {
                return Err("package_asset_unavailable".into());
            }
            if let Some(Source::Model(id)) = source {
                let model = steel_models::get(c, ws, id)?.ok_or("package_model_missing")?;
                steel_models::scientific_cards_from_record(&model)?;
                let body = model
                    .artifact_json
                    .ok_or("package_model_missing")?
                    .into_bytes();
                if body.len() != item.bytes.unwrap_or(0) || digest(&body) != item.sha256 {
                    return Err("package_asset_hash_mismatch".into());
                }
                *source = Some(Source::Bytes(body));
            }
        }
        Ok(result)
    })
}
impl Capture {
    /// File bytes, ZIP assembly and hashing run after the application DB lock is released.
    pub fn prepare(self) -> Result<Prepared, String> {
        let metadata = crate::research_package::archive::export(self.projection)?;
        let mut assets = Vec::new();
        for (_, (item, source, extension)) in self.candidates {
            let bytes = match source.ok_or("package_asset_unavailable")? {
                Source::Bytes(bytes) => bytes,
                Source::File(mut file) => {
                    let before = file.metadata().map_err(|_| "package_asset_unavailable")?;
                    if before.len() > MAX_ASSET as u64 {
                        return Err("package_size_budget".into());
                    }
                    let mut bytes = Vec::new();
                    file.by_ref()
                        .take(MAX_ASSET as u64 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| "package_asset_read_failed")?;
                    let after = file.metadata().map_err(|_| "package_asset_unavailable")?;
                    if before.len() != after.len()
                        || before.modified().ok() != after.modified().ok()
                    {
                        return Err("package_asset_changed".into());
                    }
                    bytes
                }
                Source::Model(_) => return Err("package_model_missing".into()),
            };
            let asset = Asset {
                resource_id: item.resource_id,
                role: item.role,
                extension,
                sha256: item.sha256,
                bytes: item.bytes.unwrap_or(0),
            };
            assets.push((asset, bytes));
        }
        archive::prepare(metadata, assets, true)
    }
}
