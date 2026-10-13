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
