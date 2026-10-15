use super::*;

#[test]
fn local_evidence_search_reads_a_real_ready_fts_index_and_returns_bound_hashes() {
    use crate::rag::model::{EmbeddingIdentity, EmbeddingVectorBatch};
    use crate::storage::repositories::knowledge;
    let (mut f, mut input) = setup();
    let version = f.evidence.version_id.parse().unwrap();
    let document = f.evidence.document_id.parse().unwrap();
    let (profile,model,policy):(String,String,String)=f.connection.query_row("SELECT embedding_profile_id,embedding_model_id,chunk_policy_version FROM knowledge_document_versions WHERE id=?1",[&f.evidence.version_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    // Synthetic vectors satisfy the real activation contract; this test uses
    // lexical FTS only and makes no semantic embedding/model service claim.
    for chunk in knowledge::list_chunks_for_embedding(&f.connection, "ws", version).unwrap() {
        let vector = 1f32.to_le_bytes().to_vec();
        knowledge::persist_embedding_batch(
            &mut f.connection,
            "ws",
            version,
            &[EmbeddingVectorBatch {
                vector_key: Uuid::new_v4().to_string(),
                identity: EmbeddingIdentity {
                    provider_profile_id: profile.clone(),
                    model_id: model.clone(),
                    dimension: 1,
                    normalized_text_sha256: hash(&chunk.text),
                    policy_version: policy.clone(),
                },
                vector_sha256: format!("{:x}", Sha256::digest(&vector)),
                vector_blob: vector,
                chunk_ids: vec![chunk.id],
            }],
        )
        .unwrap();
    }
    knowledge::finalize_flat_index(&mut f.connection, "ws", version).unwrap();
    knowledge::activate_document_version(&mut f.connection, "ws", document, version).unwrap();
    let hits = search_evidence(&f.connection, "ws", "hardness").unwrap();
    assert_eq!(
        hits.as_array().unwrap().len(),
        1,
        "the native caller must supply actual owned bases, not a deny-all empty list"
    );
    let hit = &hits[0];
    assert_eq!(hit["chunkId"], f.evidence.chunk_id);
    assert_eq!(hash(hit["quote"].as_str().unwrap()), hit["chunkSha256"]);
    input.conditions[0].quote = hit["quote"].as_str().unwrap().into();
    input.conditions[0].chunk_id = hit["chunkId"].as_str().unwrap().into();
    input.conditions[0].chunk_sha256 = hit["chunkSha256"].as_str().unwrap().into();
    let saved = save(&mut f.connection, "ws", &f.project_id, input.clone()).unwrap();
    assert_eq!(
        list(
            &f.connection,
            "ws",
            &f.project_id,
            &input.evidence_artifact_id,
            0
        )
        .unwrap()["items"][0]["snapshot"]["conditions"][0]["chunkSha256"],
        hit["chunkSha256"]
    );
    assert!(!saved["artifactId"].as_str().unwrap().is_empty());
    assert_eq!(
        search_evidence(&f.connection, "other", "hardness").unwrap(),
        json!([])
    );
    f.connection
        .execute(
            "UPDATE knowledge_chunks SET text=text||' changed' WHERE id=?1",
            [&f.evidence.chunk_id],
        )
        .unwrap();
    assert!(search_evidence(&f.connection, "ws", "hardness").is_err());
    assert_eq!(
        list(
            &f.connection,
            "ws",
            &f.project_id,
            &input.evidence_artifact_id,
            0
        )
        .unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    close(f);
}

#[test]
fn owned_base_budget_and_two_independent_history_pages_never_truncate_or_change_user_configuration()
{
    let (mut f, input) = setup();
    for n in 1..=26 {
        let mut next = input.clone();
        next.conditions[0].text = next.conditions[0].quote[..n].to_string();
        save(&mut f.connection, "ws", &f.project_id, next).unwrap();
    }
    let first = list(
        &f.connection,
        "ws",
        &f.project_id,
        &input.evidence_artifact_id,
        0,
    )
    .unwrap();
    let second = list(
        &f.connection,
        "ws",
        &f.project_id,
        &input.evidence_artifact_id,
        25,
    )
    .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 25);
    assert_eq!(first["hasMore"], true);
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["hasMore"], false);
    for n in 0..26 {
        let mut e = f.evidence.clone();
        e.conclusion = format!("Synthetic reviewer note {n}");
        save_evidence(&f.connection, "ws", &f.project_id, e).unwrap();
    }
    assert_eq!(
        sources(&f.connection, "ws", &f.project_id, 0).unwrap()["hasMore"],
        true
    );
    assert_eq!(
        sources(&f.connection, "ws", &f.project_id, 25).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for n in 0..256 {
        crate::storage::repositories::knowledge::create_knowledge_base(
            &f.connection,
            "ws",
            &format!("Synthetic scope {n}"),
        )
        .unwrap();
    }
    assert_eq!(
        search_evidence(&f.connection, "ws", "hardness").unwrap_err(),
        "literature_evidence_base_budget"
    );
    close(f);
}
