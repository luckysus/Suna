//! Append-only, human-cited conditions; no inferred comparability or training rows.
use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConditionKind {
    MaterialSystem,
    Composition,
    HeatTreatment,
    Microstructure,
    TestMethod,
    TestConditions,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CitedCondition {
    pub kind: ConditionKind,
    pub text: String,
    pub quote: String,
    pub chunk_id: String,
    pub chunk_sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveContext {
    pub evidence_artifact_id: String,
    pub evidence_sha256: String,
    pub conditions: Vec<CitedCondition>,
    pub confirmed: bool,
}
fn base(c: &Connection, ws: &str, project: &str, id: &str) -> Result<artifacts::Artifact, String> {
    Uuid::parse_str(id).map_err(|_| "invalid_literature_context_evidence")?;
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 AND artifact_id=?3)",params![ws,project,id],|r|r.get(0)).map_err(err)?;
    let a = artifacts::get(c, ws, id)?;
    if !exists || a.project_id != project || a.kind != "literature_evidence" || a.state != "ready" {
        return Err("invalid_literature_context_evidence".into());
    }
    let p = a
        .payload
        .as_ref()
        .ok_or("invalid_literature_context_evidence")?;
    quality::evidence(p)?;
    let (body,row_hash):(String,String)=c.query_row("SELECT payload_json,content_sha256 FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 AND artifact_id=?3",params![ws,project,id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    if row_hash != a.content_sha256
        || hash(&body) != row_hash
        || serde_json::from_str::<Value>(&body).ok().as_ref() != Some(p)
    {
        return Err("literature_context_evidence_changed".into());
    }
    Ok(a)
}
fn page(offset: u32) -> Result<(), String> {
    if offset > 10000 || offset % 25 != 0 {
        Err("literature_context_page_invalid".into())
    } else {
        Ok(())
    }
}
pub fn sources(c: &Connection, ws: &str, project: &str, offset: u32) -> Result<Value, String> {
    projects::get(c, ws, project)?;
    page(offset)?;
    let mut q=c.prepare("SELECT artifact_id FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at,id LIMIT 26 OFFSET ?3").map_err(err)?;
    let ids = q
        .query_map(params![ws, project, offset], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let more = ids.len() > 25;
    let mut items = vec![];
    for id in ids.into_iter().take(25) {
        let a = base(c, ws, project, &id)?;
        let p = a.payload.as_ref().unwrap();
        items.push(json!({"artifactId":a.id,"evidenceSha256":a.content_sha256,"field":p["field"],"value":p["value"],"unit":p["unit"],"documentId":p["documentId"],"versionId":p["versionId"],"quote":p["quote"]}));
    }
    Ok(json!({"items":items,"hasMore":more,"offset":offset,"readOnly":true}))
}
pub fn save(
    c: &mut Connection,
    ws: &str,
    project: &str,
    input: SaveContext,
) -> Result<Value, String> {
    if !input.confirmed || input.conditions.is_empty() || input.conditions.len() > 24 {
        return Err("literature_context_review_required".into());
    }
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    projects::require_active(&tx, ws, project)?;
    let original = base(&tx, ws, project, &input.evidence_artifact_id)?;
    if original.content_sha256 != input.evidence_sha256 {
        return Err("literature_context_evidence_changed".into());
    }
    let p = original.payload.as_ref().unwrap();
    if verify_evidence(&tx, ws, &quality::evidence(p)?)? != *p {
        return Err("literature_context_source_changed".into());
    }
    let mut checked = vec![];
    let mut seen = BTreeSet::new();
    for item in input.conditions {
        if item.text.trim().is_empty()
            || item.text.len() > 1000
            || item.quote.trim().is_empty()
            || item.quote.len() > 4000
            || !item.quote.contains(&item.text)
            || !crate::research_package::valid_sha(&item.chunk_sha256)
            || crate::rag::model::ChunkId::new(item.chunk_id.clone()).is_err()
            || !seen.insert(json!(&item).to_string())
        {
            return Err("invalid_literature_condition".into());
        }
        let current:Option<(String,String,String,String)>=tx.query_row("SELECT k.text,k.content_sha256,k.source_location_json,v.content_sha256 FROM knowledge_chunks k JOIN knowledge_document_versions v ON v.workspace_id=k.workspace_id AND v.id=k.version_id WHERE k.workspace_id=?1 AND k.id=?2 AND k.version_id=?3 AND v.document_id=?4",params![ws,item.chunk_id,p["versionId"].as_str(),p["documentId"].as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
        let (text, sha, location, document_sha) =
            current.ok_or("literature_context_source_missing")?;
        if hash(&text) != sha
            || sha != item.chunk_sha256
            || !text.contains(&item.quote)
            || p["documentSha256"] != document_sha
        {
            return Err("literature_context_source_changed".into());
        }
        checked.push(json!({"kind":item.kind,"text":item.text,"quote":item.quote,"chunkId":item.chunk_id,"chunkSha256":sha,"location":serde_json::from_str::<Value>(&location).map_err(|_|"invalid_literature_context_location")?}));
    }
    let payload = json!({"schemaVersion":"literature-experimental-context.v1","sourceType":"literature","reviewed":true,
        "evidenceArtifactId":original.id,"evidenceSha256":original.content_sha256,"documentId":p["documentId"],"versionId":p["versionId"],"documentSha256":p["documentSha256"],
        "conditions":checked,"scientificComparability":"not_assessed","rawSourceBytesVerified":false,"unitsAndProcessOrderInferred":false});
    let sha = hash(&payload.to_string());
    let key = format!("experimental-context:{}:{sha}", original.id);
    let existed:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='literature_evidence' AND source_key=?3)",params![ws,project,key],|r|r.get(0)).map_err(err)?;
    let id = artifacts::create(
        &tx,
        ws,
        project,
        "literature_evidence",
        "人工引用实验条件补充",
        &key,
        None,
        payload,
    )?;
    artifacts::edge(&tx, ws, &original.id, &id, "based_on_evidence")?;
    if !existed {
        projects::audit(
            &tx,
            ws,
            project,
            "literature_context_reviewed",
            json!({"artifactId":id,"evidenceArtifactId":original.id}),
        )?;
    }
    tx.commit().map_err(err)?;
    Ok(
        json!({"artifactId":id,"contentSha256":sha,"sourceType":"literature","scientificComparability":"not_assessed"}),
    )
}
pub fn list(
    c: &Connection,
    ws: &str,
    project: &str,
    evidence_id: &str,
    offset: u32,
) -> Result<Value, String> {
    projects::get(c, ws, project)?;
    page(offset)?;
    let original = base(c, ws, project, evidence_id)?;
    let mut q=c.prepare("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='literature_evidence' AND source_key LIKE ?3 ORDER BY created_at,id LIMIT 26 OFFSET ?4").map_err(err)?;
    let ids = q
        .query_map(
            params![
                ws,
                project,
                format!("experimental-context:{evidence_id}:%"),
                offset
            ],
            |r| r.get::<_, String>(0),
        )
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    let more = ids.len() > 25;
    let mut items = vec![];
    for id in ids.into_iter().take(25) {
        let a = artifacts::get(c, ws, &id)?;
        let p = a
            .payload
            .as_ref()
            .ok_or("literature_context_snapshot_invalid")?;
        if a.project_id != project
            || p["schemaVersion"] != "literature-experimental-context.v1"
            || p["evidenceArtifactId"] != evidence_id
            || p["evidenceSha256"] != original.content_sha256
            || p["scientificComparability"] != "not_assessed"
            || p["sourceType"] != "literature"
            || p["reviewed"] != true
        {
            return Err("literature_context_snapshot_invalid".into());
        }
        items.push(json!({"artifactId":a.id,"contentSha256":a.content_sha256,"createdAt":a.created_at,"snapshot":p}));
    }
    Ok(json!({"items":items,"hasMore":more,"offset":offset,"readOnly":true}))
}

#[cfg(test)]
#[path = "experimental_context_tests.rs"]
mod tests;
