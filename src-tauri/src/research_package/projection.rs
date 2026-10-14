//! Project verified through the actual Suna repository, then allowlist metadata.
use crate::research_package::{identity, label, valid_sha};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub struct Projection {
    pub project_id: String,
    pub records: Vec<Value>,
    pub dependencies: Vec<Value>,
    pub edges: Vec<Value>,
    pub app_version: String,
}
pub fn from_connection(c: &Connection, ws: &str, project: &str) -> Result<Projection, String> {
    // Read paginated objects and frozen artifacts from one SQLite snapshot, even
    // when a scheduler connection commits new results during the export.
    crate::storage::repositories::research_artifacts::atomic(c, || {
        let source = crate::storage::repositories::research_delivery::manifest(c, ws, project)?;
        from_manifest(&source)
    })
}
pub fn from_manifest(m: &Value) -> Result<Projection, String> {
    if m["schemaVersion"] != "research-manifest.v1" {
        return Err("unsupported_project_manifest".into());
    }
    let project_id = identity(
        m["project"]["id"]
            .as_str()
            .ok_or("package_project_missing")?,
    )?;
    let objects = m["objects"]
        .as_array()
        .filter(|a| a.len() <= 10000)
        .ok_or("package_object_budget")?;
    let artifacts = m["artifacts"]
        .as_array()
        .filter(|a| a.len() <= 10000)
        .ok_or("package_object_budget")?;
    let mut records = BTreeMap::new();
    let mut dependencies = BTreeMap::new();
    records.insert(
        project_id.clone(),
        json!({"id":project_id,"kind":"project","name":label(m["project"]["name"].as_str())}),
    );
    for obj in objects {
        let id = identity(obj["id"].as_str().ok_or("package_resource_id_invalid")?)?;
        let origin = obj["kind"].as_str().ok_or("package_object_kind_invalid")?;
        if let Some(previous) = records.get(&id) {
            if previous["origin_kind"] != origin {
                return Err("package_object_identity_conflict".into());
            }
            continue;
        }
        let kind = match origin {
            "model" | "dataset" | "report" => origin,
            _ => "artifact",
        };
        records.insert(id.clone(),json!({"id":id,"kind":kind,"origin_kind":label(Some(origin)),"name":label(obj["title"].as_str())}));
        if ["model", "dataset", "document"].contains(&origin) {
            dependencies.insert(
                id.clone(),
                json!({"resource_id":id,"kind":origin,"status":"missing"}),
            );
        }
    }
    for artifact in artifacts {
        let id = identity(
            artifact["id"]
                .as_str()
                .ok_or("package_resource_id_invalid")?,
        )?;
        if records
            .get(&id)
            .is_some_and(|old| old["origin_kind"] != "artifact")
        {
            return Err("package_object_identity_conflict".into());
        }
        let sha = artifact["contentSha256"]
            .as_str()
            .filter(|v| valid_sha(v))
            .ok_or("package_artifact_digest_missing")?;
        let kind = artifact["kind"]
            .as_str()
            .ok_or("package_object_kind_invalid")?;
        let mut record = json!({"id":id,"kind":"artifact","origin_kind":label(Some(kind)),"origin_schema":label(artifact["schemaVersion"].as_str()),"name":label(artifact["title"].as_str()),"source_artifact_sha256":sha});
        let p = &artifact["payload"];
        if !p.is_null() && !p.is_object() {
            return Err("package_artifact_schema_invalid".into());
        }
        if kind == "dataset_snapshot" {
            if let Some(sha) = p["source_sha256"].as_str().filter(|v| valid_sha(v)) {
                record["source_sha256"] = json!(sha);
                if let Some(dep) = p["dataset_id"]
                    .as_str()
                    .and_then(|id| dependencies.get_mut(id))
                {
                    expected_digest(dep, sha)?;
                }
            }
            if let Some(columns) = p["columns"].as_array() {
                if columns.len() > 256 {
                    return Err("package_unit_schema_invalid".into());
                }
                record["units"]=json!(columns.iter().map(|c|json!({"name":label(c["canonical_field"].as_str().or(c["original_name"].as_str())),"unit":c["unit"].as_str().map(|v|label(Some(v)))})).collect::<Vec<_>>());
            }
        }
        if kind == "model_snapshot" {
            if let Some(sha) = p["model_sha256"]
                .as_str()
                .or(p["result"]["model_sha256"].as_str())
                .filter(|v| valid_sha(v))
            {
                record["model_sha256"] = json!(sha);
                if let Some(dep) = p["model_id"]
                    .as_str()
                    .and_then(|id| dependencies.get_mut(id))
                {
                    expected_digest(dep, sha)?;
                }
            }
            if let Some(v) = p["version"].as_u64().filter(|v| *v > 0) {
                record["version"] = json!(v);
            }
            if let Some(task) = p["training_task_id"]
                .as_str()
                .filter(|v| identity(v).is_ok())
            {
                record["source_task_id"] = json!(task);
            }
            if let Some(algorithm) = p["scientific_cards"]["model_card"]["algorithm"]
                .as_str()
                .or(p["manifest"]["algorithm"].as_str())
                .or(p["request"]["algorithm"].as_str())
            {
                record["algorithm"] = json!(algorithm);
            }
            let card = &p["scientific_cards"]["model_card"];
            if let Some(version) = card["artifact_version"]
                .as_str()
                .or(p["manifest"]["artifact_version"].as_str())
            {
                record["artifact_version"] = json!(version);
            }
            let mut units = Vec::new();
            if let Some(fields) = card["features"].as_array() {
                if fields.len() > 128 {
                    return Err("package_unit_schema_invalid".into());
                }
                units.extend(fields.iter().map(|f| json!({"name":label(f["name"].as_str()),"unit":f["unit"].as_str().map(|v|label(Some(v)))})));
            }
            if let Some(name) = card["target"]["name"].as_str() {
                units.push(json!({"name":label(Some(name)),"unit":card["target"]["unit"].as_str().map(|v|label(Some(v)))}));
            }
            if !units.is_empty() {
                record["units"] = json!(units);
            }
        }
        records.insert(id, record);
    }
    if records.len() > 2047 {
        return Err("package_object_budget".into());
    }
    let edges=m["edges"].as_array().filter(|e|e.len()<=10000).ok_or("package_edge_budget")?.iter().map(|edge|{
        Ok(json!({"from_id":identity(edge["fromId"].as_str().ok_or("package_edge_invalid")?)?,"to_id":identity(edge["toId"].as_str().ok_or("package_edge_invalid")?)?,"relation":edge["relation"].as_str().ok_or("package_edge_invalid")?}))
    }).collect::<Result<Vec<Value>,String>>()?;
    Ok(Projection {
        project_id,
        records: records.into_values().collect(),
        dependencies: dependencies.into_values().collect(),
        edges,
        app_version: label(m["appVersion"].as_str()),
    })
}

fn expected_digest(dependency: &mut Value, sha: &str) -> Result<(), String> {
    if dependency["expected_sha256"]
        .as_str()
        .is_some_and(|old| old != sha)
    {
        return Err("package_dependency_digest_conflict".into());
    }
    dependency["expected_sha256"] = json!(sha);
    Ok(())
}
