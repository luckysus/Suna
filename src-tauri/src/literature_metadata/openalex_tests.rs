use super::*;
use serde_json::json;

fn page(row: Value) -> Value {
    json!({"results":[row],"meta":{"next_cursor":"opaque+cursor/="}})
}
fn work() -> Value {
    json!({"id":"https://openalex.org/W123456","title":"Synthetic steel work"})
}
#[test]
fn openalex_search_is_anonymous_encoded_data_with_fixed_endpoint_and_page_budget() {
    let adapter = provider(ProviderId::OpenAlex);
    assert_eq!(adapter.endpoint(), "https://api.openalex.org/works");
    let search = Search {
        provider: ProviderId::OpenAlex,
        query: "steel 钢+?&api_key=not-a-credential".into(),
        cursor: Some("opaque+cursor/=".into()),
        from_year: Some(2020),
        to_year: Some(2024),
    };
    let params = adapter.parameters(&search);
    assert!(params.contains(&("search".into(), search.query.clone())));
    assert!(params.contains(&(
        "filter".into(),
        "from_publication_date:2020-01-01,to_publication_date:2024-12-31".into()
    )));
    assert!(params.contains(&("cursor".into(), "opaque+cursor/=".into())));
    assert!(params.contains(&("per-page".into(), "20".into())));
    assert!(!params
        .iter()
        .any(|(key, _)| matches!(key.as_str(), "api_key" | "mailto" | "endpoint")));
    let mut url = reqwest::Url::parse(adapter.endpoint()).unwrap();
    url.query_pairs_mut().extend_pairs(&params);
    assert_eq!(
        url.query_pairs().filter(|(key, _)| key == "search").count(),
        1
    );
    assert!(!url.query_pairs().any(|(key, _)| key == "api_key"));
    assert_eq!(
        serde_json::to_value(&search).unwrap()["provider"],
        "openalex"
    );
    assert!(serde_json::from_value::<Search>(
        json!({"provider":"openalex","query":"steel","api_key":"not-accepted"})
    )
    .is_err());
}
#[test]
fn openalex_exact_abstract_doi_and_source_survive_without_inferred_license_or_fulltext() {
    let row = json!({"id":"https://openalex.org/W123456","title":"Synthetic work","doi":"https://doi.org/10.1234/ABC","publication_year":2021,"authorships":[{"author":{"display_name":"Fixture Author"}}],"primary_location":{"source":{"display_name":"Synthetic Journal"},"license":null,"landing_page_url":"http://127.0.0.1/private","pdf_url":"file:///private"},"open_access":{"is_oa":true},"abstract_inverted_index":{"钢":[0,2],"improves":[1],"toughness.":[3]}});
    let (rows, next) = provider(ProviderId::OpenAlex).parse(&page(row)).unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.provider, "openalex");
    assert_eq!(row.external_id, "W123456");
    assert_eq!(row.doi.as_deref(), Some("10.1234/abc"));
    assert_eq!(row.year, Some(2021));
    assert_eq!(row.authors, ["Fixture Author"]);
    assert_eq!(row.journal.as_deref(), Some("Synthetic Journal"));
    assert_eq!(
        row.abstract_text.as_deref(),
        Some("钢 improves 钢 toughness.")
    );
    assert_eq!(row.source_url, "https://openalex.org/W123456");
    assert!(row.license.is_none());
    assert_eq!(row.fulltext_state, "link_only");
    assert_eq!(next.as_deref(), Some("opaque+cursor/="));
    let raw = serde_json::to_string(row).unwrap();
    assert!(!raw.contains("127.0.0.1") && !raw.contains("file:///"));
    let (missing, _) = provider(ProviderId::OpenAlex).parse(&page(work())).unwrap();
    assert!(
        missing[0].doi.is_none()
            && missing[0].year.is_none()
            && missing[0].abstract_text.is_none()
            && missing[0].journal.is_none()
            && missing[0].authors.is_empty()
    );
    let mut licensed = work();
    licensed["primary_location"] = json!({"license":"cc-by"});
    assert_eq!(
        provider(ProviderId::OpenAlex)
            .parse(&page(licensed))
            .unwrap()
            .0[0]
            .license
            .as_deref(),
        Some("cc-by")
    );
}
#[test]
fn openalex_invalid_identities_fields_pages_and_cursors_fail_without_guessing() {
    let adapter = provider(ProviderId::OpenAlex);
    for id in [
        "https://foreign.invalid/W1",
        "https://openalex.org.evil/W1",
        "https://openalex.org/W1/../../private",
        "http://openalex.org/W1",
        "https://openalex.org/W",
        "https://openalex.org/A1",
        "https://openalex.org/W1?api_key=private",
    ] {
        let mut row = work();
        row["id"] = json!(id);
        assert!(adapter.parse(&page(row)).is_err(), "{id}");
    }
    for (key, value) in [
        ("doi", json!(9)),
        ("doi", json!("invalid")),
        ("publication_year", json!("2020")),
        ("publication_year", json!(2300)),
        ("authorships", json!({})),
        ("authorships", json!(vec![json!({}); 101])),
    ] {
        let mut row = work();
        row[key] = value;
        assert!(adapter.parse(&page(row)).is_err(), "{key}");
    }
    assert!(adapter
        .parse(&json!({"error":"not a results page"}))
        .is_err());
    assert!(adapter.parse(&json!({"results":vec![work();21]})).is_err());
    for cursor in [json!(1), json!("x".repeat(4097)), json!("cursor\nunsafe")] {
        assert!(adapter
            .parse(&json!({"results":[work()],"meta":{"next_cursor":cursor}}))
            .is_err());
    }
    let (rows, cursor) = adapter
        .parse(&json!({"results":[],"meta":{"next_cursor":"unused"}}))
        .unwrap();
    assert!(rows.is_empty() && cursor.is_none());
}
#[test]
fn openalex_abstract_cannot_fill_holes_duplicate_positions_or_expand_unbounded() {
    for index in [
        json!("not an index"),
        json!({"word":[1]}),
        json!({"a":[0],"b":[0]}),
        json!({"word":[6000]}),
        json!({"word":[-1]}),
        json!({"word":[0.5]}),
        json!({"word":[]}),
        json!({"a\nprivate":[0]}),
        json!({"word":(0..6001).collect::<Vec<_>>()}),
    ] {
        let mut row = work();
        row["abstract_inverted_index"] = index;
        assert!(provider(ProviderId::OpenAlex).parse(&page(row)).is_err());
    }
    let mut row = work();
    row["abstract_inverted_index"] = json!({"x".repeat(500):(0..61).collect::<Vec<_>>()});
    assert!(provider(ProviderId::OpenAlex).parse(&page(row)).is_err());
    let mut row = work();
    row["abstract_inverted_index"] = json!({});
    assert!(
        provider(ProviderId::OpenAlex).parse(&page(row)).unwrap().0[0]
            .abstract_text
            .is_none()
    );
}
