use super::*;
pub(super) struct OpenAlex;

impl LiteratureMetadataProvider for OpenAlex {
    fn endpoint(&self) -> &'static str {
        "https://api.openalex.org/works"
    }
    fn parameters(&self, s: &Search) -> Vec<(String, String)> {
        let mut q = vec![
            ("search".into(), s.query.clone()),
            ("per-page".into(), "20".into()),
            ("cursor".into(), s.cursor.clone().unwrap_or("*".into())),
            ("select".into(), "id,doi,title,authorships,publication_year,primary_location,abstract_inverted_index".into()),
        ];
        let mut filters = vec![];
        if let Some(y) = s.from_year {
            filters.push(format!("from_publication_date:{y}-01-01"));
        }
        if let Some(y) = s.to_year {
            filters.push(format!("to_publication_date:{y}-12-31"));
        }
        if !filters.is_empty() {
            q.push(("filter".into(), filters.join(",")));
        }
        q
    }
    fn parse(&self, body: &Value) -> Result<MetadataPage, String> {
        let items = body["results"]
            .as_array()
            .ok_or("openalex_invalid_response")?;
        if items.len() > 20 {
            return Err("openalex_item_limit".into());
        }
        let mut rows = vec![];
        for item in items {
            let title = item["title"].as_str().unwrap_or("");
            if title.trim().is_empty() {
                continue;
            }
            let id = item["id"]
                .as_str()
                .and_then(|s| s.strip_prefix("https://openalex.org/"))
                .ok_or("openalex_invalid_identity")?;
            let digits = id.strip_prefix('W').ok_or("openalex_invalid_identity")?;
            if digits.is_empty() || digits.len() > 32 || !digits.bytes().all(|c| c.is_ascii_digit())
            {
                return Err("openalex_invalid_identity".into());
            }
            let authors = match item.get("authorships").filter(|v| !v.is_null()) {
                None => vec![],
                Some(value) => {
                    let list = value.as_array().ok_or("openalex_invalid_authors")?;
                    if list.len() > 100 {
                        return Err("openalex_author_limit".into());
                    }
                    list.iter()
                        .filter_map(|a| a["author"]["display_name"].as_str().map(str::to_string))
                        .collect()
                }
            };
            let doi = match item.get("doi").filter(|v| !v.is_null()) {
                None => None,
                Some(value) => Some(normalize_doi(
                    value.as_str().ok_or("openalex_invalid_doi")?,
                )?),
            };
            let year = match item.get("publication_year").filter(|v| !v.is_null()) {
                None => None,
                Some(value) => {
                    let year = value
                        .as_i64()
                        .filter(|y| (1000..=2200).contains(y))
                        .ok_or("openalex_invalid_year")?;
                    Some(year)
                }
            };
            rows.push(Metadata {
                provider: "openalex".into(),
                external_id: id.into(),
                doi,
                title: title.into(),
                authors,
                journal: item["primary_location"]["source"]["display_name"]
                    .as_str()
                    .map(str::to_string),
                year,
                abstract_text: abstract_text(&item["abstract_inverted_index"])?,
                // Provider-supplied download/landing URLs are never fetched or returned.
                source_url: format!("https://openalex.org/{id}"),
                license: item["primary_location"]["license"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .map(str::to_string),
                fulltext_state: "link_only".into(),
            });
        }
        let cursor = if rows.is_empty() {
            None
        } else {
            match body["meta"].get("next_cursor").filter(|v| !v.is_null()) {
                None => None,
                Some(value) => {
                    let value = value.as_str().ok_or("openalex_invalid_cursor")?;
                    if value.len() > 4096 || value.chars().any(char::is_control) {
                        return Err("openalex_cursor_limit".into());
                    }
                    (!value.is_empty()).then(|| value.to_string())
                }
            }
        };
        Ok((rows, cursor))
    }
}

// Reconstruct only the exact indexed word sequence. Never fill gaps, truncate,
// infer missing words, or treat this bibliographic abstract as full-text evidence.
fn abstract_text(index: &Value) -> Result<Option<String>, String> {
    if index.is_null() {
        return Ok(None);
    }
    let words = index.as_object().ok_or("openalex_invalid_abstract")?;
    if words.len() > 6000 {
        return Err("openalex_abstract_limit".into());
    }
    let mut ordered = std::collections::BTreeMap::new();
    let mut bytes = 0usize;
    for (word, positions) in words {
        if word.is_empty() || word.len() > 500 || word.chars().any(char::is_control) {
            return Err("openalex_invalid_abstract".into());
        }
        let positions = positions.as_array().ok_or("openalex_invalid_abstract")?;
        if positions.is_empty() || positions.len() > 6000 {
            return Err("openalex_abstract_limit".into());
        }
        for position in positions {
            let position = position
                .as_u64()
                .filter(|p| *p < 6000)
                .ok_or("openalex_abstract_limit")?;
            if ordered.insert(position, word.as_str()).is_some() {
                return Err("openalex_abstract_position_conflict".into());
            }
            bytes = bytes
                .checked_add(word.len() + 1)
                .ok_or("openalex_abstract_limit")?;
            if bytes > 30000 {
                return Err("openalex_abstract_limit".into());
            }
        }
    }
    if ordered.is_empty() {
        return Ok(None);
    }
    if ordered.keys().enumerate().any(|(i, p)| i as u64 != *p) {
        return Err("openalex_abstract_position_gap".into());
    }
    Ok(Some(
        ordered.values().copied().collect::<Vec<_>>().join(" "),
    ))
}
