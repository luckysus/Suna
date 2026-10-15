//! Inspect only the already hashed bytes, never reopen an untrusted SQLite path.
use super::*;
use rusqlite::{serialize::OwnedData, Connection, DatabaseName};
use std::{
    collections::{BTreeMap, BTreeSet},
    ptr::NonNull,
};

const TABLES: [(&str, &str); 3] = [
    ("package_objects", "CREATE TABLE package_objects(original_id TEXT PRIMARY KEY,id TEXT UNIQUE NOT NULL,body_json TEXT NOT NULL,source_sha256 TEXT NOT NULL)"),
    ("package_assets", "CREATE TABLE package_assets(resource_id TEXT PRIMARY KEY REFERENCES package_objects(original_id),name TEXT NOT NULL,sha256 TEXT NOT NULL,bytes INTEGER NOT NULL,role TEXT NOT NULL)"),
    ("package_edges", "CREATE TABLE package_edges(from_id TEXT REFERENCES package_objects(id),to_id TEXT REFERENCES package_objects(id),relation TEXT,PRIMARY KEY(from_id,to_id,relation))"),
];
fn memory(bytes: &[u8]) -> Result<Connection, String> {
    if bytes.len() < 100 || bytes.len() > MAX_TOTAL || !bytes.starts_with(b"SQLite format 3\0") {
        return Err("package_isolation_conflict".into());
    }
    let mut c = Connection::open_in_memory().map_err(|_| "package_isolation_storage_error")?;
    // rusqlite's OwnedData requires SQLite's allocator, not a Rust Vec allocation.
    // The nonzero, bounded allocation is checked before copying exactly its size.
    let pointer =
        NonNull::new(unsafe { rusqlite::ffi::sqlite3_malloc64(bytes.len() as u64) }.cast::<u8>())
            .ok_or("package_isolation_storage_error")?;
    let owned = unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.as_ptr(), bytes.len());
        OwnedData::from_raw_nonnull(pointer, bytes.len())
    };
    // Ownership transfers to SQLite; the buffer is read-only and freed on close.
    c.deserialize(DatabaseName::Main, owned, true)
        .map_err(|_| "package_isolation_conflict")?;
    c.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")
        .map_err(|_| "package_isolation_conflict")?;
    schema(&c)?;
    Ok(c)
}
fn normalize(sql: &str) -> String {
    sql.chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
fn schema(c: &Connection) -> Result<(), String> {
    let mut query=c.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' LIMIT 4").map_err(|_| "package_isolation_conflict")?;
    let rows = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| "package_isolation_conflict")?;
    let mut seen = BTreeSet::new();
    for row in rows {
        let (kind, name, table, sql) = row.map_err(|_| "package_isolation_conflict")?;
        let expected = TABLES
            .iter()
            .find(|(known, _)| *known == name)
            .ok_or("package_isolation_conflict")?;
        if kind != "table"
            || table != name
            || sql.len() > 2048
            || normalize(&sql) != normalize(expected.1)
            || !seen.insert(name)
        {
            return Err("package_isolation_conflict".into());
        }
    }
    if seen.len() != TABLES.len() {
        return Err("package_isolation_conflict".into());
    }
    Ok(())
}
pub(super) fn verify(package: &Verified, bytes: &[u8]) -> Result<BTreeMap<String, String>, String> {
    let c = memory(bytes)?;
    let mut mapping = BTreeMap::new();
    let mut imported = BTreeSet::new();
    let mut query = c
        .prepare("SELECT original_id,id,body_json,source_sha256 FROM package_objects LIMIT ?1")
        .map_err(|_| "package_isolation_conflict")?;
    let rows = query
        .query_map([package.metadata.objects.len() + 1], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| "package_isolation_conflict")?;
    for row in rows {
        let (original, id, body, sha) = row.map_err(|_| "package_isolation_conflict")?;
        let expected = package
            .metadata
            .objects
            .iter()
            .find(|(known, _, _)| known == &original)
            .ok_or("package_isolation_conflict")?;
        if identity(&id).is_err()
            || sha != expected.2
            || body.len() > super::super::MAX_FILE
            || crate::research_package::strict_json::decode(body.as_bytes())
                .ok()
                .as_ref()
                != Some(&expected.1)
            || !imported.insert(id.clone())
            || mapping.insert(original, id).is_some()
        {
            return Err("package_isolation_conflict".into());
        }
    }
    if mapping.len() != package.metadata.objects.len() {
        return Err("package_isolation_conflict".into());
    }
    drop(query);
    let expected_assets: BTreeSet<_> = package
        .assets
        .iter()
        .map(|(a, _)| {
            (
                a.resource_id.clone(),
                a.name(),
                a.sha256.clone(),
                a.bytes as u64,
                a.role.clone(),
            )
        })
        .collect();
    let mut query = c
        .prepare("SELECT resource_id,name,sha256,bytes,role FROM package_assets LIMIT ?1")
        .map_err(|_| "package_isolation_conflict")?;
    let rows = query
        .query_map([package.assets.len() + 1], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|_| "package_isolation_conflict")?;
    let mut assets = BTreeSet::new();
    for row in rows {
        if !assets.insert(row.map_err(|_| "package_isolation_conflict")?) {
            return Err("package_isolation_conflict".into());
        }
    }
    if assets != expected_assets {
        return Err("package_isolation_conflict".into());
    }
    drop(query);
    let expected_edges: BTreeSet<_> = package
        .metadata
        .edges
        .iter()
        .map(|e| {
            let from = e["from_id"].as_str().ok_or("package_isolation_conflict")?;
            let to = e["to_id"].as_str().ok_or("package_isolation_conflict")?;
            Ok((
                mapping
                    .get(from)
                    .ok_or("package_isolation_conflict")?
                    .clone(),
                mapping.get(to).ok_or("package_isolation_conflict")?.clone(),
                e["relation"]
                    .as_str()
                    .ok_or("package_isolation_conflict")?
                    .to_string(),
            ))
        })
        .collect::<Result<_, &str>>()?;
    let mut query = c
        .prepare("SELECT from_id,to_id,relation FROM package_edges LIMIT ?1")
        .map_err(|_| "package_isolation_conflict")?;
    let rows = query
        .query_map([package.metadata.edges.len() + 1], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|_| "package_isolation_conflict")?;
    let mut edges = BTreeSet::new();
    for row in rows {
        if !edges.insert(row.map_err(|_| "package_isolation_conflict")?) {
            return Err("package_isolation_conflict".into());
        }
    }
    if edges != expected_edges {
        return Err("package_isolation_conflict".into());
    }
    drop(query);
    c.close().map_err(|_| "package_isolation_storage_error")?;
    Ok(mapping)
}
