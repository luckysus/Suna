use super::*;

pub fn parse_crossref(body: &Value) -> Result<(Vec<Metadata>, Option<String>), String> {
    let items = body["message"]["items"]
        .as_array()
        .ok_or("crossref_invalid_response")?;
    if items.len() > 20 {
        return Err("crossref_item_limit".into());
    }
    let mut records = vec![];
    for item in items {
        let doi = item["DOI"].as_str().map(normalize_doi).transpose()?;
        let title = item["title"][0].as_str().unwrap_or("").to_string();
        if title.is_empty() {
            continue;
        }
        let Some(id) = doi.clone() else { continue };
        let authors = item["author"]
            .as_array()
            .map(|a| {
                a.iter()
                    .take(100)
                    .map(|r| {
                        format!(
                            "{} {}",
                            r["given"].as_str().unwrap_or(""),
                            r["family"].as_str().unwrap_or("")
                        )
                        .trim()
                        .into()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let year = item["published"]["date-parts"][0][0]
            .as_i64()
            .or(item["issued"]["date-parts"][0][0].as_i64());
        records.push(Metadata {
            provider: "crossref".into(),
            external_id: id.clone(),
            doi: Some(id.clone()),
            title,
            authors,
            journal: item["container-title"][0].as_str().map(str::to_string),
            year,
            abstract_text: item["abstract"].as_str().map(str::to_string),
            source_url: format!("https://doi.org/{id}"),
            license: item["license"][0]["URL"].as_str().map(str::to_string),
            fulltext_state: "link_only".into(),
        });
    }
    let cursor = body["message"]["next-cursor"]
        .as_str()
        .filter(|_| !records.is_empty())
        .map(str::to_string);
    Ok((records, cursor))
}
