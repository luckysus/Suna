//! Freeze the specific persisted revision, never the mutable current revision.
use super::*;
use crate::literature_metadata::{Metadata, ProviderId};
use std::collections::BTreeSet;

fn provider(value: &Value) -> Result<ProviderId, String> {
    match value.get("provider") {
        None => Ok(ProviderId::Crossref), // Pre-adapter tasks used Crossref only.
        Some(v) => v
            .as_str()
            .and_then(ProviderId::parse)
            .ok_or_else(|| "literature_source_mismatch".into()),
    }
}

pub(super) fn payload(
    c: &Connection,
    ws: &str,
    request: &Value,
    result: &Value,
) -> Result<Value, String> {
    let source = provider(request)?;
    if provider(result)? != source {
        return Err("literature_source_mismatch".into());
    }
    let records = result["records"]
        .as_array()
        .ok_or("missing_literature_result")?;
    if records.len() > 20 || request["query"].as_str().is_none() {
        return Err("invalid_literature_snapshot".into());
    }
    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    for record in records {
        let id = record["id"].as_str().ok_or("literature_source_mismatch")?;
        let revision = record["revisionId"]
            .as_str()
            .ok_or("literature_source_mismatch")?;
        if Uuid::parse_str(id).is_err()
            || Uuid::parse_str(revision).is_err()
            || !seen.insert((id, revision))
        {
            return Err("literature_source_mismatch".into());
        }
        let (body, sha): (String, String) = c.query_row(
            "SELECT metadata_json,content_sha256 FROM literature_metadata_revisions WHERE workspace_id=?1 AND literature_id=?2 AND id=?3",
            params![ws, id, revision], |r| Ok((r.get(0)?, r.get(1)?)),
        ).optional().map_err(err)?.ok_or("literature_source_mismatch")?;
        let actual: Metadata =
            serde_json::from_str(&body).map_err(|_| "literature_source_mismatch")?;
        if format!("{:x}", Sha256::digest(body.as_bytes())) != sha
            || record["revisionSha256"] != sha
            || actual.provider != source.as_str()
            || record["metadata"] != json!(actual)
        {
            return Err("literature_source_mismatch".into());
        }
        sources.push(json!({"literatureId":id,"revisionId":revision,"metadataSha256":sha}));
    }
    Ok(
        json!({"provider":source.as_str(),"querySha256":format!("{:x}",Sha256::digest(request["query"].to_string().as_bytes())),
        "fromYear":request["fromYear"],"toYear":request["toYear"],"sampleScope":"user_search_results","records":sources}),
    )
}
