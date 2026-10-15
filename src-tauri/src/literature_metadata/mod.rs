//! Public bibliographic adapters. Full-text retrieval and evidence review stay in
//! the existing permissioned RAG/research flow; these adapters return link-only metadata.
use serde::{Deserialize, Serialize};
use serde_json::Value;
mod crossref;
mod europe_pmc;
mod openalex;
mod types;
pub use crossref::parse_crossref;
pub use types::{normalize_doi, Metadata, Search};
pub type MetadataPage = (Vec<Metadata>, Option<String>);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderId {
    #[default]
    #[serde(rename = "crossref")]
    Crossref,
    #[serde(rename = "europe_pmc")]
    EuropePmc,
    #[serde(rename = "openalex")]
    OpenAlex,
}
impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crossref => "crossref",
            Self::EuropePmc => "europe_pmc",
            Self::OpenAlex => "openalex",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "crossref" => Some(Self::Crossref),
            "europe_pmc" => Some(Self::EuropePmc),
            "openalex" => Some(Self::OpenAlex),
            _ => None,
        }
    }
}
pub trait LiteratureMetadataProvider: Sync {
    fn endpoint(&self) -> &'static str;
    fn parameters(&self, search: &Search) -> Vec<(String, String)>;
    fn parse(&self, body: &Value) -> Result<MetadataPage, String>;
}
pub fn provider(id: ProviderId) -> &'static dyn LiteratureMetadataProvider {
    match id {
        ProviderId::Crossref => &Crossref,
        ProviderId::EuropePmc => &europe_pmc::EuropePmc,
        ProviderId::OpenAlex => &openalex::OpenAlex,
    }
}
struct Crossref;
impl LiteratureMetadataProvider for Crossref {
    fn endpoint(&self) -> &'static str {
        "https://api.crossref.org/works"
    }
    fn parameters(&self, s: &Search) -> Vec<(String, String)> {
        let mut q = vec![
            ("query".into(), s.query.clone()),
            ("rows".into(), "20".into()),
            ("cursor".into(), s.cursor.clone().unwrap_or("*".into())),
        ];
        let mut filters = vec![];
        if let Some(y) = s.from_year {
            filters.push(format!("from-pub-date:{y}-01-01"));
        }
        if let Some(y) = s.to_year {
            filters.push(format!("until-pub-date:{y}-12-31"));
        }
        if !filters.is_empty() {
            q.push(("filter".into(), filters.join(",")));
        }
        q
    }
    fn parse(&self, body: &Value) -> Result<MetadataPage, String> {
        parse_crossref(body)
    }
}
#[cfg(test)]
mod openalex_tests;
#[cfg(test)]
mod tests;
