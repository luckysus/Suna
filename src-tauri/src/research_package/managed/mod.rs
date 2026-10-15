//! Explicitly selected managed assets. Imported assets are isolated and never executed.
pub mod archive;
pub mod catalog;
pub mod library;
mod snapshot;
pub mod staging;
use super::{digest, identity, valid_sha};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const VERSION: &str = "research-managed-package.v1";
pub const MAX_ASSETS: usize = 512;
pub const MAX_ASSET: usize = 32 * 1024 * 1024;
pub const MAX_TOTAL: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asset {
    pub resource_id: String,
    pub role: String,
    pub extension: String,
    pub sha256: String,
    pub bytes: usize,
}
impl Asset {
    pub fn name(&self) -> String {
        format!("assets/{}.{}", self.sha256, self.extension)
    }
    fn validate(&self) -> Result<(), String> {
        identity(&self.resource_id)?;
        if !valid_sha(&self.sha256)
            || self.bytes == 0
            || self.bytes > MAX_ASSET
            || !matches!(
                (self.role.as_str(), self.extension.as_str()),
                ("dataset_source", "csv" | "json")
                    | ("image_original", "png" | "jpg" | "jpeg" | "tif" | "tiff")
                    | ("model_artifact", "json")
            )
        {
            return Err("package_asset_schema_invalid".into());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    metadata_sha256: String,
    assets: Vec<Asset>,
    rights_status: String,
}

/// Construction is private: staging only accepts a fully verified archive.
pub struct Verified {
    package_sha256: String,
    metadata_bytes: Vec<u8>,
    metadata: super::ValidatedPackage,
    assets: Vec<(Asset, Vec<u8>)>,
}
impl Verified {
    pub fn summary(&self) -> Value {
        let included: std::collections::BTreeSet<_> = self
            .assets
            .iter()
            .map(|(a, _)| a.resource_id.as_str())
            .collect();
        let missing: Vec<_> = self
            .metadata
            .missing_dependencies
            .iter()
            .filter(|d| !included.contains(d["resource_id"].as_str().unwrap_or("")))
            .collect();
        json!({"schemaVersion":VERSION,"scope":"managed_assets_isolated","packageSha256":self.package_sha256,
            "sourceProjectId":self.metadata.source_project_id,"metadata":self.metadata.summary(),
            "assets":self.assets.iter().map(|(a,_)|a).collect::<Vec<_>>(),"assetCount":self.assets.len(),
            "assetBytes":self.assets.iter().map(|(a,_)|a.bytes).sum::<usize>(),"missingDependencies":missing,
            "rightsStatus":"user_declared_authorized","rightsIndependentlyVerified":false,
            "writesToLiveWorkspace":false,"modelsActivated":false,"measurementsCreated":false})
    }
}

pub struct Prepared {
    bytes: Vec<u8>,
    summary: Value,
}
impl Prepared {
    pub fn summary(&self) -> &Value {
        &self.summary
    }
    pub fn confirm(self, expected: &str) -> Result<Self, String> {
        if !valid_sha(expected) || self.summary["packageSha256"] != expected {
            return Err("package_preview_changed".into());
        }
        Ok(self)
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod library_tests;
#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
