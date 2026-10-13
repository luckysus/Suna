use super::*;

pub fn list(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<SteelDatasetRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, source_name, source_path, source_sha256, format, selected_sheet,
                    row_count, column_count, truncated, mapping_state, preview_json,
                    created_at, updated_at
             FROM steel_datasets
             WHERE workspace_id = ?1
             ORDER BY updated_at DESC, id",
        )
        .map_err(|error| error.to_string())?;
    let parents = statement
        .query_map(params![workspace_id], map_parent)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    parents
        .into_iter()
        .map(|mut record| {
            record.columns = list_columns(connection, workspace_id, &record.id)?;
            Ok(record)
        })
        .collect()
}

pub fn get(
    connection: &Connection,
    workspace_id: &str,
    dataset_id: &str,
) -> Result<Option<SteelDatasetRecord>, String> {
    let mut record = connection
        .query_row(
            "SELECT id, source_name, source_path, source_sha256, format, selected_sheet,
                    row_count, column_count, truncated, mapping_state, preview_json,
                    created_at, updated_at
             FROM steel_datasets
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, dataset_id],
            map_parent,
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(record) = &mut record {
        record.columns = list_columns(connection, workspace_id, &record.id)?;
    }
    Ok(record)
}

fn list_columns(
    connection: &Connection,
    workspace_id: &str,
    dataset_id: &str,
) -> Result<Vec<SteelDatasetColumnRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT ordinal, original_name, duplicate, inferred_type, canonical_field, unit,
                    non_empty_count, missing_count, invalid_count, min_value, max_value
             FROM steel_dataset_columns
             WHERE workspace_id = ?1 AND dataset_id = ?2
             ORDER BY ordinal",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id, dataset_id], map_column)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string());
    rows
}

fn map_parent(row: &rusqlite::Row<'_>) -> rusqlite::Result<SteelDatasetRecord> {
    let preview_json: String = row.get(10)?;
    Ok(SteelDatasetRecord {
        id: row.get(0)?,
        source_name: row.get(1)?,
        source_path: row.get(2)?,
        source_sha256: row.get(3)?,
        format: row.get(4)?,
        selected_sheet: row.get(5)?,
        row_count: count_value(row.get(6)?, 6)?,
        column_count: count_value(row.get(7)?, 7)?,
        truncated: row.get::<_, i64>(8)? != 0,
        mapping_state: row.get(9)?,
        preview: serde_json::from_str(&preview_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        columns: Vec::new(),
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_column(row: &rusqlite::Row<'_>) -> rusqlite::Result<SteelDatasetColumnRecord> {
    Ok(SteelDatasetColumnRecord {
        ordinal: count_value(row.get(0)?, 0)?,
        original_name: row.get(1)?,
        duplicate: row.get::<_, i64>(2)? != 0,
        inferred_type: row.get(3)?,
        canonical_field: row.get(4)?,
        unit: row.get(5)?,
        non_empty_count: count_value(row.get(6)?, 6)?,
        missing_count: count_value(row.get(7)?, 7)?,
        invalid_count: count_value(row.get(8)?, 8)?,
        min: row.get(9)?,
        max: row.get(10)?,
    })
}

fn count_value(value: i64, column: usize) -> rusqlite::Result<usize> {
    usize::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}
