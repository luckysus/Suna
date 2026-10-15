use super::evidence_tests::{fixture, Fixture};
use super::*;

fn archive_count(c: &Connection, p: &str) -> usize {
    artifacts::list(c, "ws", p, 100, 0).unwrap().len()
}
fn close(f: Fixture) {
    drop(f.connection);
    std::fs::remove_dir_all(f.root).unwrap();
}

#[test]
fn actual_reviewed_evidence_normalizes_units_archives_idempotently_and_reopens_without_changing_sources(
) {
    let mut f = fixture();
    save_evidence(&f.connection, "ws", &f.project_id, f.evidence.clone()).unwrap();
    let mut strength = f.evidence.clone();
    strength.field = "property.tensile_strength".into();
    strength.value = 1.5;
    strength.unit = "GPa".into();
    strength.quote = "Tensile strength 1.5 GPa".into();
    save_evidence(&f.connection, "ws", &f.project_id, strength).unwrap();
    let mut unknown = f.evidence.clone();
    unknown.field = "custom.synthetic_marker".into();
    unknown.value = 2.;
    unknown.unit = "arb".into();
    unknown.doi = None;
    unknown.quote = "custom marker 2 arb".into();
    save_evidence(&f.connection, "ws", &f.project_id, unknown).unwrap();
    let before = evidence_rows(&f.connection, "ws", &f.project_id).unwrap();
    let changes: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let preview = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(
        preview,
        quality::preview(&f.connection, "ws", &f.project_id).unwrap()
    );
    assert_eq!(
        f.connection
            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        changes
    );
    let report = &preview["report"];
    assert_eq!(report["counts"]["indexedReferenceMatched"], 3);
    assert_eq!(report["counts"]["unknownField"], 1);
    assert_eq!(report["counts"]["doiNotRecorded"], 1);
    let row = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["field"] == "property.tensile_strength")
        .unwrap();
    assert_eq!(row["normalizedValue"], 1500.);
    assert_eq!(row["canonicalUnit"], "MPa");
    let row = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["field"] == "custom.synthetic_marker")
        .unwrap();
    assert!(row["canonicalUnit"].is_null());
    assert!(row["normalizedValue"].is_null());
    for row in report["checks"].as_array().unwrap() {
        assert_eq!(row["comparability"], "conditions_not_recorded");
        assert_eq!(row["rawSourceBytesVerified"], false);
        assert!(row.get("quote").is_none());
        assert!(row.get("conclusion").is_none());
        let original =
            artifacts::get(&f.connection, "ws", row["artifactId"].as_str().unwrap()).unwrap();
        assert_eq!(original.content_sha256, row["evidenceSha256"]);
    }
    assert_eq!(archive_count(&f.connection, &f.project_id), 3);
    let sha = preview["reportSha256"].as_str().unwrap();
    let receipt = quality::archive(&mut f.connection, "ws", &f.project_id, sha).unwrap();
    assert_eq!(
        receipt,
        quality::archive(&mut f.connection, "ws", &f.project_id, sha).unwrap()
    );
    assert_eq!(archive_count(&f.connection, &f.project_id), 4);
    assert_eq!(
        evidence_rows(&f.connection, "ws", &f.project_id).unwrap(),
        before
    );
    let saved =
        artifacts::get(&f.connection, "ws", receipt["artifactId"].as_str().unwrap()).unwrap();
    assert_eq!(saved.payload, Some(report.clone()));
    assert_eq!(saved.content_sha256, sha);
    drop(f.connection);
    let mut c = Connection::open(f.root.join("研究.sqlite")).unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    assert_eq!(
        artifacts::get(&c, "ws", &saved.id).unwrap().content_sha256,
        saved.content_sha256
    );
    assert_eq!(quality::preview(&c, "ws", &f.project_id).unwrap(), preview);
    let (jobs,datasets,models):(i64,i64,i64)=c.query_row("SELECT (SELECT COUNT(*) FROM background_tasks),(SELECT COUNT(*) FROM steel_datasets),(SELECT COUNT(*) FROM steel_models)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!((jobs, datasets, models), (0, 0, 0));
    drop(c);
    std::fs::remove_dir_all(f.root).unwrap();
}

