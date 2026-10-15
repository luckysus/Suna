use super::ProviderId;
use serde::{Deserialize, Serialize};

pub fn normalize_doi(s: &str) -> Result<String, String> {
    let mut s = s.trim().to_lowercase();
    for prefix in [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi:",
    ] {
        if let Some(v) = s.strip_prefix(prefix) {
            s = v.trim().into();
            break;
        }
    }
    let (prefix, suffix) = s.split_once('/').ok_or("invalid_doi")?;
    if !prefix.starts_with("10.")
        || !prefix[3..].chars().all(|c| c.is_ascii_digit())
        || prefix.len() < 7
        || suffix.is_empty()
        || s.len() > 300
        || s.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err("invalid_doi".into());
    }
    Ok(s)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    pub provider: String,
    pub external_id: String,
    pub doi: Option<String>,
    pub title: String,
    pub authors: Vec<String>,
    pub journal: Option<String>,
    pub year: Option<i64>,
    pub abstract_text: Option<String>,
    pub source_url: String,
    pub license: Option<String>,
    pub fulltext_state: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Search {
    #[serde(default)]
    pub provider: ProviderId,
    pub query: String,
    pub cursor: Option<String>,
    pub from_year: Option<u32>,
    pub to_year: Option<u32>,
}
