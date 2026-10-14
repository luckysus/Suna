//! In-memory ZIP writer/reader. Canonical ASCII entry names eliminate path aliases.
use crate::{
    digest, hash, identity, valid_sha, Entry, Manifest, ValidatedPackage, MAX_ENTRIES, MAX_FILE,
    MAX_TOTAL, VERSION,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};
pub fn entry_name(name: &str) -> bool {
    if name == "manifest.json" {
        return true;
    }
    name.strip_prefix("records/")
        .and_then(|s| s.strip_suffix(".json"))
        .is_some_and(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_digit()))
}
fn classic_envelope(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() < 22 || bytes.len() > MAX_TOTAL {
        return Err("package_size_budget".into());
    }
    let lower = bytes.len().saturating_sub(65557);
    let at = (lower..=bytes.len() - 22)
        .rev()
        .find(|&at| {
            bytes[at..at + 4] == *b"PK\x05\x06"
                && at + 22 + u16::from_le_bytes([bytes[at + 20], bytes[at + 21]]) as usize
                    == bytes.len()
        })
        .ok_or("package_zip_invalid")?;
    let u16_at = |n| u16::from_le_bytes([bytes[at + n], bytes[at + n + 1]]);
    let u32_at = |n| {
        u32::from_le_bytes([
            bytes[at + n],
            bytes[at + n + 1],
            bytes[at + n + 2],
            bytes[at + n + 3],
        ]) as usize
    };
    let count = u16_at(10) as usize;
    if u16_at(4) != 0
        || u16_at(6) != 0
        || u16_at(8) as usize != count
        || !(2..=MAX_ENTRIES).contains(&count)
        || u32_at(12).checked_add(u32_at(16)) != Some(at)
    {
        return Err("package_zip_envelope_invalid".into());
    }
    Ok(())
}
pub fn inspect(bytes: &[u8]) -> Result<ValidatedPackage, String> {
    classic_envelope(bytes)?;
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| "package_zip_invalid")?;
    if zip.len() > MAX_ENTRIES {
        return Err("package_size_budget".into());
    }
    let mut contents = BTreeMap::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|_| "package_zip_invalid")?;
        let name = file.name().to_string();
        if !entry_name(&name)
            || file.is_dir()
            || file.encrypted()
            || !matches!(
                file.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err("package_zip_entry_invalid".into());
        }
        let kind = file.unix_mode().unwrap_or(0) & 0o170000;
        if kind != 0 && kind != 0o100000 {
            return Err("package_zip_non_regular".into());
        }
        let size = usize::try_from(file.size()).map_err(|_| "package_size_budget")?;
        total = total
            .checked_add(size)
            .filter(|n| *n <= MAX_TOTAL)
            .ok_or("package_size_budget")?;
        if size > MAX_FILE || file.size() > file.compressed_size().max(1).saturating_mul(128) {
            return Err("package_expansion_budget".into());
        }
        let mut data = Vec::new();
        file.by_ref()
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|_| "package_zip_read_failed")?;
        if data.len() != size || data.len() > MAX_FILE {
            return Err("package_expansion_budget".into());
        }
        if contents.insert(name, data).is_some() {
            return Err("duplicate_package_entry".into());
        }
    }
    let raw = contents
        .get("manifest.json")
        .ok_or("package_manifest_missing")?;
    let value = crate::strict_json::decode(raw)?;
    let manifest: Manifest =
        serde_json::from_value(value.clone()).map_err(|_| "package_manifest_invalid")?;
    if manifest.schema_version != VERSION
        || manifest.scope != "metadata_only"
        || manifest.entries.len() + 1 != contents.len()
    {
        return Err("unsupported_package_schema".into());
    }
    if manifest.app_version.chars().count() > 128 {
        return Err("package_manifest_invalid".into());
    }
    let project = identity(&manifest.source_project_id)?;
    let mut seen = BTreeSet::new();
    let mut objects = Vec::new();
    for entry in manifest.entries {
        if !entry_name(&entry.name)
            || entry.name == "manifest.json"
            || entry.bytes > MAX_FILE
            || !valid_sha(&entry.sha256)
            || !seen.insert(entry.name.clone())
        {
            return Err("package_entry_schema_invalid".into());
        }
        let body = contents
            .get(&entry.name)
            .ok_or("package_entry_set_mismatch")?;
        if body.len() != entry.bytes || digest(body) != entry.sha256 {
            return Err("package_hash_mismatch".into());
        }
        let body = crate::strict_json::decode(body)?;
        let id = crate::validation::record(&body)?;
        objects.push((id, body, entry.sha256));
    }
    if !objects
        .iter()
        .any(|(id, body, _)| id == &project && body["kind"] == "project")
        || objects
            .iter()
            .filter(|(_, body, _)| body["kind"] == "project")
            .count()
            != 1
    {
        return Err("package_project_reference_missing".into());
    }
    crate::validation::lineage(&objects, &manifest.external_dependencies, &manifest.edges)?;
    Ok(ValidatedPackage {
        receipt_sha256: hash(&value)?,
        source_project_id: project,
        objects,
        missing_dependencies: manifest.external_dependencies,
        edges: manifest.edges,
    })
}
pub fn export(projection: crate::projection::Projection) -> Result<Vec<u8>, String> {
    if projection.records.is_empty() || projection.records.len() >= MAX_ENTRIES {
        return Err("package_object_budget".into());
    }
    let mut entries = Vec::new();
    let mut bodies = Vec::new();
    let mut total = 0usize;
    for (i, record) in projection.records.into_iter().enumerate() {
        crate::validation::record(&record)?;
        let body = serde_json::to_vec(&record).map_err(|_| "package_json_invalid")?;
        total = total
            .checked_add(body.len())
            .filter(|n| *n <= MAX_TOTAL)
            .ok_or("package_size_budget")?;
        if body.len() > MAX_FILE {
            return Err("package_file_budget".into());
        }
        entries.push(Entry {
            name: format!("records/{i:04}.json"),
            bytes: body.len(),
            sha256: digest(&body),
        });
        bodies.push(body);
    }
    let manifest = Manifest {
        schema_version: VERSION.into(),
        scope: "metadata_only".into(),
        source_project_id: projection.project_id,
        entries,
        external_dependencies: projection.dependencies,
        edges: projection.edges,
        app_version: projection.app_version,
    };
    let body = serde_json::to_vec(&manifest).map_err(|_| "package_json_invalid")?;
    if body.len() > MAX_FILE || total.checked_add(body.len()).is_none_or(|n| n > MAX_TOTAL) {
        return Err("package_size_budget".into());
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer
        .start_file("manifest.json", options)
        .map_err(|_| "package_zip_write_failed")?;
    writer
        .write_all(&body)
        .map_err(|_| "package_zip_write_failed")?;
    for (entry, body) in manifest.entries.iter().zip(bodies) {
        writer
            .start_file(&entry.name, options)
            .map_err(|_| "package_zip_write_failed")?;
        writer
            .write_all(&body)
            .map_err(|_| "package_zip_write_failed")?;
    }
    let bytes = writer
        .finish()
        .map_err(|_| "package_zip_write_failed")?
        .into_inner();
    inspect(&bytes)?;
    Ok(bytes)
}
