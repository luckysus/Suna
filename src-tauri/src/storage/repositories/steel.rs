use crate::steel::DatasetPreview;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::collections::HashSet;
use uuid::Uuid;

mod model;

pub use model::{DatasetColumnMapping, SteelDatasetColumnRecord, SteelDatasetRecord};

pub fn save_preview(
    connection: &mut Connection,
    workspace_id: &str,
    source_path: &str,
    source_sha256: &str,
    preview: &DatasetPreview,
    mappings: &[DatasetColumnMapping],
) -> Result<SteelDatasetRecord, String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let result = save_preview_on_connection(
        &transaction,
        workspace_id,
        source_path,
        source_sha256,
        preview,
        mappings,
    )?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(result)
}

pub fn save_preview_on_connection(
    connection: &Connection,
    workspace_id: &str,
    source_path: &str,
    source_sha256: &str,
    preview: &DatasetPreview,
    mappings: &[DatasetColumnMapping],
) -> Result<SteelDatasetRecord, String> {
    save_preview_scoped(
        connection,
        workspace_id,
        source_path,
        source_sha256,
        preview,
        mappings,
        None,
    )
}
pub fn save_preview_scoped(
    connection: &Connection,
    workspace_id: &str,
    source_path: &str,
    source_sha256: &str,
    preview: &DatasetPreview,
    mappings: &[DatasetColumnMapping],
    project: Option<&str>,
) -> Result<SteelDatasetRecord, String> {
    let mappings = normalize_mappings(preview, mappings)?;
    let preview_json = serde_json::to_string(preview).map_err(|error| error.to_string())?;
    let now = Utc::now().to_rfc3339();

    connection
        .execute(
            "INSERT INTO steel_datasets
               (workspace_id, id, source_name, source_path, source_sha256, format,
                selected_sheet, row_count, column_count, truncated, mapping_state,
                preview_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'draft', ?11, ?12, ?12)
             ON CONFLICT(workspace_id, source_sha256, selected_sheet) DO UPDATE SET
               source_name = excluded.source_name,
               source_path = excluded.source_path,
               format = excluded.format,
               row_count = excluded.row_count,
               column_count = excluded.column_count,
               truncated = excluded.truncated,
               mapping_state = 'draft',
               preview_json = excluded.preview_json,
               updated_at = excluded.updated_at",
            params![
                workspace_id,
                Uuid::new_v4().to_string(),
                preview.source_name,
                source_path,
                source_sha256,
                preview.format,
                preview.selected_sheet,
                i64::try_from(preview.row_count).map_err(|_| "dataset row count is too large")?,
                i64::try_from(preview.column_count)
                    .map_err(|_| "dataset column count is too large")?,
                if preview.truncated { 1 } else { 0 },
                preview_json,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;

    let dataset_id: String = connection
        .query_row(
            "SELECT id FROM steel_datasets
             WHERE workspace_id = ?1 AND source_sha256 = ?2 AND selected_sheet = ?3",
            params![workspace_id, source_sha256, preview.selected_sheet],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM steel_dataset_columns
             WHERE workspace_id = ?1 AND dataset_id = ?2",
            params![workspace_id, dataset_id],
        )
        .map_err(|error| error.to_string())?;

    for (ordinal, column) in preview.columns.iter().enumerate() {
        let mapping = mappings.iter().find(|mapping| mapping.ordinal == ordinal);
        connection
            .execute(
                "INSERT INTO steel_dataset_columns
                   (workspace_id, dataset_id, ordinal, original_name, duplicate,
                    inferred_type, canonical_field, unit, non_empty_count, missing_count,
                    invalid_count, min_value, max_value)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    workspace_id,
                    dataset_id,
                    i64::try_from(ordinal).map_err(|_| "dataset column index is too large")?,
                    column.name,
                    if column.duplicate { 1 } else { 0 },
                    column.inferred_type,
                    mapping.and_then(|mapping| mapping.canonical_field.as_deref()),
                    mapping.and_then(|mapping| mapping.unit.as_deref()),
                    i64::try_from(column.non_empty_count)
                        .map_err(|_| "dataset non-empty count is too large")?,
                    i64::try_from(column.missing_count)
                        .map_err(|_| "dataset missing count is too large")?,
                    i64::try_from(column.invalid_count)
                        .map_err(|_| "dataset invalid count is too large")?,
                    column.min,
                    column.max,
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    if let Some(project) = project {
        super::research_projects::require_active(connection, workspace_id, project)?;
        super::research_projects::attach_in_transaction(
            connection,
            workspace_id,
            project,
            "dataset",
            &dataset_id,
            "attached",
        )?;
        super::research_artifacts::dataset_snapshot(
            connection,
            workspace_id,
            project,
            &dataset_id,
        )?;
    } else {
        super::research_artifacts::imported_dataset(&connection, workspace_id, &dataset_id)?;
    }
    get(connection, workspace_id, &dataset_id)?
        .ok_or_else(|| "saved dataset disappeared".to_string())
}

pub fn activate(
    connection: &mut Connection,
    workspace_id: &str,
    dataset_id: &str,
) -> Result<Option<SteelDatasetRecord>, String> {
    activate_version(connection, workspace_id, dataset_id, None)
}

pub(crate) fn activate_version(
    connection: &mut Connection,
    workspace_id: &str,
    dataset_id: &str,
    expected: Option<(&str, &str)>,
) -> Result<Option<SteelDatasetRecord>, String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let version = transaction
        .query_row(
            "SELECT source_sha256, updated_at FROM steel_datasets WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, dataset_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((source_sha256, updated_at)) = version else {
        return Ok(None);
    };
    if expected.is_some_and(|(hash, updated)| hash != source_sha256 || updated != updated_at) {
        return Err("dataset_activation_review_changed".into());
    }

    let has_canonical_field = {
        let mut statement = transaction
            .prepare(
                "SELECT duplicate, canonical_field, unit
                 FROM steel_dataset_columns
                 WHERE workspace_id = ?1 AND dataset_id = ?2
                 ORDER BY ordinal",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params![workspace_id, dataset_id], |row| {
                Ok((
                    row.get::<_, i64>(0)? != 0,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?;
        let mut has_canonical_field = false;
        for row in rows {
            let (duplicate, canonical_field, unit) = row.map_err(|error| error.to_string())?;
            if duplicate {
                return Err(
                    "dataset activation requires duplicate columns to be resolved".to_string(),
                );
            }
            if canonical_field
                .as_deref()
                .is_some_and(|field| !field.trim().is_empty())
            {
                has_canonical_field = true;
            } else if unit
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Err(
                    "dataset activation requires every unit to have a canonical field".to_string(),
                );
            }
        }
        has_canonical_field
    };
    if !has_canonical_field {
        return Err("dataset activation requires at least one canonical field".to_string());
    }

    transaction
        .execute(
            "UPDATE steel_datasets SET mapping_state = 'ready', updated_at = ?3
             WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, dataset_id, Utc::now().to_rfc3339()],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    get(connection, workspace_id, dataset_id)
}

fn normalize_mappings(
    preview: &DatasetPreview,
    mappings: &[DatasetColumnMapping],
) -> Result<Vec<DatasetColumnMapping>, String> {
    let mut ordinals = HashSet::new();
    let mut canonical_fields = HashSet::new();
    let mut normalized = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        if mapping.ordinal >= preview.columns.len() {
            return Err(format!(
                "dataset mapping ordinal {} is out of range",
                mapping.ordinal
            ));
        }
        if !ordinals.insert(mapping.ordinal) {
            return Err(format!(
                "dataset mapping ordinal {} is duplicated",
                mapping.ordinal
            ));
        }
        let canonical_field = mapping
            .canonical_field
            .as_deref()
            .map(str::trim)
            .filter(|field| !field.is_empty())
            .map(str::to_string);
        let unit = mapping
            .unit
            .as_deref()
            .map(str::trim)
            .filter(|unit| !unit.is_empty())
            .map(str::to_string);
        if let Some(field) = canonical_field.as_deref() {
            if !is_canonical_field(field) {
                return Err(format!(
                    "canonical field {field} must use lower snake_case or namespace.field"
                ));
            }
            if !canonical_fields.insert(field.to_ascii_lowercase()) {
                return Err(format!(
                    "dataset canonical field {field} is mapped more than once"
                ));
            }
        }
        if unit.is_some() && canonical_field.is_none() {
            return Err("dataset unit requires a canonical field".to_string());
        }
        if let Some(unit) = unit.as_deref() {
            if unit.chars().count() > 24
                || unit
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace())
            {
                return Err(format!("dataset unit {unit} contains invalid characters"));
            }
        }
        if canonical_field.is_some() || unit.is_some() {
            normalized.push(DatasetColumnMapping {
                ordinal: mapping.ordinal,
                canonical_field,
                unit,
            });
        }
    }
    Ok(normalized)
}

fn is_canonical_field(field: &str) -> bool {
    let mut characters = field.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '_'
                || character == '.'
        })
        && !field.ends_with('_')
        && !field.ends_with('.')
        && !field.contains("..")
        && !field.contains("__")
}

/// Immutable version writers never invoke the legacy import/upsert path for an existing hash.
pub fn save_new_preview_scoped(
    connection: &Connection,
    workspace_id: &str,
    source_path: &str,
    source_sha256: &str,
    preview: &DatasetPreview,
    mappings: &[DatasetColumnMapping],
    project: &str,
) -> Result<SteelDatasetRecord, String> {
    let exists:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM steel_datasets WHERE workspace_id=?1 AND source_sha256=?2 AND selected_sheet=?3)",params![workspace_id,source_sha256,preview.selected_sheet],|r|r.get(0)).map_err(|e|e.to_string())?;
    if exists {
        return Err(
            "identical_dataset_version_already_exists: existing data and mappings were preserved"
                .into(),
        );
    }
    save_preview_scoped(
        connection,
        workspace_id,
        source_path,
        source_sha256,
        preview,
        mappings,
        Some(project),
    )
}

mod queries;
pub use queries::{get, list};
