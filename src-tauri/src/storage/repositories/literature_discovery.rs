//! Public metadata is distinct from verified original-text evidence.
use super::{research_artifacts as artifacts, research_projects as projects, settings};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
fn hash(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
pub const NETWORK_KEY: &str = "suna.literature.online_enabled";
use crate::literature_metadata::ProviderId;
pub use crate::literature_metadata::{normalize_doi, Metadata};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    pub revision_id: String,
    pub metadata: Metadata,
    pub retrieved_at: String,
    pub revision_sha256: String,
    pub already_existed: bool,
}
pub fn store(c: &Connection, ws: &str, mut m: Metadata) -> Result<Record, String> {
    if ProviderId::parse(&m.provider).is_none()
        || m.title.is_empty()
        || m.title.chars().count() > 2000
        || m.authors.len() > 100
        || m.authors.iter().any(|s| s.len() > 500)
        || m.external_id.len() > 500
        || m.abstract_text.as_ref().is_some_and(|s| s.len() > 30000)
        || m.year.is_some_and(|y| !(1000..=2200).contains(&y))
        || m.fulltext_state != "link_only"
    {
        return Err("invalid_literature_metadata".into());
    }
    m.doi = m.doi.map(|s| normalize_doi(&s)).transpose()?;
    let key = if let Some(d) = &m.doi {
        format!("doi:{d}")
    } else {
        format!("{}:{}", m.provider, m.external_id)
    };
    let body = json!(m).to_string();
    let sha = hash(&body);
    artifacts::atomic(c, || {
        let existing: Option<String> = c
            .query_row(
                "SELECT id FROM literature_identities WHERE workspace_id=?1 AND identity_key=?2",
                params![ws, key],
                |r| r.get(0),
            )
            .optional()
            .map_err(err)?;
        let existed = existing.is_some();
        let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now().to_rfc3339();
        c.execute(
            "INSERT OR IGNORE INTO literature_identities VALUES(?1,?2,?3,?4,NULL,?5)",
            params![ws, id, key, m.doi, now],
        )
        .map_err(err)?;
        let old:Option<(String,String)>=c.query_row("SELECT id,retrieved_at FROM literature_metadata_revisions WHERE workspace_id=?1 AND literature_id=?2 AND content_sha256=?3",params![ws,id,sha],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
        let (revision, time) = if let Some(old) = old {
            old
        } else {
            let rid = Uuid::new_v4().to_string();
            c.execute(
                "INSERT INTO literature_metadata_revisions VALUES(?1,?2,?3,?4,?5,?6)",
                params![ws, rid, id, body, sha, now],
            )
            .map_err(err)?;
            (rid, now)
        };
        c.execute("UPDATE literature_identities SET current_revision_id=?3 WHERE workspace_id=?1 AND id=?2",params![ws,id,revision]).map_err(err)?;
        Ok(Record {
            id,
            revision_id: revision,
            metadata: m,
            retrieved_at: time,
            revision_sha256: sha,
            already_existed: existed,
        })
    })
}
pub fn get(c: &Connection, ws: &str, id: &str) -> Result<Record, String> {
    let(body,revision,time,sha):(String,String,String,String)=c.query_row("SELECT r.metadata_json,r.id,r.retrieved_at,r.content_sha256 FROM literature_identities i JOIN literature_metadata_revisions r ON r.workspace_id=i.workspace_id AND r.id=i.current_revision_id WHERE i.workspace_id=?1 AND i.id=?2",params![ws,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?.ok_or("literature_not_found")?;
    if hash(&body) != sha {
        return Err("literature_revision_hash_mismatch".into());
    }
    Ok(Record {
        id: id.into(),
        revision_id: revision,
        metadata: serde_json::from_str(&body).map_err(|e| e.to_string())?,
        retrieved_at: time,
        revision_sha256: sha,
        already_existed: true,
    })
}
pub fn list(c: &Connection, ws: &str, query: &str, offset: u32) -> Result<Vec<Record>, String> {
    let mut q=c.prepare("SELECT i.id FROM literature_identities i JOIN literature_metadata_revisions r ON r.workspace_id=i.workspace_id AND r.id=i.current_revision_id WHERE i.workspace_id=?1 AND r.metadata_json LIKE '%'||?2||'%' ORDER BY i.created_at DESC,i.id LIMIT 20 OFFSET ?3").map_err(err)?;
    let ids = q
        .query_map(params![ws, query, offset], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    ids.iter().map(|id| get(c, ws, id)).collect()
}
pub fn online(c: &Connection, ws: &str) -> Result<bool, String> {
    Ok(settings::get(c, ws, NETWORK_KEY)?.is_some_and(|v| v == "true" || v == "\"true\""))
}
pub fn reserve_request(c: &Connection, ws: &str, now: i64) -> Result<(), String> {
    reserve_for_provider(c, ws, ProviderId::Crossref, now)
}
pub fn reserve_for_provider(
    c: &Connection,
    ws: &str,
    provider: ProviderId,
    now: i64,
) -> Result<(), String> {
    if !online(c, ws)? {
        return Err("literature_network_disabled".into());
    }
    let changed=c.execute("INSERT INTO literature_provider_rate VALUES(?1,?2,?3) ON CONFLICT(workspace_id,provider) DO UPDATE SET last_request_ms=excluded.last_request_ms WHERE literature_provider_rate.last_request_ms<=excluded.last_request_ms-1000",params![ws,provider.as_str(),now]).map_err(err)?;
    if changed == 0 {
        return Err("literature_rate_limited".into());
    }
    Ok(())
}
pub fn attach_metadata(c: &Connection, ws: &str, p: &str, id: &str) -> Result<(), String> {
    projects::require_active(c, ws, p)?;
    let r = get(c, ws, id)?;
    artifacts::atomic(c, || {
        artifacts::create(
            c,
            ws,
            p,
            "literature_evidence",
            &r.metadata.title.chars().take(280).collect::<String>(),
            &format!("metadata:{}:{}", id, r.revision_id),
            None,
            json!({"literatureId":id,"revisionId":r.revision_id,"metadataSha256":r.revision_sha256,"metadata":r.metadata,"reviewed":false,"evidenceStatus":"metadata_only_no_original_text"}),
        )?;
        projects::audit(
            c,
            ws,
            p,
            "literature_metadata_attached",
            json!({"literatureId":id,"revisionId":r.revision_id}),
        )?;
        Ok(())
    })
}
pub fn link_fulltext(
    c: &mut Connection,
    ws: &str,
    p: &str,
    id: &str,
    document: &str,
    version: &str,
    rights: &str,
) -> Result<(), String> {
    get(c, ws, id)?;
    projects::require_active(c, ws, p)?;
    if !matches!(rights, "user_owned" | "open_access_verified") {
        return Err("fulltext_rights_required".into());
    }
    let owned:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM knowledge_document_versions v JOIN knowledge_source_documents d ON d.workspace_id=v.workspace_id AND d.id=v.document_id WHERE v.workspace_id=?1 AND d.id=?2 AND v.id=?3)",params![ws,document,version],|r|r.get(0)).map_err(err)?;
    if !owned {
        return Err("fulltext_not_found_in_workspace".into());
    }
    let tx = c.transaction().map_err(err)?;
    tx.execute(
        "INSERT OR IGNORE INTO literature_fulltext_links VALUES(?1,?2,?3,?4,?5,?6)",
        params![ws, id, document, version, rights, Utc::now().to_rfc3339()],
    )
    .map_err(err)?;
    projects::attach_in_transaction(&tx, ws, p, "document", document, "attached")?;
    projects::audit(
        &tx,
        ws,
        p,
        "literature_fulltext_linked",
        json!({"literatureId":id,"documentId":document,"versionId":version,"rightsBasis":rights}),
    )?;
    tx.commit().map_err(err)
}
#[cfg(test)]
#[path = "literature_discovery/evidence_tests.rs"]
mod evidence_tests;
#[cfg(test)]
#[path = "literature_discovery/tests.rs"]
mod tests;

mod evidence;
pub use evidence::{evidence_dataset, evidence_rows, save_evidence, verify_evidence, Evidence};
pub mod quality;
#[cfg(test)]
mod quality_tests;
