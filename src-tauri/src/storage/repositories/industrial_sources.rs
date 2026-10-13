//! File exchange and read-only production source snapshots use the existing dataset store.
use super::{
    database_connections as connections, database_query_results as results,
    research_artifacts as artifacts, research_projects as projects, steel,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use uuid::Uuid;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub id: &'static str,
    pub label: &'static str,
    pub capability: &'static str,
    pub mode: &'static str,
    pub input_schema_version: &'static str,
    pub output_schema_version: &'static str,
    pub requires_license: bool,
    pub requires_user_approval: bool,
}
pub fn adapters() -> Vec<Adapter> {
    vec![
        Adapter {
            id: "custom_file",
            label: "实验室文件交换",
            capability: "custom",
            mode: "file_import",
            input_schema_version: "material-file.v1",
            output_schema_version: "material-file.v1",
            requires_license: false,
            requires_user_approval: true,
        },
        Adapter {
            id: "jmatpro_file",
            label: "JMatPro 导出文件",
            capability: "thermophysical",
            mode: "file_import",
            input_schema_version: "material-file.v1",
            output_schema_version: "material-file.v1",
            requires_license: true,
            requires_user_approval: true,
        },
        Adapter {
            id: "thermocalc_file",
            label: "Thermo-Calc 导出文件",
            capability: "phase_equilibrium",
            mode: "file_import",
            input_schema_version: "material-file.v1",
            output_schema_version: "material-file.v1",
            requires_license: true,
            requires_user_approval: true,
        },
    ]
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileImport {
    pub project_id: String,
    pub source_path: String,
    pub adapter_id: String,
    pub software_version: String,
    pub rights_basis: String,
    pub rights_confirmed: bool,
    pub batch: String,
    pub input_conditions: Value,
    pub output_units: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub mappings: Vec<steel::DatasetColumnMapping>,
}
fn snapshot(
    c: &Connection,
    ws: &str,
    p: &str,
    d: &steel::SteelDatasetRecord,
    kind: &str,
    metadata: Value,
) -> Result<(), String> {
    let body = metadata.to_string();
    let sha = hash(body.as_bytes());
    let id = Uuid::new_v4().to_string();
    c.execute(
        "INSERT INTO material_source_snapshots VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            ws,
            id,
            p,
            d.id,
            kind,
            body,
            sha,
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .map_err(err)?;
    let a = artifacts::create(
        c,
        ws,
        p,
        "analysis_result",
        "专业数据来源快照",
        &format!("source:{id}"),
        None,
        json!({"datasetId":d.id,"sourceKind":kind,"metadata":metadata,"metadataSha256":sha,"sourceSha256":d.source_sha256}),
    )?;
    let dataset =
        artifacts::dataset_snapshot(c, ws, p, &d.id)?.ok_or("missing_dataset_snapshot")?;
    artifacts::edge(c, ws, &a, &dataset, "based_on_evidence")?;
    projects::audit(
        c,
        ws,
        p,
        "material_source_imported",
        json!({"datasetId":d.id,"sourceKind":kind,"sha256":d.source_sha256}),
    )
}
fn folder(root: &Path) -> Result<std::path::PathBuf, String> {
    let folder = root.join("objects").join("material-data");
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    if !folder
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("material_storage_outside_data_root".into());
    }
    Ok(folder)
}
pub fn file_import(
    c: &Connection,
    ws: &str,
    root: &Path,
    input: FileImport,
) -> Result<steel::SteelDatasetRecord, String> {
    projects::require_active(c, ws, &input.project_id)?;
    if !input.rights_confirmed
        || input.rights_basis.trim().is_empty()
        || input.rights_basis.len() > 500
        || input.software_version.trim().is_empty()
        || input.software_version.len() > 160
        || input.batch.len() > 160
        || !adapters().iter().any(|a| a.id == input.adapter_id)
        || input.output_units.len() > 256
        || input.input_conditions.to_string().len() > 20000
    {
        return Err("invalid_material_adapter_source_or_rights".into());
    }
    let conditions = input
        .input_conditions
        .as_object()
        .ok_or("material_conditions_must_be_typed_object")?;
    for (name, v) in conditions {
        if name.len() > 160
            || v["value"].as_f64().is_none_or(|n| !n.is_finite())
            || v["unit"]
                .as_str()
                .is_none_or(|u| u.trim().is_empty() || u.len() > 64)
        {
            return Err("invalid_material_input_condition".into());
        }
    }
    let (auth, file) = crate::permissions::path::authorize_existing_file_with_handle(Path::new(
        &input.source_path,
    ))
    .map_err(|_| "material_file_not_authorized")?;
    let extension = auth
        .canonical_path()
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "csv" | "xlsx" | "json") {
        return Err("material_adapter_supported_csv_xlsx_json".into());
    }
    let mut bytes = vec![];
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "material_source_read_failed")?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("material_source_byte_limit".into());
    }
    let path = folder(root)?.join(format!("{}.{}", Uuid::new_v4(), extension));
    let result = (|| {
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        output
            .write_all(&bytes)
            .and_then(|_| output.sync_all())
            .map_err(|e| e.to_string())?;
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })?;
        for (column, unit) in &input.output_units {
            if unit.trim().is_empty()
                || unit.len() > 64
                || !preview.columns.iter().any(|c| &c.name == column)
            {
                return Err("invalid_material_output_units".into());
            }
        }
        let metadata = json!({"adapterId":input.adapter_id,"mode":"file_import","softwareVersion":input.software_version,"rightsBasis":input.rights_basis,"sourceBatch":input.batch,"inputConditions":input.input_conditions,"outputUnits":input.output_units,"schemaVersion":"material-file.v1","sourceType":"external_simulation","originalFileName":auth.canonical_path().file_name().unwrap_or_default().to_string_lossy(),"fileSha256":hash(&bytes),"importedAt":chrono::Utc::now().to_rfc3339(),"limitations":["用户提供的软件导出文件；未调用商业软件API或实际设备","软件计算值不等同于实测性能","文件内模型、许可证及软件运行真实性需来源方核对"]});
        artifacts::atomic(c, || {
            let d = steel::save_new_preview_scoped(
                c,
                ws,
                &source,
                &hash(&bytes),
                &preview,
                &input.mappings,
                &input.project_id,
            )?;
            snapshot(
                c,
                ws,
                &input.project_id,
                &d,
                "external_simulation",
                metadata,
            )?;
            Ok(d)
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
#[cfg(test)]
#[path = "industrial_sources/tests.rs"]
mod tests;

mod production;
pub use production::{import_query, prepare, profiles, save_profile, submit_read, ReadProfile};
