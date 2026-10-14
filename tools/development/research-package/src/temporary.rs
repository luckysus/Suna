//! Simulation opens its own temporary SQLite. No live DB connection is accepted.
use crate::{valid_sha, ValidatedPackage};
use rusqlite::{params, Connection, TransactionBehavior};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub fn simulate_import(package: &ValidatedPackage) -> Result<Value, String> {
    let mut c = Connection::open_in_memory().map_err(|_| "package_temporary_storage_error")?;
    let first = import(&mut c, package)?;
    let again = import(&mut c, package)?;
    if first != again {
        return Err("package_idempotence_failed".into());
    }
    Ok(
        json!({"schemaVersion":"research-package-dry-run.v1","receiptSha256":package.receipt_sha256,
        "sourceProjectId":package.source_project_id,"metadataRecordCount":package.objects.len(),"edgeCount":package.edges.len(),
        "missingDependencies":package.missing_dependencies,"status":"metadata_only_missing_assets",
        "writesToLiveWorkspace":false,"assetsRestored":false,"idempotenceVerified":true,"idMapping":first}),
    )
}
fn import(c: &mut Connection, p: &ValidatedPackage) -> Result<BTreeMap<String, String>, String> {
    if !valid_sha(&p.receipt_sha256) {
        return Err("package_receipt_invalid".into());
    }
    if p.objects
        .iter()
        .any(|(id, body, _)| crate::validation::record(body).as_ref() != Ok(id))
    {
        return Err("package_resource_identity_mismatch".into());
    }
    crate::validation::lineage(&p.objects, &p.missing_dependencies, &p.edges)?;
    if p.objects.iter().any(|(_, _, sha)| !valid_sha(sha)) {
        return Err("package_record_digest_invalid".into());
    }
    let unsafe_table:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name NOT IN ('package_receipts','package_metadata','package_edges'))",[],|r|r.get(0)).map_err(|_|"package_temporary_storage_error")?;
    if unsafe_table || !c.is_autocommit() {
        return Err("live_database_import_forbidden".into());
    }
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| "package_temporary_storage_error")?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS package_receipts(sha256 TEXT PRIMARY KEY,source_project_id TEXT NOT NULL,missing_json TEXT NOT NULL);CREATE TABLE IF NOT EXISTS package_metadata(id TEXT PRIMARY KEY,original_id TEXT NOT NULL,receipt_sha256 TEXT NOT NULL,source_sha256 TEXT NOT NULL,metadata_json TEXT NOT NULL,status TEXT NOT NULL,UNIQUE(receipt_sha256,original_id));CREATE TABLE IF NOT EXISTS package_edges(receipt_sha256 TEXT,from_id TEXT,to_id TEXT,relation TEXT,PRIMARY KEY(receipt_sha256,from_id,to_id,relation));").map_err(|_|"package_temporary_storage_error")?;
    let existing: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM package_receipts WHERE sha256=?1)",
            [&p.receipt_sha256],
            |r| r.get(0),
        )
        .map_err(|_| "package_temporary_storage_error")?;
    let mut mapping = BTreeMap::new();
    if existing {
        let source: String = tx
            .query_row(
                "SELECT source_project_id FROM package_receipts WHERE sha256=?1",
                [&p.receipt_sha256],
                |r| r.get(0),
            )
            .map_err(|_| "package_temporary_storage_error")?;
        if source != p.source_project_id {
            return Err("package_receipt_conflict".into());
        }
        for (id, _, sha) in &p.objects {
            let (new_id,stored_sha):(String,String)=tx.query_row("SELECT id,source_sha256 FROM package_metadata WHERE receipt_sha256=?1 AND original_id=?2",params![p.receipt_sha256,id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|"package_receipt_conflict")?;
            if &stored_sha != sha {
                return Err("package_receipt_conflict".into());
            }
            mapping.insert(id.clone(), new_id);
        }
        tx.commit().map_err(|_| "package_temporary_storage_error")?;
        return Ok(mapping);
    }
    tx.execute(
        "INSERT INTO package_receipts VALUES(?1,?2,?3)",
        params![
            p.receipt_sha256,
            p.source_project_id,
            serde_json::to_string(&p.missing_dependencies).map_err(|_| "package_json_invalid")?
        ],
    )
    .map_err(|_| "package_temporary_storage_error")?;
    for (id, body, sha) in &p.objects {
        let new = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO package_metadata VALUES(?1,?2,?3,?4,?5,'metadata_only_missing_assets')",
            params![new, id, p.receipt_sha256, sha, body.to_string()],
        )
        .map_err(|_| "package_temporary_storage_error")?;
        mapping.insert(id.clone(), new);
    }
    for edge in &p.edges {
        let from = crate::identity(edge["from_id"].as_str().ok_or("package_edge_invalid")?)?;
        let to = crate::identity(edge["to_id"].as_str().ok_or("package_edge_invalid")?)?;
        tx.execute(
            "INSERT INTO package_edges VALUES(?1,?2,?3,?4)",
            params![
                p.receipt_sha256,
                mapping[&from],
                mapping[&to],
                edge["relation"].as_str()
            ],
        )
        .map_err(|_| "package_temporary_storage_error")?;
    }
    tx.commit().map_err(|_| "package_temporary_storage_error")?;
    Ok(mapping)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package() -> ValidatedPackage {
        let project = uuid::Uuid::new_v4().to_string();
        let artifact = uuid::Uuid::new_v4().to_string();
        let bytes = crate::archive::export(crate::projection::Projection {
            project_id: project.clone(),
            records: vec![
                json!({"id":project,"kind":"project"}),
                json!({"id":artifact,"kind":"artifact"}),
            ],
            dependencies: vec![],
            edges: vec![json!({"from_id":project,"to_id":artifact,"relation":"generated_by_run"})],
            app_version: "synthetic-test".into(),
        })
        .unwrap();
        crate::archive::inspect(&bytes).unwrap()
    }
    #[test]
    fn temporary_metadata_and_source_hashes_reopen_without_duplicate_ids() {
        let path =
            std::env::temp_dir().join(format!("suna-归档模拟-{}.sqlite3", uuid::Uuid::new_v4()));
        let mut c = Connection::open(&path).unwrap();
        let p = package();
        let mapping = import(&mut c, &p).unwrap();
        assert_eq!(import(&mut c, &p).unwrap(), mapping);
        drop(c);
        let mut reopened = Connection::open(&path).unwrap();
        assert_eq!(import(&mut reopened, &p).unwrap(), mapping);
        for (id, _, sha) in &p.objects {
            assert_eq!(
                reopened
                    .query_row(
                        "SELECT source_sha256 FROM package_metadata WHERE original_id=?1",
                        [id],
                        |r| r.get::<_, String>(0)
                    )
                    .unwrap(),
                *sha
            );
        }
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn insertion_failure_rolls_back_created_schema_and_metadata() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE package_edges(receipt_sha256 TEXT,from_id TEXT,to_id TEXT,relation TEXT,PRIMARY KEY(receipt_sha256,from_id,to_id,relation));CREATE TRIGGER synthetic_import_fault BEFORE INSERT ON package_edges BEGIN SELECT RAISE(ABORT,'synthetic-fault'); END;").unwrap();
        assert!(import(&mut c, &package()).is_err());
        let tables:u32 = c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('package_metadata','package_receipts')",[],|r|r.get(0)).unwrap();
        assert_eq!(tables, 0);
        assert!(c.is_autocommit());
    }
    #[test]
    fn application_tables_and_nested_transactions_are_never_import_targets() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute("CREATE TABLE settings(secret TEXT)", []).unwrap();
        assert_eq!(
            import(&mut c, &package()).unwrap_err(),
            "live_database_import_forbidden"
        );
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("BEGIN").unwrap();
        assert_eq!(
            import(&mut c, &package()).unwrap_err(),
            "live_database_import_forbidden"
        );
        c.execute_batch("ROLLBACK").unwrap();
    }
}
