//! Local index traceability, not a scientific-validity or probability score.
use super::*;
use crate::steel::materials::{self, Registry};
use std::collections::BTreeMap;

fn evidence(value: &Value) -> Result<Evidence, String> {
    if value["reviewed"] != true || value["sourceType"] != "literature" {
        return Err("invalid_literature_evidence_snapshot".into());
    }
    let e:Evidence=serde_json::from_value(json!({"documentId":value["documentId"],"versionId":value["versionId"],"chunkId":value["chunkId"],
        "quote":value["quote"],"field":value["field"],"value":value["value"],"unit":value["unit"],"conclusion":value["conclusion"],"doi":value["doi"]}))
        .map_err(|_|"invalid_literature_evidence_snapshot")?;
    if !e.value.is_finite()
        || [&e.document_id, &e.version_id]
            .iter()
            .any(|id| Uuid::parse_str(id).is_err())
        || crate::rag::model::ChunkId::new(e.chunk_id.clone()).is_err()
    {
        return Err("invalid_literature_evidence_snapshot".into());
    }
    Ok(e)
}
fn check(
    c: &Connection,
    ws: &str,
    project: &str,
    p: &Value,
    registry: &Registry,
) -> Result<Value, String> {
    let e = evidence(p)?;
    let sha = hash(&p.to_string());
    let artifact_id:String=c.query_row("SELECT artifact_id FROM literature_reviewed_evidence WHERE workspace_id=?1 AND project_id=?2 AND content_sha256=?3",params![ws,project,sha],|r|r.get(0))
        .map_err(|_|"literature_quality_evidence_mismatch")?;
    let artifact = artifacts::get(c, ws, &artifact_id)?;
    if artifact.project_id != project
        || artifact.kind != "literature_evidence"
        || artifact.content_sha256 != sha
        || artifact.payload.as_ref() != Some(p)
    {
        return Err("literature_quality_evidence_mismatch".into());
    }
    let current:Option<(String,String,String,String)>=c.query_row(
        "SELECT k.text,k.source_location_json,k.content_sha256,v.content_sha256 FROM knowledge_chunks k JOIN knowledge_document_versions v ON v.workspace_id=k.workspace_id AND v.id=k.version_id WHERE k.workspace_id=?1 AND k.id=?2 AND k.version_id=?3 AND v.document_id=?4",
        params![ws,e.chunk_id,e.version_id,e.document_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
    let (indexed, observed) = match current {
        None => ("missing", None),
        Some((text, location, chunk_sha, document_sha)) => {
            let observed=hash(&json!({"textSha256":hash(&text),"declaredChunkSha256":chunk_sha,"documentSha256":document_sha,"locationSha256":hash(&location)}).to_string());
            let state = if hash(&text) == chunk_sha
                && p["chunkSha256"] == chunk_sha
                && p["documentSha256"] == document_sha
                && text.contains(&e.quote)
                && serde_json::from_str::<Value>(&location).ok().as_ref() == Some(&p["location"])
            {
                "matched"
            } else {
                "changed"
            };
            (state, Some(observed))
        }
    };
    let doi = if let Some(doi) = &e.doi {
        let linked:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM literature_fulltext_links l JOIN literature_identities i ON i.workspace_id=l.workspace_id AND i.id=l.literature_id WHERE l.workspace_id=?1 AND l.document_id=?2 AND l.version_id=?3 AND i.doi=?4)",params![ws,e.document_id,e.version_id,doi],|r|r.get(0)).map_err(err)?;
        if linked {
            "linked"
        } else {
            "link_missing"
        }
    } else {
        "not_recorded"
    };
    let field = registry.fields.iter().find(|f| f.id == e.field);
    let normalized =
        field.and_then(|f| materials::convert(e.value, &e.unit, &f.canonical_unit).ok());
    let unit_state = match (field, normalized) {
        (None, _) => "unknown_field",
        (Some(_), None) => "incompatible_or_overflow",
        _ => "normalized",
    };
    Ok(
        json!({"evidenceSha256":sha,"artifactId":artifact_id,"documentId":e.document_id,"versionId":e.version_id,"chunkId":e.chunk_id,
        "field":e.field,"value":e.value,"unit":e.unit,"indexedSourceState":indexed,"observedIndexSha256":observed,"doiState":doi,
        "locationRecorded":p["location"].as_object().is_some_and(|v|!v.is_empty()),"unitState":unit_state,
        "canonicalField":field.map(|f|&f.id),"canonicalUnit":field.map(|f|&f.canonical_unit),"normalizedValue":normalized,
        "rawSourceBytesVerified":false,"comparability":"conditions_not_recorded"}),
    )
}
pub fn preview(c: &Connection, ws: &str, project: &str) -> Result<Value, String> {
    artifacts::atomic(c, || {
        let rows = evidence_rows(c, ws, project)?;
        let registry = super::super::material_rules::registry(c, ws, Some(project))?;
        let checks = rows
            .iter()
            .map(|p| check(c, ws, project, p, &registry))
            .collect::<Result<Vec<_>, _>>()?;
        let mut counts: BTreeMap<&str, usize> = [
            "indexedReferenceMatched",
            "missingIndexedSource",
            "changedIndexedSource",
            "doiNotRecorded",
            "doiLinkMissing",
            "locationNotRecorded",
            "unknownField",
            "incompatibleUnit",
        ]
        .into_iter()
        .map(|key| (key, 0))
        .collect();
        for row in &checks {
            let indexed = match row["indexedSourceState"].as_str() {
                Some("matched") => "indexedReferenceMatched",
                Some("missing") => "missingIndexedSource",
                _ => "changedIndexedSource",
            };
            *counts.get_mut(indexed).unwrap() += 1;
            for (condition, key) in [
                (row["doiState"] == "not_recorded", "doiNotRecorded"),
                (row["doiState"] == "link_missing", "doiLinkMissing"),
                (row["locationRecorded"] == false, "locationNotRecorded"),
                (row["unitState"] == "unknown_field", "unknownField"),
                (
                    row["unitState"] == "incompatible_or_overflow",
                    "incompatibleUnit",
                ),
            ] {
                if condition {
                    *counts.get_mut(key).unwrap() += 1;
                }
            }
        }
        let report = json!({"schemaVersion":"literature-evidence-quality.v1","algorithmVersion":"index-traceability.v1","projectId":project,
            "scope":"reviewed_numeric_evidence","sourceType":"literature","registrySha256":hash(&json!(registry).to_string()),
            "reviewedEvidenceCount":checks.len(),"counts":counts,"checks":checks,
            "limitations":["indexed_references_only","raw_source_bytes_not_verified","location_metadata_is_not_page_verification","numeric_assignment_requires_human_review","test_method_not_recorded","experiment_conditions_not_recorded","no_retraction_check","not_a_scientific_validity_score"]});
        Ok(json!({"readOnly":true,"reportSha256":hash(&report.to_string()),"report":report}))
    })
}
pub fn archive(
    c: &mut Connection,
    ws: &str,
    project: &str,
    expected_sha256: &str,
) -> Result<Value, String> {
    let tx = c
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(err)?;
    projects::require_active(&tx, ws, project)?;
    let current = preview(&tx, ws, project)?;
    if current["reportSha256"] != expected_sha256 {
        return Err("literature_quality_preview_changed".into());
    }
    if current["report"]["reviewedEvidenceCount"] == 0 {
        return Err("no_reviewed_literature_evidence".into());
    }
    let id = artifacts::create(
        &tx,
        ws,
        project,
        "analysis_result",
        "文献证据溯源核查报告",
        &format!("literature-quality:{expected_sha256}"),
        None,
        current["report"].clone(),
    )?;
    let verified = artifacts::get(&tx, ws, &id)?;
    if verified.content_sha256 != expected_sha256 {
        return Err("literature_quality_evidence_mismatch".into());
    }
    tx.commit().map_err(err)?;
    Ok(
        json!({"artifactId":id,"contentSha256":verified.content_sha256,"reportSha256":expected_sha256}),
    )
}
