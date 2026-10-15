//! Metadata package v1. No file extraction, live database writes or asset restoration.
pub mod archive;
pub mod delivery;
pub mod managed;
pub mod projection;
pub mod selected_file;
pub mod strict_json;
pub mod temporary;
pub mod validation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const VERSION: &str = "research-package-manifest.v1";
pub const MAX_ENTRIES: usize = 2048;
pub const MAX_TOTAL: usize = 64 * 1024 * 1024;
pub const MAX_FILE: usize = 8 * 1024 * 1024;
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hash(value: &Value) -> Result<String, String> {
    Ok(digest(
        &serde_json::to_vec(value).map_err(|_| "package_json_invalid")?,
    ))
}
pub fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub fn identity(value: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(value)
        .map(|id| id.to_string())
        .map_err(|_| "package_resource_id_invalid".into())
}
pub fn label(value: Option<&str>) -> String {
    value
        .filter(|v| {
            !v.trim().is_empty()
                && v.chars().count() <= 128
                && !v.chars().any(char::is_control)
                && !v.trim().starts_with(['/', '\\'])
                && !v.contains(':')
        })
        .unwrap_or("未公开名称")
        .to_string()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub bytes: usize,
    pub sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub scope: String,
    pub source_project_id: String,
    pub entries: Vec<Entry>,
    pub external_dependencies: Vec<Value>,
    pub edges: Vec<Value>,
    pub app_version: String,
    pub assets_included: bool,
    pub provenance_mode: String,
    pub rights_status: String,
}
pub struct ValidatedPackage {
    receipt_sha256: String,
    source_project_id: String,
    objects: Vec<(String, Value, String)>,
    missing_dependencies: Vec<Value>,
    edges: Vec<Value>,
}
impl ValidatedPackage {
    pub fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }
    pub fn summary(&self) -> Value {
        serde_json::json!({"schemaVersion":VERSION,"scope":"metadata_only","sourceProjectId":self.source_project_id,
        "receiptSha256":self.receipt_sha256,"metadataRecordCount":self.objects.len(),"edgeCount":self.edges.len(),
        "missingDependencies":self.missing_dependencies,"assetsIncluded":false,"provenanceMode":"references_only","rightsStatus":"not_assessed","writesToLiveWorkspace":false,"assetsRestored":false})
    }
}
#[cfg(test)]
mod tests;
