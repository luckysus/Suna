use super::*;

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

pub fn evidence_dataset(
    c: &Connection,
    ws: &str,
    p: &str,
    root: &std::path::Path,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
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
            let dataset = crate::storage::repositories::steel::save_new_preview_scoped(
                c,
                ws,
                &source,
                &sha,
                &preview,
                &[],
                p,
            )?;
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
