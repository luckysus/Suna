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
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadProfile {
    pub connection_id: String,
    pub database: String,
    pub table: String,
    pub columns: Vec<String>,
    pub batch_column: String,
    pub row_limit: u64,
}
fn identifier(s: &str) -> Result<String, String> {
    if s.is_empty() || s.len() > 128 || !s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("invalid_read_identifier".into());
    }
    Ok(format!("[{s}]"))
}
pub fn prepare(p: &ReadProfile) -> Result<String, String> {
    if p.columns.is_empty()
        || p.columns.len() > 128
        || p.columns
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != p.columns.len()
        || !p.columns.contains(&p.batch_column)
        || p.row_limit == 0
        || p.row_limit > 10000
        || p.database.trim().is_empty()
        || p.database.len() > 128
    {
        return Err("invalid_read_profile".into());
    }
    let table = p
        .table
        .split('.')
        .map(identifier)
        .collect::<Result<Vec<_>, _>>()?;
    if table.len() > 2 {
        return Err("invalid_read_table".into());
    }
    let columns = p
        .columns
        .iter()
        .map(|c| identifier(c))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!(
        "SELECT {} FROM {} WHERE {} = @P1",
        columns.join(","),
        table.join("."),
        identifier(&p.batch_column)?
    ))
}
pub fn save_profile(c: &Connection, ws: &str, p: ReadProfile) -> Result<String, String> {
    prepare(&p)?;
    let connection = Uuid::parse_str(&p.connection_id).map_err(|_| "invalid_connection_id")?;
    connections::get(c, ws, connection)?.ok_or("connection_not_found_in_workspace")?;
    let id = Uuid::new_v4().to_string();
    c.execute(
        "INSERT INTO database_read_profiles VALUES(?1,?2,?3,?4,?5)",
        params![
            ws,
            id,
            p.connection_id,
            json!(p).to_string(),
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .map_err(err)?;
    Ok(id)
}
pub fn profiles(c: &Connection, ws: &str) -> Result<Value, String> {
    let mut q=c.prepare("SELECT id,profile_json FROM database_read_profiles WHERE workspace_id=?1 ORDER BY created_at DESC,id LIMIT 100").map_err(err)?;
    let rows=q.query_map([ws],|r|{let raw:String=r.get(1)?;Ok(json!({"id":r.get::<_,String>(0)?,"profile":serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null)}))}).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    Ok(json!(rows))
}
pub fn submit_read(
    c: &Connection,
    ws: &str,
    p: &str,
    id: &str,
    batch: &str,
) -> Result<crate::tasks::TaskRecord, String> {
    projects::require_active(c, ws, p)?;
    if batch.trim().is_empty() || batch.len() > 1000 {
        return Err("invalid_source_batch".into());
    }
    let raw: String = c
        .query_row(
            "SELECT profile_json FROM database_read_profiles WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .ok_or("read_profile_not_found")?;
    let profile: ReadProfile = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let sql = prepare(&profile)?;
    let connection = connections::get(
        c,
        ws,
        Uuid::parse_str(&profile.connection_id).map_err(|_| "invalid_connection_id")?,
    )?
    .ok_or("connection_not_found")?;
    if !connection.enabled {
        return Err("connection_disabled".into());
    }
    crate::tasks::repository::create(c,crate::tasks::NewTask{workspace_id:ws.into(),kind:crate::database::query_task::DATABASE_QUERY_KIND.into(),payload_json:json!({"_research_project_id":p,"connection_id":profile.connection_id,"database":profile.database,"sql":sql,"row_limit":profile.row_limit,"parameters":[batch],"read_profile_id":id,"source_batch":batch}).to_string(),checkpoint_json:None,next_run_at:None,progress:0}).map_err(|e|e.to_string())
}
pub fn import_query(
    c: &Connection,
    ws: &str,
    p: &str,
    root: &Path,
    task: &str,
) -> Result<steel::SteelDatasetRecord, String> {
    projects::require_active(c, ws, p)?;
    let id = Uuid::parse_str(task).map_err(|_| "invalid_query_task_id")?;
    let task_record = crate::tasks::repository::get(c, ws, id)
        .map_err(|e| e.to_string())?
        .ok_or("query_task_not_found")?;
    if task_record.kind != crate::database::query_task::DATABASE_QUERY_KIND
        || task_record.state != crate::tasks::TaskState::Completed
    {
        return Err("query_not_completed".into());
    }
    let result = results::get(c, ws, id)?.ok_or("query_result_missing")?;
    let payload: Value =
        serde_json::from_str(&task_record.payload_json).map_err(|e| e.to_string())?;
    if payload["_research_project_id"].as_str() != Some(p) {
        return Err("query_outside_project".into());
    }
    if result.rows.len() > 10000 || result.columns.len() > 256 {
        return Err("query_snapshot_limit".into());
    }
    let mut csv = result
        .columns
        .iter()
        .map(|s| crate::database::catalog::csv_cell(Some(s)))
        .collect::<Vec<_>>()
        .join(",")
        + "\n";
    for r in &result.rows {
        csv += &(r
            .iter()
            .map(|s| crate::database::catalog::csv_cell(s.as_deref()))
            .collect::<Vec<_>>()
            .join(",")
            + "\n");
        if csv.len() > 32 * 1024 * 1024 {
            return Err("query_snapshot_byte_limit".into());
        }
    }
    let path = folder(root)?.join(format!("{}.csv", Uuid::new_v4()));
    let response = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(csv.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        let source = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source.clone(),
            sheet: None,
        })?;
        artifacts::atomic(c, || {
            let dataset = steel::save_new_preview_scoped(
                c,
                ws,
                &source,
                &hash(csv.as_bytes()),
                &preview,
                &[],
                p,
            )?;
            snapshot(
                c,
                ws,
                p,
                &dataset,
                "production_readonly",
                json!({"connectionRef":result.connection_id,"queryTaskId":task,"queryTemplate":result.query_text,"queryTemplateSha256":hash(result.query_text.as_bytes()),"parameterSha256":hash(payload["parameters"].to_string().as_bytes()),"queryTime":result.created_at,"sourceBatch":payload["source_batch"],"columnMapping":result.columns,"rowCount":result.row_count,"truncated":result.truncated,"datasetSha256":dataset.source_sha256,"schemaVersion":"production-source.v1"}),
            )?;
            Ok(dataset)
        })
    })();
    if response.is_err() {
        let _ = fs::remove_file(path);
    }
    response
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Connection, std::path::PathBuf, String, String) {
        let root = std::env::temp_dir().join(format!("suna-工业合成-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            projects::SaveResearchProject {
                id: None,
                name: "工业合成验证".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        let id = Uuid::new_v4();
        connections::save(
            &mut c,
            "ws",
            &connections::DatabaseConnectionRecord {
                id,
                display_name: "隔离连接夹具（未联网）".into(),
                host: "127.0.0.1".into(),
                port: 1433,
                database_name: "fixture".into(),
                username: "readonly_fixture".into(),
                timeout_ms: 1000,
                enabled: true,
                last_checked_at: None,
                last_latency_ms: None,
                last_version: None,
                last_error: None,
            },
        )
        .unwrap();
        (c, root, p.id, id.to_string())
    }
    fn profile(connection: &str) -> ReadProfile {
        ReadProfile {
            connection_id: connection.into(),
            database: "fixture".into(),
            table: "dbo.heats".into(),
            columns: vec!["C".into(), "HV".into(), "batch".into()],
            batch_column: "batch".into(),
            row_limit: 100,
        }
    }
    #[test]
    fn templates_are_parameterized_whitelisted_immutable_and_workspace_scoped() {
        let (c, root, p, connection) = fixture();
        let profile_id = save_profile(&c, "ws", profile(&connection)).unwrap();
        let injection = "B'; DROP TABLE heats--";
        let task = submit_read(&c, "ws", &p, &profile_id, injection).unwrap();
        let payload: Value = serde_json::from_str(&task.payload_json).unwrap();
        assert_eq!(
            payload["sql"],
            "SELECT [C],[HV],[batch] FROM [dbo].[heats] WHERE [batch] = @P1"
        );
        assert!(!payload["sql"].as_str().unwrap().contains(injection));
        assert_eq!(payload["parameters"][0], injection);
        assert!(submit_read(&c, "other", &p, &profile_id, "b").is_err());
        assert!(c
            .execute("UPDATE database_read_profiles SET profile_json='{}'", [])
            .is_err());
        let mut bad = profile(&connection);
        bad.table = "dbo.heats;DROP TABLE x".into();
        assert!(prepare(&bad).is_err());
        bad = profile(&connection);
        bad.batch_column = "not_whitelisted".into();
        assert!(prepare(&bad).is_err());
        bad = profile(&connection);
        bad.row_limit = 10001;
        assert!(prepare(&bad).is_err());
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_exchange_creates_real_snapshot_with_simulation_identity_without_software_license() {
        let (c, root, p, _) = fixture();
        let path = root.join("合成软件导出.json");
        let bytes=b"[{\"temperature\":800,\"phase_fraction\":0.2},{\"temperature\":900,\"phase_fraction\":0.3}]";
        fs::write(&path, bytes).unwrap();
        let request = FileImport {
            project_id: p.clone(),
            source_path: path.to_string_lossy().into(),
            adapter_id: "custom_file".into(),
            software_version: "synthetic-fixture-v1".into(),
            rights_basis: "synthetic local test data".into(),
            rights_confirmed: true,
            batch: "synthetic-A".into(),
            input_conditions: json!({"chem.C":{"value":0.3,"unit":"wt%"}}),
            output_units: std::collections::BTreeMap::from([
                ("temperature".into(), "degC".into()),
                ("phase_fraction".into(), "fraction".into()),
            ]),
            mappings: vec![],
        };
        let d = file_import(&c, "ws", &root, request).unwrap();
        assert_eq!(d.row_count, 2);
        assert_eq!(d.mapping_state, "draft");
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&d.source_path).unwrap(), bytes);
        let snapshot: String = c
            .query_row(
                "SELECT metadata_json FROM material_source_snapshots",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(snapshot.contains("external_simulation"));
        assert!(!snapshot.contains(root.to_str().unwrap()));
        assert_eq!(artifacts::edges(&c, "ws", &p, 25, 0).unwrap().len(), 1);
        assert!(c
            .execute("DELETE FROM material_source_snapshots", [])
            .is_err());
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn completed_query_cache_creates_new_dataset_and_preserves_query_parameters_without_credentials(
    ) {
        let (c, root, p, connection) = fixture();
        let spec = profile(&connection);
        let profile_id = save_profile(&c, "ws", spec).unwrap();
        let task = submit_read(&c, "ws", &p, &profile_id, "synthetic-batch").unwrap();
        let payload: Value = serde_json::from_str(&task.payload_json).unwrap();
        let record = results::QueryResultRecord {
            task_id: task.id,
            connection_id: Uuid::parse_str(&connection).unwrap(),
            database_name: "fixture".into(),
            query_text: payload["sql"].as_str().unwrap().into(),
            row_count: 2,
            truncated: false,
            duration_ms: 1,
            csv_path: "unused fixture path".into(),
            columns: vec!["C".into(), "HV".into(), "batch".into()],
            rows: vec![
                vec![
                    Some("0.3".into()),
                    Some("500".into()),
                    Some("synthetic-batch".into()),
                ],
                vec![
                    Some("0.4".into()),
                    Some("550".into()),
                    Some("synthetic-batch".into()),
                ],
            ],
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        results::insert(&c, "ws", &record).unwrap();
        c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json='{\"stage\":\"completed\"}' WHERE id=?1",[task.id.to_string()]).unwrap();
        artifacts::reconcile_task(&c, "ws", &task.id.to_string()).unwrap();
        let d = import_query(&c, "ws", &p, &root, &task.id.to_string()).unwrap();
        assert_eq!(d.row_count, 2);
        let text = c
            .query_row(
                "SELECT metadata_json FROM material_source_snapshots",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap();
        assert!(text.contains("queryTemplateSha256"));
        assert!(text.contains("synthetic-batch"));
        assert!(!text.contains("readonly_fixture"));
        assert!(import_query(&c, "other", &p, &root, &task.id.to_string()).is_err());
        assert!(import_query(&c, "ws", &p, &root, &task.id.to_string()).is_err());
        assert_eq!(steel::list(&c, "ws").unwrap().len(), 1);
        drop(c);
        let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        assert_eq!(steel::list(&c, "ws").unwrap().len(), 1);
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
}