#[test]
fn current_index_damage_or_removal_is_explicit_and_cannot_save_a_stale_preview() {
    let mut f = fixture();
    save_evidence(&f.connection, "ws", &f.project_id, f.evidence.clone()).unwrap();
    let original = evidence_rows(&f.connection, "ws", &f.project_id).unwrap();
    let before = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    let sha = before["reportSha256"].as_str().unwrap();
    let saved = quality::archive(&mut f.connection, "ws", &f.project_id, sha).unwrap();
    let original_text: String = f
        .connection
        .query_row(
            "SELECT text FROM knowledge_chunks WHERE id=?1 AND version_id=?2",
            params![f.evidence.chunk_id, f.evidence.version_id],
            |r| r.get(0),
        )
        .unwrap();
    // Deliberate corruption of this disposable local index, not an application API.
    f.connection
        .execute(
            "UPDATE knowledge_chunks SET text=text||' altered' WHERE id=?1 AND version_id=?2",
            params![f.evidence.chunk_id, f.evidence.version_id],
        )
        .unwrap();
    let changed = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(changed["report"]["counts"]["changedIndexedSource"], 1);
    assert_eq!(
        quality::archive(&mut f.connection, "ws", &f.project_id, sha).unwrap_err(),
        "literature_quality_preview_changed"
    );
    assert_eq!(archive_count(&f.connection, &f.project_id), 2);
    f.connection.execute("UPDATE knowledge_chunks SET text=text||' a second alteration' WHERE id=?1 AND version_id=?2",params![f.evidence.chunk_id,f.evidence.version_id]).unwrap();
    let another = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(another["report"]["counts"]["changedIndexedSource"], 1);
    assert_ne!(another["reportSha256"], changed["reportSha256"]);
    assert_eq!(
        quality::archive(
            &mut f.connection,
            "ws",
            &f.project_id,
            changed["reportSha256"].as_str().unwrap()
        )
        .unwrap_err(),
        "literature_quality_preview_changed"
    );
    f.connection.execute("UPDATE knowledge_chunks SET text=?3,source_location_json=?4 WHERE id=?1 AND version_id=?2",params![f.evidence.chunk_id,f.evidence.version_id,original_text,json!({"syntheticChangedLocation":true}).to_string()]).unwrap();
    let moved = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(moved["report"]["counts"]["changedIndexedSource"], 1);
    assert_ne!(moved["reportSha256"], another["reportSha256"]);
    f.connection
        .execute(
            "DELETE FROM knowledge_chunks WHERE id=?1 AND version_id=?2",
            params![f.evidence.chunk_id, f.evidence.version_id],
        )
        .unwrap();
    f.connection
        .execute(
            "DELETE FROM literature_fulltext_links WHERE document_id=?1",
            [&f.evidence.document_id],
        )
        .unwrap();
    let missing = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(missing["report"]["counts"]["missingIndexedSource"], 1);
    assert_eq!(missing["report"]["counts"]["doiLinkMissing"], 1);
    assert_eq!(
        missing["report"]["checks"][0]["rawSourceBytesVerified"],
        false
    );
    assert_eq!(
        artifacts::get(&f.connection, "ws", saved["artifactId"].as_str().unwrap())
            .unwrap()
            .payload,
        Some(before["report"].clone())
    );
    assert_eq!(
        evidence_rows(&f.connection, "ws", &f.project_id).unwrap(),
        original
    );
    close(f);
}

#[test]
fn conversion_overflow_remains_unknown_and_never_serializes_a_false_finite_value() {
    let mut f = fixture();
    let mut overflow = f.evidence.clone();
    overflow.field = "property.tensile_strength".into();
    overflow.value = 1e308;
    overflow.unit = "GPa".into();
    overflow.quote = "Synthetic overflow 1e308 GPa".into();
    save_evidence(&f.connection, "ws", &f.project_id, overflow).unwrap();
    let result = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(result["report"]["counts"]["incompatibleUnit"], 1);
    assert_eq!(result["report"]["checks"][0]["value"], 1e308);
    assert!(result["report"]["checks"][0]["normalizedValue"].is_null());
    let saved = quality::archive(
        &mut f.connection,
        "ws",
        &f.project_id,
        result["reportSha256"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(saved["contentSha256"], result["reportSha256"]);
    close(f);
}

#[test]
fn project_unit_revision_invalidates_preview_and_hardness_scales_remain_incompatible() {
    let mut f = fixture();
    save_evidence(&f.connection, "ws", &f.project_id, f.evidence.clone()).unwrap();
    let original = evidence_rows(&f.connection, "ws", &f.project_id).unwrap();
    let before = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    let mut field = crate::steel::materials::base()
        .fields
        .into_iter()
        .find(|x| x.id == "property.hardness_hv")
        .unwrap();
    field.canonical_unit = "HRC".into();
    super::super::material_rules::save_overlay(
        &f.connection,
        "ws",
        &f.project_id,
        vec![field],
        vec!["property.hardness_hv".into()],
    )
    .unwrap();
    assert_eq!(
        quality::archive(
            &mut f.connection,
            "ws",
            &f.project_id,
            before["reportSha256"].as_str().unwrap()
        )
        .unwrap_err(),
        "literature_quality_preview_changed"
    );
    let current = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_ne!(
        current["report"]["registrySha256"],
        before["report"]["registrySha256"]
    );
    assert_eq!(current["report"]["counts"]["incompatibleUnit"], 1);
    assert!(current["report"]["checks"][0]["normalizedValue"].is_null());
    assert_eq!(
        evidence_rows(&f.connection, "ws", &f.project_id).unwrap(),
        original
    );
    assert_eq!(archive_count(&f.connection, &f.project_id), 1);
    close(f);
}

#[test]
fn empty_foreign_or_archived_project_never_creates_a_quality_result_or_training_data() {
    let mut f = fixture();
    let empty = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    assert_eq!(empty["report"]["reviewedEvidenceCount"], 0);
    assert_eq!(
        quality::archive(
            &mut f.connection,
            "ws",
            &f.project_id,
            empty["reportSha256"].as_str().unwrap()
        )
        .unwrap_err(),
        "no_reviewed_literature_evidence"
    );
    assert!(quality::preview(&f.connection, "foreign", &f.project_id).is_err());
    save_evidence(&f.connection, "ws", &f.project_id, f.evidence.clone()).unwrap();
    let actual = quality::preview(&f.connection, "ws", &f.project_id).unwrap();
    projects::archive(&mut f.connection, "ws", &f.project_id, true).unwrap();
    assert!(quality::archive(
        &mut f.connection,
        "ws",
        &f.project_id,
        actual["reportSha256"].as_str().unwrap()
    )
    .is_err());
    assert_eq!(archive_count(&f.connection, &f.project_id), 1);
    close(f);
}
