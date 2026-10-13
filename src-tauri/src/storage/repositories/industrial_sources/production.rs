use super::*;

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
