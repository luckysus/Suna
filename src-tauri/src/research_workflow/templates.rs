//! Immutable template versions in the original SQLite repository.
use crate::research_workflow::schema::{FrozenInput, Template};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn workspace(ws: &str) -> Result<(), String> {
    if ws.is_empty()
        || ws.len() > 128
        || !ws
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("workflow_workspace_invalid".into());
    }
    Ok(())
}
fn validate(template: &Template) -> Result<(), String> {
    // Template saving validates configuration only, never invents a dataset or
    // an execution-ready preview. Real run preflight still reads the repository.
    crate::research_workflow::schema::dry_run(
        template,
        &FrozenInput {
            project_id: "11111111-1111-4111-8111-111111111111".into(),
            dataset_id: "22222222-2222-4222-8222-222222222222".into(),
            dataset_sha256: "a".repeat(64),
        },
    )
    .map_err(str::to_owned)?;
    Ok(())
}
pub fn save(
    c: &mut Connection,
    ws: &str,
    template: &Template,
    expected_revision: u64,
) -> Result<Value, String> {
    workspace(ws)?;
    validate(template)?;
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| "workflow_storage_error")?;
    let previous:u64=tx.query_row("SELECT COALESCE(MAX(revision),0) FROM workflow_templates WHERE workspace_id=?1 AND id=?2",params![ws,template.id],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
    if previous != expected_revision {
        return Err("workflow_template_revision_conflict".into());
    }
    let total: u64 = tx
        .query_row(
            "SELECT COUNT(*) FROM workflow_templates WHERE workspace_id=?1",
            [ws],
            |r| r.get(0),
        )
        .map_err(|_| "workflow_storage_error")?;
    if total >= 10000 || previous >= 256 {
        return Err("workflow_template_version_budget".into());
    }
    let mut next = template.clone();
    next.revision = previous + 1;
    let body = serde_json::to_string(&next).map_err(|_| "workflow_template_invalid")?;
    if body.len() > 128 * 1024 {
        return Err("workflow_template_size_budget".into());
    }
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    tx.execute(
        "INSERT INTO workflow_templates VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            ws,
            next.id,
            next.revision,
            next.name,
            next.schema_version,
            body,
            sha,
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .map_err(|_| "workflow_storage_error")?;
    tx.commit().map_err(|_| "workflow_storage_error")?;
    get(c, ws, &next.id, next.revision)
}
pub fn get(c: &Connection, ws: &str, id: &str, revision: u64) -> Result<Value, String> {
    workspace(ws)?;
    let stored:Option<(String,String)>=c.query_row("SELECT payload_json,content_sha256 FROM workflow_templates WHERE workspace_id=?1 AND id=?2 AND revision=?3",params![ws,id,revision],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|"workflow_storage_error")?;
    let (body, sha) = stored.ok_or("workflow_template_not_found")?;
    if format!("{:x}", Sha256::digest(body.as_bytes())) != sha {
        return Err("workflow_template_hash_mismatch".into());
    }
    let template: Template =
        serde_json::from_str(&body).map_err(|_| "workflow_template_invalid")?;
    validate(&template)?;
    if template.id != id || template.revision != revision {
        return Err("workflow_template_identity_mismatch".into());
    }
    Ok(
        json!({"template":template,"contentSha256":sha,"validationStatus":"saved_schema_only","repositoryDryRunRequired":true}),
    )
}

/// Paginate immutable versions; return no paths, rows or executable content.
pub fn list(c: &Connection, ws: &str, limit: u32, offset: u32) -> Result<Vec<Value>, String> {
    workspace(ws)?;
    let mut q=c.prepare("SELECT id,revision FROM workflow_templates WHERE workspace_id=?1 ORDER BY created_at DESC,id,revision DESC LIMIT ?2 OFFSET ?3").map_err(|_|"workflow_storage_error")?;
    let ids = q
        .query_map(params![ws, limit.clamp(1, 50), offset.min(10000)], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?))
        })
        .map_err(|_| "workflow_storage_error")?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "workflow_storage_error")?;
    ids.into_iter().map(|(id,revision)|{
        let saved=get(c,ws,&id,revision)?;
        Ok(json!({"id":id,"revision":revision,"name":saved["template"]["name"],"contentSha256":saved["contentSha256"]}))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_workflow::testing::fixture;
    #[test]
    fn saves_independent_versions_and_reopens_without_overwriting_history() {
        let f = fixture();
        let path = f.root.join("模板版本.sqlite3");
        let mut c = Connection::open(&path).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let first = save(&mut c, "local", &f.template, 0).unwrap();
        let mut next = f.template.clone();
        next.name = "新的合成模板名称".into();
        let second = save(&mut c, "local", &next, 1).unwrap();
        assert_eq!(second["template"]["revision"], 2);
        assert_ne!(first["contentSha256"], second["contentSha256"]);
        assert_eq!(get(&c, "local", &f.template.id, 1).unwrap(), first);
        drop(c);
        let reopened = Connection::open(path).unwrap();
        assert_eq!(get(&reopened, "local", &f.template.id, 1).unwrap(), first);
        assert_eq!(get(&reopened, "local", &f.template.id, 2).unwrap(), second);
        drop(reopened);
    }
    #[test]
    fn stale_revision_and_cross_workspace_cannot_mutate_or_read_other_templates() {
        let f = fixture();
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        save(&mut c, "local", &f.template, 0).unwrap();
        assert_eq!(
            save(&mut c, "local", &f.template, 0).unwrap_err(),
            "workflow_template_revision_conflict"
        );
        assert!(get(&c, "another-workspace", &f.template.id, 1).is_err());
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM workflow_templates", [], |r| r
                .get::<_, u64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn immutable_template_and_digest_guards_remain_enforced() {
        let f = fixture();
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        save(&mut c, "local", &f.template, 0).unwrap();
        assert!(c
            .execute("UPDATE workflow_templates SET name='changed'", [])
            .is_err());
        assert!(c.execute("DELETE FROM workflow_templates", []).is_err());
        c.execute_batch("DROP TRIGGER workflow_template_immutable")
            .unwrap();
        c.execute(
            "UPDATE workflow_templates SET content_sha256=?1",
            ["0".repeat(64)],
        )
        .unwrap();
        assert_eq!(
            get(&c, "local", &f.template.id, 1).unwrap_err(),
            "workflow_template_hash_mismatch"
        );
    }
}
