use super::*;
use crate::storage::repositories::literature_discovery::evidence_tests::{fixture, Fixture};

fn setup() -> (Fixture, SaveContext) {
    let f = fixture();
    save_evidence(&f.connection, "ws", &f.project_id, f.evidence.clone()).unwrap();
    let source = sources(&f.connection, "ws", &f.project_id, 0).unwrap()["items"][0].clone();
    let text: String = f
        .connection
        .query_row(
            "SELECT text FROM knowledge_chunks WHERE id=?1",
            [&f.evidence.chunk_id],
            |r| r.get(0),
        )
        .unwrap();
    let input = SaveContext {
        evidence_artifact_id: source["artifactId"].as_str().unwrap().into(),
        evidence_sha256: source["evidenceSha256"].as_str().unwrap().into(),
        confirmed: true,
        conditions: vec![CitedCondition {
            kind: ConditionKind::TestConditions,
            text: "Synthetic fixture".into(),
            quote: "Synthetic fixture hardness 500 HV and strength 1500 MPa.".into(),
            chunk_id: f.evidence.chunk_id.clone(),
            chunk_sha256: hash(&text),
        }],
    };
    (f, input)
}
fn close(f: Fixture) {
    drop(f.connection);
    std::fs::remove_dir_all(f.root).unwrap();
}
fn foreign_condition(c: &Connection) -> CitedCondition {
    use crate::rag::{
        chunk::{chunk_document, ChunkPolicy},
        ingest::SourceFormat,
        model::{NewChunk, NewDocumentVersion, NewSourceDocument},
        parse::{parse_document_bytes, ParseLimits},
    };
    use crate::storage::repositories::knowledge;
    let bytes = b"# Foreign fixture\n\nForeign tests used ISO 6892. Not a measurement.\n";
    let parsed =
        parse_document_bytes(bytes, SourceFormat::Markdown, ParseLimits::default()).unwrap();
    let chunks = chunk_document(&parsed, &ChunkPolicy::default()).unwrap();
    let kb = knowledge::create_knowledge_base(c, "ws", "Other synthetic paper").unwrap();
    let doc = knowledge::create_source_document(
        c,
        "ws",
        NewSourceDocument {
            knowledge_base_id: kb.id,
            display_name: "Other fixture.md".into(),
            source_kind: "markdown".into(),
        },
    )
    .unwrap();
    let version = knowledge::create_document_version(
        c,
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
            c,
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
    let chunk = chunks.iter().find(|x| x.text.contains("ISO 6892")).unwrap();
    CitedCondition {
        kind: ConditionKind::TestMethod,
        text: "ISO 6892".into(),
        quote: "Foreign tests used ISO 6892.".into(),
        chunk_id: chunk.id.to_string(),
        chunk_sha256: chunk.content_sha256.clone(),
    }
}

#[test]
fn context_is_real_cited_append_only_idempotent_and_reopens_without_training_or_measurements() {
    let (mut f, input) = setup();
    let originals = evidence_rows(&f.connection, "ws", &f.project_id).unwrap();
    let old = artifacts::get(&f.connection, "ws", &input.evidence_artifact_id).unwrap();
    let receipt = save(&mut f.connection, "ws", &f.project_id, input.clone()).unwrap();
    let changes: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        receipt,
        save(&mut f.connection, "ws", &f.project_id, input.clone()).unwrap()
    );
    assert_eq!(
        f.connection
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        changes
    );
    let first = list(
        &f.connection,
        "ws",
        &f.project_id,
        &input.evidence_artifact_id,
        0,
    )
    .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    let p = &first["items"][0]["snapshot"];
    assert_eq!(p["conditions"][0]["text"], "Synthetic fixture");
    assert_eq!(p["scientificComparability"], "not_assessed");
    assert_eq!(p["unitsAndProcessOrderInferred"], false);
    assert_eq!(p["rawSourceBytesVerified"], false);
    assert_eq!(p["evidenceSha256"], old.content_sha256);
    assert_eq!(hash(&p.to_string()), receipt["contentSha256"]);
    let mut another = input.clone();
    another.conditions[0].text = "hardness 500 HV".into();
    save(&mut f.connection, "ws", &f.project_id, another).unwrap();
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
        2
    );
    assert_eq!(
        evidence_rows(&f.connection, "ws", &f.project_id).unwrap(),
        originals
    );
    assert_eq!(
        artifacts::get(&f.connection, "ws", &old.id)
            .unwrap()
            .content_sha256,
        old.content_sha256
    );
    let counts:(i64,i64,i64,i64)=f.connection.query_row("SELECT (SELECT COUNT(*) FROM background_tasks),(SELECT COUNT(*) FROM steel_datasets),(SELECT COUNT(*) FROM steel_models),(SELECT COUNT(*) FROM experiment_measurements)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
    assert!(f
        .connection
        .execute(
            "UPDATE research_artifacts SET immutable_payload_json='{}' WHERE id=?1",
            [receipt["artifactId"].as_str().unwrap()]
        )
        .is_err());
    let project = f.project_id.clone();
    let root = f.root.clone();
    drop(f.connection);
    let c = Connection::open(root.join("研究.sqlite")).unwrap();
    assert_eq!(
        list(&c, "ws", &project, &input.evidence_artifact_id, 0).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_confirmation_forged_quotes_units_and_private_fields_cannot_be_reviewed_context() {
    let (mut f, input) = setup();
    for edit in 0..7 {
        let mut bad = input.clone();
        match edit {
            0 => bad.confirmed = false,
            1 => bad.conditions.clear(),
            2 => bad.conditions = vec![bad.conditions[0].clone(); 25],
            3 => bad.conditions[0].text = "Invented heat treatment 900 degC".into(),
            4 => {
                bad.conditions[0].quote = "Invented sample".into();
                bad.conditions[0].text = "sample".into();
            }
            5 => bad.conditions[0].chunk_sha256 = "0".repeat(64),
            _ => bad.evidence_sha256 = "0".repeat(64),
        }
        assert!(
            save(&mut f.connection, "ws", &f.project_id, bad).is_err(),
            "{edit}"
        );
    }
    assert_eq!(
        list(
            &f.connection,
            "ws",
            &f.project_id,
            &input.evidence_artifact_id,
            0
        )
        .unwrap()["items"],
        json!([])
    );

    close(f);
}
#[test]
fn contexts_cannot_cross_workspace_project_document_or_changed_source() {
    let (mut f, input) = setup();
    assert!(sources(&f.connection, "other", &f.project_id, 0).is_err());
    assert!(save(&mut f.connection, "other", &f.project_id, input.clone()).is_err());
    let second = projects::save(
        &mut f.connection,
        "ws",
        projects::SaveResearchProject {
            id: None,
            name: "另一合成项目".into(),
            description: "".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    assert!(save(&mut f.connection, "ws", &second.id, input.clone()).is_err());
    assert!(list(
        &f.connection,
        "ws",
        &second.id,
        &input.evidence_artifact_id,
        0
    )
    .is_err());
    let mut changed = input.clone();
    changed.conditions[0].chunk_id = "chunk-missing-fixture".into();
    assert!(save(&mut f.connection, "ws", &f.project_id, changed).is_err());
    let mut foreign = input.clone();
    foreign.conditions = vec![foreign_condition(&f.connection)];
    assert!(save(&mut f.connection, "ws", &f.project_id, foreign).is_err());
    f.connection
        .execute(
            "UPDATE knowledge_chunks SET text=text||' changed' WHERE id=?1",
            [&f.evidence.chunk_id],
        )
        .unwrap();
    assert!(save(&mut f.connection, "ws", &f.project_id, input.clone()).is_err());
    assert_eq!(
        list(
            &f.connection,
            "ws",
            &f.project_id,
            &input.evidence_artifact_id,
            0
        )
        .unwrap()["items"],
        json!([])
    );
    close(f);
}
#[test]
fn source_pages_are_read_only_and_archived_projects_and_invalid_offsets_cannot_write() {
    let (mut f, input) = setup();
    let before: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let rows = sources(&f.connection, "ws", &f.project_id, 0).unwrap();
    assert_eq!(rows["items"].as_array().unwrap().len(), 1);
    assert_eq!(rows["hasMore"], false);
    assert_eq!(
        f.connection
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        before
    );
    assert!(sources(&f.connection, "ws", &f.project_id, 1).is_err());
    assert!(list(
        &f.connection,
        "ws",
        &f.project_id,
        &input.evidence_artifact_id,
        10025
    )
    .is_err());
    projects::archive(&mut f.connection, "ws", &f.project_id, true).unwrap();
    assert!(save(&mut f.connection, "ws", &f.project_id, input).is_err());
    close(f);
}

#[path = "experimental_context_index_tests.rs"]
mod index_tests;
