use super::*;
use crate::rag::{
    chunk::{chunk_document, ChunkPolicy},
    ingest::SourceFormat,
    model::{NewChunk, NewDocumentVersion, NewSourceDocument},
    parse::{parse_document_bytes, ParseLimits},
};
use crate::storage::repositories::knowledge;
pub(super) struct Fixture {
    pub root: std::path::PathBuf,
    pub connection: Connection,
    pub project_id: String,
    pub evidence: Evidence,
}
pub(super) fn fixture() -> Fixture {
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
    let bytes=b"# Synthetic measurements\n\nSynthetic fixture hardness 500 HV and strength 1500 MPa. Not real research evidence. Tensile strength 1.5 GPa and custom marker 2 arb. Synthetic overflow 1e308 GPa.\n";
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
    Fixture {
        root,
        connection: c,
        project_id: p.id,
        evidence,
    }
}
#[test]
fn real_local_parser_evidence_review_dataset_and_missing_source_gates() {
    let Fixture {
        root,
        connection: c,
        project_id,
        evidence,
    } = fixture();
    assert!(verify_evidence(&c, "other", &evidence).is_err());
    let mut bad = evidence.clone();
    bad.value = 500.;
    bad.unit = "MPa".into();
    bad.quote = "strength 1500 MPa".into();
    assert!(verify_evidence(&c, "ws", &bad).is_err());
    bad = evidence.clone();
    bad.unit = "J".into();
    assert!(verify_evidence(&c, "ws", &bad).is_err());
    let id = save_evidence(&c, "ws", &project_id, evidence.clone()).unwrap();
    assert_eq!(id, save_evidence(&c, "ws", &project_id, evidence).unwrap());
    let dataset = evidence_dataset(&c, "ws", &project_id, &root).unwrap();
    assert_eq!(dataset.row_count, 1);
    assert_eq!(dataset.mapping_state, "draft");
    let rows = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
        source_path: dataset.source_path,
        sheet: None,
    })
    .unwrap();
    assert!(rows.headers.contains(&"sourceType".into()));
    assert!(evidence_dataset(&c, "ws", &project_id, &root).is_err());
    assert_eq!(evidence_rows(&c, "ws", &project_id).unwrap().len(), 1);
    drop(c);
    let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    assert_eq!(evidence_rows(&c, "ws", &project_id).unwrap().len(), 1);
    drop(c);
    std::fs::remove_dir_all(root).unwrap();
}
