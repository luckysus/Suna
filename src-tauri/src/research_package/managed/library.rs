//! Read-only, bounded access to saved isolations. No live database is accepted.
use super::*;
use crate::permissions::path::AuthorizedRoots;
use std::path::{Path, PathBuf};

const PAGE: usize = 25;
const MAX_ISOLATIONS: usize = 10_000;
const RECEIPT_BUDGET: usize = 1024 * 1024;

fn parent(data_root: &Path) -> Result<Option<PathBuf>, String> {
    let guard = AuthorizedRoots::new(vec![data_root.to_path_buf()])
        .map_err(|_| "package_isolation_path_invalid")?;
    let parent = data_root.join("research-package-imports");
    guard
        .authorize(&parent)
        .map_err(|_| "package_isolation_path_invalid")?;
    match std::fs::symlink_metadata(&parent) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(_) if staging::ordinary_directory(&parent) => Ok(Some(parent)),
        _ => Err("package_isolation_path_invalid".into()),
    }
}
fn offset(value: u32) -> Result<usize, String> {
    let value = value as usize;
    if value > MAX_ISOLATIONS || value % PAGE != 0 {
        return Err("package_isolation_page_invalid".into());
    }
    Ok(value)
}
pub fn list(data_root: &Path, start: u32) -> Result<Value, String> {
    let start = offset(start)?;
    let mut entries = Vec::new();
    if let Some(parent) = parent(data_root)? {
        // Scan only direct entries in this application-owned namespace.
        for (i, entry) in std::fs::read_dir(&parent)
            .map_err(|_| "package_isolation_read_failed")?
            .enumerate()
        {
            if i >= MAX_ISOLATIONS {
                return Err("package_isolation_inventory_budget".into());
            }
            let entry = entry.map_err(|_| "package_isolation_read_failed")?;
            let Some(id) = entry
                .file_name()
                .to_str()
                .map(str::to_owned)
                .filter(|id| valid_sha(id))
            else {
                continue;
            };
            entries.push((id, entry.path()));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let total = entries.len();
    let items: Vec<_> = entries
        .into_iter()
        .skip(start)
        .take(PAGE)
        .map(|(id, root)| {
            let state = if staging::ordinary_directory(&root) {
                staging::read_owned(&root, "receipt.json", RECEIPT_BUDGET)
                    .and_then(|bytes| crate::research_package::strict_json::decode(&bytes))
                    .ok()
                    .filter(|r| {
                        r["schemaVersion"] == "research-managed-isolation.v1"
                            && r["packageSha256"] == id
                    })
                    .map(|r| match r.get("originalArchiveStored") {
                        Some(Value::Bool(true)) => "stored_unverified",
                        None | Some(Value::Bool(false)) => "requires_original_archive",
                        _ => "incomplete_or_damaged",
                    })
                    .unwrap_or("incomplete_or_damaged")
            } else {
                "incomplete_or_damaged"
            };
            json!({"isolationId":id,"state":state})
        })
        .collect();
    Ok(
        json!({"schemaVersion":"research-managed-isolations.v1","readOnly":true,"writesToLiveWorkspace":false,
        "offset":start,"total":total,"hasMore":start+items.len()<total,"items":items}),
    )
}

pub fn open(data_root: &Path, id: &str, start: u32) -> Result<Value, String> {
    let start = offset(start)?;
    if !valid_sha(id) {
        return Err("package_isolation_id_invalid".into());
    }
    let parent = parent(data_root)?.ok_or("package_isolation_incomplete")?;
    let root = parent.join(id);
    AuthorizedRoots::new(vec![data_root.to_path_buf()])
        .map_err(|_| "package_isolation_path_invalid")?
        .authorize(&root)
        .map_err(|_| "package_isolation_path_invalid")?;
    if !staging::ordinary_directory(&root) {
        return Err("package_isolation_path_invalid".into());
    }
    let receipt = crate::research_package::strict_json::decode(&staging::read_owned(
        &root,
        "receipt.json",
        RECEIPT_BUDGET,
    )?)?;
    if receipt["schemaVersion"] != "research-managed-isolation.v1" || receipt["packageSha256"] != id
    {
        return Err("package_isolation_conflict".into());
    }
    if receipt["originalArchiveStored"] != true {
        return Err("package_isolation_original_required".into());
    }
    let bytes = staging::read_owned(&root, "package.zip", MAX_TOTAL)?;
    if digest(&bytes) != id {
        return Err("package_isolation_conflict".into());
    }
    let package = archive::inspect(&bytes)?;
    let isolation = staging::verify_existing(&package, &root)?;
    let total = package.metadata.objects.len();
    let records:Vec<_>=package.metadata.objects.iter().skip(start).take(PAGE).map(|(id,record,sha)|
        json!({"originalId":id,"importedId":isolation["idMapping"][id],"recordSha256":sha,"record":record})).collect();
    Ok(
        json!({"schemaVersion":"research-managed-isolation-view.v1","readOnly":true,"isolation":isolation,
        "offset":start,"total":total,"hasMore":start+records.len()<total,"records":records}),
    )
}
