use super::*;
use serde_json::json;
fn search(id: ProviderId) -> Search {
    Search {
        provider: id,
        query: "钢铁 toughness".into(),
        cursor: Some("opaque+cursor".into()),
        from_year: Some(2020),
        to_year: Some(2024),
    }
}
#[test]
fn legacy_requests_keep_crossref_and_unregistered_endpoints_are_rejected() {
    let old: Search = serde_json::from_value(
        json!({"query":"steel","cursor":null,"fromYear":null,"toYear":null}),
    )
    .unwrap();
    assert_eq!(old.provider, ProviderId::Crossref);
    assert!(
        serde_json::from_value::<Search>(json!({"query":"steel","provider":"unknown"})).is_err()
    );
    assert!(serde_json::from_value::<Search>(
        json!({"query":"steel","endpoint":"http://127.0.0.1"})
    )
    .is_err());
    assert!(ProviderId::parse("unknown").is_none());
    assert_eq!(
        serde_json::to_value(search(ProviderId::EuropePmc)).unwrap()["provider"],
        "europe_pmc"
    );
}
#[test]
fn provider_queries_preserve_filters_cursors_and_fixed_budgets() {
    let cross = provider(ProviderId::Crossref);
    let c = cross.parameters(&search(ProviderId::Crossref));
    assert!(c.contains(&("rows".into(), "20".into())));
    assert!(c.contains(&(
        "filter".into(),
        "from-pub-date:2020-01-01,until-pub-date:2024-12-31".into()
    )));
    assert!(c.contains(&("cursor".into(), "opaque+cursor".into())));
    let europe = provider(ProviderId::EuropePmc);
    let e = europe.parameters(&search(ProviderId::EuropePmc));
    assert!(e.contains(&(
        "query".into(),
        "(钢铁 toughness) AND FIRST_PDATE:[2020-01-01 TO 2024-12-31]".into()
    )));
    assert!(e.contains(&("pageSize".into(), "20".into())));
    assert!(e.contains(&("cursorMark".into(), "opaque+cursor".into())));
    assert!(e.contains(&("resultType".into(), "core".into())));
    assert_eq!(
        europe.endpoint(),
        "https://www.ebi.ac.uk/europepmc/webservices/rest/search"
    );
}
#[test]
fn european_metadata_retains_doi_provenance_unknowns_and_link_only_rights() {
    let body = json!({"resultList":{"result":[{"source":"MED","id":"12345","title":"Synthetic steel metadata","doi":"https://doi.org/10.1234/ABC","pubYear":"2021","authorList":{"author":[{"fullName":"Fixture Author"}]},"journalInfo":{"journal":{"title":"Synthetic Journal"}},"isOpenAccess":"Y"},{"source":"PPR","id":"PPR123","title":"Synthetic preprint without DOI"}]},"nextCursorMark":"next"});
    let (rows, next) = provider(ProviderId::EuropePmc).parse(&body).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].provider, "europe_pmc");
    assert_eq!(rows[0].external_id, "MED:12345");
    assert_eq!(rows[0].doi.as_deref(), Some("10.1234/abc"));
    assert_eq!(rows[0].year, Some(2021));
    assert_eq!(
        rows[0].source_url,
        "https://europepmc.org/article/MED/12345"
    );
    assert!(rows[0].license.is_none());
    assert_eq!(rows[0].fulltext_state, "link_only");
    assert!(rows[1].doi.is_none() && rows[1].year.is_none() && rows[1].abstract_text.is_none());
    assert_eq!(next.as_deref(), Some("next"));
}
#[test]
fn malformed_metadata_identity_paging_and_oversized_pages_do_not_become_results() {
    let adapter = provider(ProviderId::EuropePmc);
    for row in [
        json!({"id":"../../private","source":"MED","title":"bad"}),
        json!({"id":"1","source":"https://foreign.invalid","title":"bad"}),
        json!({"id":"1","source":"MED","title":"bad","doi":"invalid"}),
        json!({"id":"1","source":"MED","title":"bad","pubYear":"unknown"}),
    ] {
        assert!(adapter
            .parse(&json!({"resultList":{"result":[row]}}))
            .is_err());
    }
    assert!(adapter.parse(&json!({"error":"upstream error"})).is_err());
    assert!(adapter
        .parse(&json!({"resultList":{"result":vec![json!({});21]}}))
        .is_err());
    assert!(adapter.parse(&json!({"resultList":{"result":[{"id":"1","source":"MED","title":"fixture"}]},"nextCursorMark":"x".repeat(4097)})).is_err());
    let (empty, next) = adapter
        .parse(&json!({"resultList":{"result":[]},"nextCursorMark":"unused"}))
        .unwrap();
    assert!(empty.is_empty() && next.is_none());
}
