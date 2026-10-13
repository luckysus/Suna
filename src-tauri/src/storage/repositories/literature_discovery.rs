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
pub fn normalize_doi(s: &str) -> Result<String, String> {
    let mut s = s.trim().to_lowercase();
    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ] {
        if let Some(v) = s.strip_prefix(prefix) {
            s = v.trim().into();
            break;
        }
    }
    let (prefix, suffix) = s.split_once('/').ok_or("invalid_doi")?;
    if !prefix.starts_with("10.")
        || !prefix[3..].chars().all(|c| c.is_ascii_digit())
        || prefix.len() < 7
        || suffix.is_empty()
        || s.len() > 300
        || s.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("invalid_doi".into());
    }
    Ok(s)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    pub provider: String,
    pub external_id: String,
    pub doi: Option<String>,
    pub title: String,
    pub authors: Vec<String>,
    pub journal: Option<String>,
    pub year: Option<i64>,
    pub abstract_text: Option<String>,
    pub source_url: String,
    pub license: Option<String>,
    pub fulltext_state: String,
}
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
    if m.provider != "crossref"
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
    if !online(c, ws)? {
        return Err("literature_network_disabled".into());
    }
    let changed=c.execute("INSERT INTO literature_provider_rate VALUES(?1,'crossref',?2) ON CONFLICT(workspace_id,provider) DO UPDATE SET last_request_ms=excluded.last_request_ms WHERE literature_provider_rate.last_request_ms<=excluded.last_request_ms-1000",params![ws,now]).map_err(err)?;
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
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub document_id: String,
    pub version_id: String,
    pub chunk_id: String,
    pub quote: String,
    pub field: String,
    pub value: f64,
    pub unit: String,
    pub conclusion: String,
    pub doi: Option<String>,
}
pub fn verify_evidence(c: &Connection, ws: &str, e: &Evidence) -> Result<Value, String> {
    if !e.value.is_finite()
        || e.quote.trim().is_empty()
        || e.quote.len() > 30000
        || e.field.trim().is_empty()
        || e.field.len() > 160
        || e.conclusion.len() > 10000
        || e.unit.trim().is_empty()
        || e.unit.len() > 64
    {
        return Err("invalid_literature_evidence".into());
    }
    let(text,location,chunk_sha,source_sha):(String,String,String,String)=c.query_row("SELECT k.text,k.source_location_json,k.content_sha256,v.content_sha256 FROM knowledge_chunks k JOIN knowledge_document_versions v ON v.workspace_id=k.workspace_id AND v.id=k.version_id WHERE k.workspace_id=?1 AND k.id=?2 AND k.version_id=?3 AND v.document_id=?4",params![ws,e.chunk_id,e.version_id,e.document_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?.ok_or("evidence_source_not_found_in_workspace")?;
    if hash(&text) != chunk_sha || !text.contains(&e.quote) {
        return Err("evidence_quote_or_hash_mismatch".into());
    }
    let mut numeric_found = false;
    let mut pair_found = false;
    let mut start = None;
    for (end, ch) in e
        .quote
        .char_indices()
        .chain(std::iter::once((e.quote.len(), ' ')))
    {
        if ch.is_ascii_digit() || matches!(ch, '.' | '+' | '-' | 'e' | 'E') {
            if start.is_none() {
                start = Some(end);
            }
        } else if let Some(begin) = start.take() {
            if e.quote[begin..end].parse::<f64>().ok() == Some(e.value) {
                numeric_found = true;
                let tail = e.quote[end..].trim_start_matches(|c: char| {
                    c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']')
                });
                let prefix = e.quote[..begin].trim_end();
                let unit = e.unit.trim().to_lowercase();
                if tail.to_lowercase().starts_with(&unit) || prefix.to_lowercase().ends_with(&unit)
                {
                    pair_found = true;
                }
            }
        }
    }
    if !numeric_found {
        return Err("evidence_numeric_value_not_in_quote".into());
    }
    if !pair_found {
        return Err("evidence_unit_not_paired_with_value".into());
    }
    if let Some(field) = crate::steel::materials::base()
        .fields
        .iter()
        .find(|f| f.id == e.field)
    {
        crate::steel::materials::conversion(&e.unit, &field.canonical_unit)?;
    }
    let doi = e.doi.as_ref().map(|d| normalize_doi(d)).transpose()?;
    if let Some(doi) = &doi {
        let matched:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM literature_fulltext_links l JOIN literature_identities i ON i.workspace_id=l.workspace_id AND i.id=l.literature_id WHERE l.workspace_id=?1 AND l.document_id=?2 AND l.version_id=?3 AND i.doi=?4)",params![ws,e.document_id,e.version_id,doi],|r|r.get(0)).map_err(err)?;
        if !matched {
            return Err("evidence_doi_not_linked_to_original".into());
        }
    }
    Ok(
        json!({"documentId":e.document_id,"versionId":e.version_id,"chunkId":e.chunk_id,"chunkSha256":chunk_sha,"documentSha256":source_sha,"location":serde_json::from_str::<Value>(&location).map_err(|e|e.to_string())?,"quote":e.quote,"field":e.field,"value":e.value,"unit":e.unit,"conclusion":e.conclusion,"doi":doi,"reviewed":true,"sourceType":"literature"}),
    )
}
pub fn save_evidence(c: &Connection, ws: &str, p: &str, e: Evidence) -> Result<String, String> {
    projects::require_active(c, ws, p)?;
    let payload = verify_evidence(c, ws, &e)?;
    let body = payload.to_string();
    let sha = hash(&body);
    artifacts::atomic(c, || {
        let old:Option<String>=c.query_row("SELECT id FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 AND content_sha256=?3",params![ws,p,sha],|r|r.get(0)).optional().map_err(err)?;
        if let Some(id) = old {
            return Ok(id);
        }
        let id = Uuid::new_v4().to_string();
        let a = artifacts::create(
            c,
            ws,
            p,
            "literature_evidence",
            "人工审核文献数值证据",
            &format!("evidence:{sha}"),
            None,
            payload,
        )?;
        c.execute(
            "INSERT INTO literature_reviewed_evidence VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![ws, id, p, body, sha, a, Utc::now().to_rfc3339()],
        )
        .map_err(err)?;
        projects::audit(
            c,
            ws,
            p,
            "literature_evidence_accepted",
            json!({"id":id,"chunkId":e.chunk_id}),
        )?;
        Ok(id)
    })
}
pub fn evidence_rows(c: &Connection, ws: &str, p: &str) -> Result<Vec<Value>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT payload_json,content_sha256 FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at,id LIMIT 1001").map_err(err)?;
    let rows = q
        .query_map(params![ws, p], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    if rows.len() > 1000 {
        return Err("literature_evidence_export_limit".into());
    }
    rows.into_iter()
        .map(|(body, sha)| {
            if hash(&body) != sha {
                return Err("literature_evidence_hash_mismatch".into());
            }
            serde_json::from_str(&body).map_err(|e| e.to_string())
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        c
    }
    fn metadata() -> Metadata {
        Metadata {
            provider: "crossref".into(),
            external_id: "10.1234/test".into(),
            doi: Some(" HTTPS://DOI.ORG/10.1234/TEST ".into()),
            title: "Synthetic metadata".into(),
            authors: vec!["Fixture Author".into()],
            journal: None,
            year: None,
            abstract_text: None,
            source_url: "https://doi.org/10.1234/test".into(),
            license: None,
            fulltext_state: "link_only".into(),
        }
    }
    #[test]
    fn doi_identity_deduplicates_but_revisions_remain_immutable() {
        let c = fixture();
        let first = store(&c, "ws", metadata()).unwrap();
        let same = store(&c, "ws", metadata()).unwrap();
        assert_eq!(first.id, same.id);
        assert_eq!(first.revision_id, same.revision_id);
        assert!(same.already_existed);
        let mut updated = metadata();
        updated.year = Some(2022);
        let revision = store(&c, "ws", updated).unwrap();
        assert_eq!(first.id, revision.id);
        assert_ne!(first.revision_id, revision.revision_id);
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM literature_metadata_revisions",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert!(get(&c, "other", &first.id).is_err());
        assert!(c
            .execute("DELETE FROM literature_metadata_revisions", [])
            .is_err());
        assert!(normalize_doi("not a doi").is_err());
        assert_eq!(
            normalize_doi(" doi:10.1234/AB(c) ").unwrap(),
            "10.1234/ab(c)"
        );
    }
    #[test]
    fn provider_rate_limit_is_workspace_scoped_and_defaults_off() {
        let mut c = fixture();
        assert!(reserve_request(&c, "ws", 1000).is_err());
        settings::set(&mut c, "ws", NETWORK_KEY, "true").unwrap();
        reserve_request(&c, "ws", 1000).unwrap();
        assert_eq!(
            reserve_request(&c, "ws", 1100).unwrap_err(),
            "literature_rate_limited"
        );
        reserve_request(&c, "ws", 2000).unwrap();
        assert!(reserve_request(&c, "other", 2000).is_err());
    }
    #[test]
    fn fulltext_requires_owned_source_and_rights_and_does_not_download_non_oa() {
        let mut c = fixture();
        let r = store(&c, "ws", metadata()).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            projects::SaveResearchProject {
                id: None,
                name: "合成课题".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        assert!(link_fulltext(
            &mut c,
            "ws",
            &p.id,
            &r.id,
            "missing",
            "missing",
            "user_owned"
        )
        .is_err());
        assert!(link_fulltext(&mut c, "ws", &p.id, &r.id, "missing", "missing", "").is_err());
        attach_metadata(&c, "ws", &p.id, &r.id).unwrap();
        let a = artifacts::list(&c, "ws", &p.id, 25, 0).unwrap();
        assert_eq!(
            artifacts::get(&c, "ws", &a[0].id).unwrap().payload.unwrap()["evidenceStatus"],
            "metadata_only_no_original_text"
        );
    }
}

pub fn evidence_dataset(
    c: &Connection,
    ws: &str,
    p: &str,
    root: &std::path::Path,
) -> Result<super::steel::SteelDatasetRecord, String> {
    use std::io::Write;
    projects::require_active(c, ws, p)?;
    let rows = evidence_rows(c, ws, p)?;
    if rows.is_empty() {
        return Err("no_reviewed_literature_evidence".into());
    }
    let bytes = serde_json::to_vec(&rows).map_err(|e| e.to_string())?;
    let folder = root.join("objects").join("literature-data");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    if !folder
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("evidence_storage_outside_data_root".into());
    }
    let path = folder.join(format!("{}.json", Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })?;
        let sha = format!("{:x}", Sha256::digest(&bytes));
        artifacts::atomic(c, || {
            let dataset =
                super::steel::save_new_preview_scoped(c, ws, &source, &sha, &preview, &[], p)?;
            let snapshot = artifacts::dataset_snapshot(c, ws, p, &dataset.id)?
                .ok_or("missing_dataset_snapshot")?;
            let mut q=c.prepare("SELECT artifact_id FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2").map_err(err)?;
            let source_ids = q
                .query_map(params![ws, p], |r| r.get::<_, String>(0))
                .map_err(err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            for a in source_ids {
                artifacts::edge(c, ws, &a, &snapshot, "based_on_evidence")?;
            }
            projects::audit(
                c,
                ws,
                p,
                "literature_evidence_dataset_created",
                json!({"datasetId":dataset.id,"sourceType":"literature","sha256":sha}),
            )?;
            Ok(dataset)
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

#[cfg(test)]
mod evidence_tests {
    use super::*;
    use crate::rag::{
        chunk::{chunk_document, ChunkPolicy},
        ingest::SourceFormat,
        model::{NewChunk, NewDocumentVersion, NewSourceDocument},
        parse::{parse_document_bytes, ParseLimits},
    };
    use crate::storage::repositories::knowledge;
    #[test]
    fn real_local_parser_evidence_review_dataset_and_missing_source_gates() {
        let root = std::env::temp_dir().join(format!("suna-文献合成-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            projects::SaveResearchProject {
                id: None,
                name: "合成文献验证".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        let metadata = store(
            &c,
            "ws",
            Metadata {
                provider: "crossref".into(),
                external_id: "10.1234/synthetic".into(),
                doi: Some("10.1234/synthetic".into()),
                title: "Synthetic evidence fixture".into(),
                authors: vec![],
                journal: None,
                year: None,
                abstract_text: None,
                source_url: "https://doi.org/10.1234/synthetic".into(),
                license: None,
                fulltext_state: "link_only".into(),
            },
        )
        .unwrap();
        let bytes=b"# Synthetic measurements\n\nSynthetic fixture hardness 500 HV and strength 1500 MPa. Not real research evidence.\n";
        let parsed =
            parse_document_bytes(bytes, SourceFormat::Markdown, ParseLimits::default()).unwrap();
        let chunks = chunk_document(&parsed, &ChunkPolicy::default()).unwrap();
        let base = knowledge::create_knowledge_base(&c, "ws", "合成原文").unwrap();
        let doc = knowledge::create_source_document(
            &c,
            "ws",
            NewSourceDocument {
                knowledge_base_id: base.id,
                display_name: "合成文献.md".into(),
                source_kind: "markdown".into(),
            },
        )
        .unwrap();
        let version = knowledge::create_document_version(
            &c,
            "ws",
            NewDocumentVersion {
                document_id: doc.id,
                content_sha256: format!("{:x}", Sha256::digest(bytes)),
                mime_type: "text/markdown".into(),
                parser: "builtin".into(),
                parser_version: "fixture".into(),
                chunk_policy_version: "fixture".into(),
                embedding_profile_id: Uuid::new_v4().to_string(),
                embedding_model_id: "unused-lexical-only-fixture".into(),
                embedding_dimension: 1,
                expected_asset_count: 0,
                expected_chunk_count: chunks.len() as u32,
            },
        )
        .unwrap();
        for chunk in &chunks {
            knowledge::add_chunk(
                &c,
                "ws",
                NewChunk {
                    id: chunk.id.clone(),
                    version_id: version.id,
                    ordinal: chunk.ordinal,
                    text: chunk.text.clone(),
                    source_location: chunk.source_location.clone(),
                    content_sha256: chunk.content_sha256.clone(),
                    policy_version: "fixture".into(),
                },
            )
            .unwrap();
        }
        link_fulltext(
            &mut c,
            "ws",
            &p.id,
            &metadata.id,
            &doc.id.to_string(),
            &version.id.to_string(),
            "user_owned",
        )
        .unwrap();
        let chunk = chunks.iter().find(|x| x.text.contains("500 HV")).unwrap();
        let evidence = Evidence {
            document_id: doc.id.to_string(),
            version_id: version.id.to_string(),
            chunk_id: chunk.id.to_string(),
            quote: "Synthetic fixture hardness 500 HV and strength 1500 MPa.".into(),
            field: "property.hardness_hv".into(),
            value: 500.,
            unit: "HV".into(),
            conclusion: "合成夹具数据，非科研事实".into(),
            doi: Some("10.1234/synthetic".into()),
        };
        assert!(verify_evidence(&c, "other", &evidence).is_err());
        let mut bad = evidence.clone();
        bad.value = 500.;
        bad.unit = "MPa".into();
        bad.quote = "strength 1500 MPa".into();
        assert!(verify_evidence(&c, "ws", &bad).is_err());
        bad = evidence.clone();
        bad.unit = "J".into();
        assert!(verify_evidence(&c, "ws", &bad).is_err());
        let id = save_evidence(&c, "ws", &p.id, evidence.clone()).unwrap();
        assert_eq!(id, save_evidence(&c, "ws", &p.id, evidence).unwrap());
        let dataset = evidence_dataset(&c, "ws", &p.id, &root).unwrap();
        assert_eq!(dataset.row_count, 1);
        assert_eq!(dataset.mapping_state, "draft");
        let rows = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
            source_path: dataset.source_path,
            sheet: None,
        })
        .unwrap();
        assert!(rows.headers.contains(&"sourceType".into()));
        assert!(evidence_dataset(&c, "ws", &p.id, &root).is_err());
        assert_eq!(evidence_rows(&c, "ws", &p.id).unwrap().len(), 1);
        drop(c);
        let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        assert_eq!(evidence_rows(&c, "ws", &p.id).unwrap().len(), 1);
        drop(c);
        std::fs::remove_dir_all(root).unwrap();
    }
}
