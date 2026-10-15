use super::*;
pub(super) struct EuropePmc;
impl LiteratureMetadataProvider for EuropePmc {
    fn endpoint(&self) -> &'static str {
        "https://www.ebi.ac.uk/europepmc/webservices/rest/search"
    }
    fn parameters(&self, s: &Search) -> Vec<(String, String)> {
        let mut query = s.query.clone();
        if s.from_year.is_some() || s.to_year.is_some() {
            query = format!(
                "({query}) AND FIRST_PDATE:[{}-01-01 TO {}-12-31]",
                s.from_year.unwrap_or(1000),
                s.to_year.unwrap_or(2200)
            );
        }
        vec![
            ("query".into(), query),
            ("format".into(), "json".into()),
            ("resultType".into(), "core".into()),
            ("pageSize".into(), "20".into()),
            ("cursorMark".into(), s.cursor.clone().unwrap_or("*".into())),
        ]
    }
    fn parse(&self, body: &Value) -> Result<MetadataPage, String> {
        let items = body["resultList"]["result"]
            .as_array()
            .ok_or("europe_pmc_invalid_response")?;
        if items.len() > 20 {
            return Err("europe_pmc_item_limit".into());
        }
        let mut rows = Vec::new();
        for item in items {
            let title = item["title"].as_str().unwrap_or("");
            if title.is_empty() {
                continue;
            }
            let source = item["source"]
                .as_str()
                .ok_or("europe_pmc_invalid_identity")?;
            let id = item["id"].as_str().ok_or("europe_pmc_invalid_identity")?;
            if source.is_empty()
                || source.len() > 16
                || !source.chars().all(|c| c.is_ascii_uppercase())
                || id.is_empty()
                || id.len() > 128
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
            {
                return Err("europe_pmc_invalid_identity".into());
            }
            let doi = item["doi"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(normalize_doi)
                .transpose()?;
            let authors = if let Some(list) = item["authorList"]["author"].as_array() {
                if list.len() > 100 {
                    return Err("europe_pmc_author_limit".into());
                }
                list.iter()
                    .filter_map(|a| a["fullName"].as_str().map(str::to_string))
                    .collect()
            } else {
                let parts: Vec<_> = item["authorString"]
                    .as_str()
                    .unwrap_or("")
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                if parts.len() > 100 {
                    return Err("europe_pmc_author_limit".into());
                }
                parts
            };
            let year = match item.get("pubYear").filter(|v| !v.is_null()) {
                None => None,
                Some(Value::String(value)) => Some(
                    value
                        .parse::<i64>()
                        .map_err(|_| "europe_pmc_invalid_year")?,
                ),
                Some(value) => Some(value.as_i64().ok_or("europe_pmc_invalid_year")?),
            };
            rows.push(Metadata {
                provider: "europe_pmc".into(),
                external_id: format!("{source}:{id}"),
                doi,
                title: title.into(),
                authors,
                journal: item["journalInfo"]["journal"]["title"]
                    .as_str()
                    .map(str::to_string),
                year,
                abstract_text: item["abstractText"].as_str().map(str::to_string),
                source_url: format!("https://europepmc.org/article/{source}/{id}"),
                license: item["license"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .map(str::to_string),
                fulltext_state: "link_only".into(),
            });
        }
        let cursor = if rows.is_empty() {
            None
        } else {
            body["nextCursorMark"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        if cursor.as_ref().is_some_and(|s| s.len() > 4096) {
            return Err("europe_pmc_cursor_limit".into());
        }
        Ok((rows, cursor))
    }
}
