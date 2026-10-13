use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetVersionRequest {
    pub project_id: String,
    pub execution_ids: Vec<String>,
    pub features: Vec<String>,
    pub properties: Vec<String>,
    pub parent_dataset_id: Option<String>,
}
pub fn create_dataset_version(
    c: &Connection,
    ws: &str,
    root: &std::path::Path,
    request: DatasetVersionRequest,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let (headers, new_rows, measurement_ids) = reviewed_table(
        c,
        ws,
        &request.project_id,
        &request.execution_ids,
        &request.features,
        &request.properties,
    )?;
    let first = get(c, ws, &request.execution_ids[0])?;
    let mut units = first.variable_units.clone();
    for m in &first.measurements {
        units.insert(m.data.property.clone(), m.data.unit.clone());
    }
    let mut rows: Vec<Vec<String>> = vec![];
    if let Some(parent) = &request.parent_dataset_id {
        let dataset = crate::storage::repositories::steel::get(c, ws, parent)?
            .ok_or("parent_dataset_not_found")?;
        let linked:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 AND target_kind='dataset' AND target_id=?3)",params![ws,request.project_id,parent],|r|r.get(0)).map_err(err)?;
        if !linked {
            return Err("parent_dataset_not_in_project".into());
        }
        if crate::steel::hash_dataset_source(&dataset.source_path)? != dataset.source_sha256 {
            return Err("parent_dataset_source_changed".into());
        }
        let table = crate::steel::read_dataset_table(&crate::steel::DatasetPreviewRequest {
            source_path: dataset.source_path,
            sheet: Some(dataset.selected_sheet),
        })?;
        if table.truncated {
            return Err("cannot_version_a_truncated_dataset".into());
        }
        let selected: Vec<usize> = request
            .features
            .iter()
            .chain(&request.properties)
            .map(|field| {
                let col = dataset
                    .columns
                    .iter()
                    .find(|column| {
                        column.canonical_field.as_deref() == Some(field.as_str())
                            || column.original_name == *field
                    })
                    .ok_or("parent_field_not_found")?;
                if col.unit.as_ref() != units.get(field) {
                    return Err("parent_dataset_unit_mismatch");
                }
                Ok(col.ordinal)
            })
            .collect::<Result<_, &str>>()?;
        for source in table.rows {
            let mut row: Vec<String> = selected
                .iter()
                .map(|i| source.get(*i).cloned().unwrap_or_default())
                .collect();
            for name in [
                "sample_id",
                "heat_id",
                "batch_id",
                "execution_id",
                "source_type",
            ] {
                row.push(
                    table
                        .headers
                        .iter()
                        .position(|h| h == name)
                        .and_then(|i| source.get(i))
                        .cloned()
                        .unwrap_or_default(),
                );
            }
            rows.push(row);
        }
    }
    rows.extend(
        new_rows
            .into_iter()
            .map(|r| r.into_iter().map(Option::unwrap_or_default).collect()),
    );
    let bytes = crate::steel::to_csv(&headers, &rows).into_bytes();
    let directory = root.join("research-datasets");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let canonical_root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
    let canonical_dir = std::fs::canonicalize(&directory).map_err(|e| e.to_string())?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err("managed_dataset_directory_outside_root".into());
    }
    let path = canonical_dir.join(format!("实验测量-{}.csv", Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let roots = crate::permissions::path::AuthorizedRoots::new(vec![canonical_root])
        .map_err(|e| e.to_string())?;
    let authorized = roots.authorize(&path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    roots
        .verify_opened_file(&authorized, &file)
        .map_err(|e| e.to_string())?;
    roots
        .verify_target(&authorized, &path)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        use std::io::Write;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        let source_path = path.to_string_lossy().to_string();
        let preview = crate::steel::preview_dataset(&crate::steel::DatasetPreviewRequest {
            source_path: source_path.clone(),
            sheet: None,
        })?;
        let sha = crate::steel::hash_dataset_source(&source_path)?;
        let mappings: Vec<crate::storage::repositories::steel::DatasetColumnMapping> = headers
            .iter()
            .enumerate()
            .map(
                |(ordinal, name)| crate::storage::repositories::steel::DatasetColumnMapping {
                    ordinal,
                    canonical_field: Some(
                        if name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                            name.to_ascii_lowercase()
                        } else {
                            format!("measurement_column_{ordinal}")
                        },
                    ),
                    unit: units.get(name).cloned(),
                },
            )
            .collect();
        artifacts::atomic(c, || {
            projects::require_active(c, ws, &request.project_id)?;
            let dataset = crate::storage::repositories::steel::save_new_preview_scoped(
                c,
                ws,
                &source_path,
                &sha,
                &preview,
                &mappings,
                &request.project_id,
            )?;
            projects::attach_in_transaction(
                c,
                ws,
                &request.project_id,
                "dataset",
                &dataset.id,
                "derived_from",
            )?;
            c.execute(
                "INSERT INTO research_dataset_versions VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    ws,
                    Uuid::new_v4().to_string(),
                    request.project_id,
                    dataset.id,
                    request.parent_dataset_id,
                    json!(measurement_ids).to_string(),
                    Utc::now().to_rfc3339()
                ],
            )
            .map_err(err)?;
            let snapshot = artifacts::dataset_snapshot(c, ws, &request.project_id, &dataset.id)?
                .ok_or("dataset_snapshot_missing")?;
            for execution in &request.execution_ids {
                let a:String=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='experiment_measurement' AND source_key=?3",params![ws,request.project_id,execution],|r|r.get(0)).map_err(err)?;
                artifacts::edge(c, ws, &a, &snapshot, "uses_dataset")?;
            }
            if let Some(parent) = &request.parent_dataset_id {
                if let Some(previous) =
                    artifacts::dataset_snapshot(c, ws, &request.project_id, parent)?
                {
                    artifacts::edge(c, ws, &previous, &snapshot, "supersedes")?;
                }
            }
            projects::audit(
                c,
                ws,
                &request.project_id,
                "measurement_dataset_created",
                json!({"dataset_id":dataset.id,"measurement_count":measurement_ids.len(),"training":"manual_confirmation_required"}),
            )?;
            Ok(dataset)
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}
