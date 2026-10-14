use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SteelModelRecord {
    pub id: String,
    pub lineage_id: String,
    pub kind: String,
    pub version: i64,
    pub source_task_id: Option<String>,
    pub model_sha256: String,
    pub manifest_json: String,
    pub artifact_json: Option<String>,
    pub model_base64: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

pub struct NewSteelModel<'a> {
    pub lineage_id: &'a str,
    pub kind: &'a str,
    pub source_task_id: Option<&'a str>,
    pub model_sha256: &'a str,
    pub manifest_json: &'a str,
    pub artifact_json: Option<&'a str>,
    pub model_base64: Option<&'a str>,
}

/// Trusted callers may fence registration inside its existing transaction.
/// This is not deserialized from desktop or Agent arguments.
pub struct RegistrationPolicy<'a> {
    pub activate_first: bool,
    pub guard: Option<&'a dyn Fn(&Connection) -> Result<(), String>>,
}
impl Default for RegistrationPolicy<'_> {
    fn default() -> Self {
        Self {
            activate_first: true,
            guard: None,
        }
    }
}

const SELECT_MODEL: &str = "SELECT id, lineage_id, kind, version, source_task_id,
         model_sha256, manifest_json, artifact_json, model_base64, is_active, created_at
         FROM steel_models WHERE workspace_id = ?1 AND id = ?2";

fn row_to_model(row: &rusqlite::Row<'_>) -> rusqlite::Result<SteelModelRecord> {
    Ok(SteelModelRecord {
        id: row.get(0)?,
        lineage_id: row.get(1)?,
        kind: row.get(2)?,
        version: row.get(3)?,
        source_task_id: row.get(4)?,
        model_sha256: row.get(5)?,
        manifest_json: row.get(6)?,
        artifact_json: row.get(7)?,
        model_base64: row.get(8)?,
        is_active: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
    })
}

pub fn create(
    connection: &mut Connection,
    workspace_id: &str,
    model: NewSteelModel<'_>,
) -> Result<SteelModelRecord, String> {
    create_inner(
        connection,
        workspace_id,
        model,
        None,
        RegistrationPolicy::default(),
    )
}

/// Register an exact artifact with its scientific metadata in one transaction.
/// Old create callers/data retain their existing behavior.
pub fn create_with_scientific_cards(
    connection: &mut Connection,
    workspace_id: &str,
    model: NewSteelModel<'_>,
    context: &serde_json::Value,
) -> Result<SteelModelRecord, String> {
    create_with_scientific_cards_with_policy(
        connection,
        workspace_id,
        model,
        context,
        RegistrationPolicy::default(),
    )
}

pub fn create_with_scientific_cards_with_policy(
    connection: &mut Connection,
    workspace_id: &str,
    model: NewSteelModel<'_>,
    context: &serde_json::Value,
    policy: RegistrationPolicy<'_>,
) -> Result<SteelModelRecord, String> {
    create_inner(connection, workspace_id, model, Some(context), policy)
}

fn create_inner(
    connection: &mut Connection,
    workspace_id: &str,
    model: NewSteelModel<'_>,
    context: Option<&serde_json::Value>,
    policy: RegistrationPolicy<'_>,
) -> Result<SteelModelRecord, String> {
    if model.lineage_id.trim().is_empty() {
        return Err("model lineage is required".to_string());
    }
    if model.kind != "linear_artifact"
        && model.kind != "sklearn_artifact"
        && model.kind != "transformer_artifact"
        && model.kind != "onnx"
    {
        return Err(
            "model kind must be linear_artifact, sklearn_artifact, transformer_artifact, or onnx"
                .to_string(),
        );
    }
    if model.model_sha256.len() != 64
        || !model
            .model_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("model sha256 is invalid".to_string());
    }
    let manifest: serde_json::Value = serde_json::from_str(model.manifest_json)
        .map_err(|error| format!("model manifest is invalid: {error}"))?;
    if !manifest.is_object() {
        return Err("model manifest must be an object".to_string());
    }
    match model.kind {
        "linear_artifact" if model.artifact_json.is_none() || model.model_base64.is_some() => {
            return Err("linear model versions must store an artifact and no blob".to_string());
        }
        "sklearn_artifact" if model.artifact_json.is_none() || model.model_base64.is_some() => {
            return Err("sklearn model versions must store an artifact and no blob".to_string());
        }
        "transformer_artifact" if model.artifact_json.is_none() || model.model_base64.is_some() => {
            return Err(
                "transformer model versions must store an artifact and no blob".to_string(),
            );
        }
        "onnx" if model.model_base64.is_none() || model.artifact_json.is_some() => {
            return Err("onnx model versions must store a blob and no artifact".to_string());
        }
        _ => {}
    }

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    if let Some(guard) = policy.guard {
        guard(&transaction)?;
    }
    let version: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM steel_models
             WHERE workspace_id = ?1 AND lineage_id = ?2",
            params![workspace_id, model.lineage_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let is_active = i64::from(version == 1 && policy.activate_first);
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let manifest_json = if let Some(context) = context {
        let mut context = context.clone();
        if !context.is_object() {
            return Err("model_card_invalid_context".into());
        }
        context["id"] = serde_json::json!(id);
        context["version"] = serde_json::json!(version);
        context["source_task_id"] = serde_json::json!(model.source_task_id);
        crate::steel::model_cards::registration_manifest(
            model
                .artifact_json
                .ok_or("model_card_training_artifact_required")?,
            model.model_sha256,
            model.manifest_json,
            &context,
        )
        .map_err(str::to_owned)?
        .to_string()
    } else {
        model.manifest_json.to_owned()
    };
    // Card preparation can take time. Recheck the deadline before any insert.
    if let Some(guard) = policy.guard {
        guard(&transaction)?;
    }
    transaction
        .execute(
            "INSERT INTO steel_models
               (workspace_id, id, lineage_id, kind, version, source_task_id,
                model_sha256, manifest_json, artifact_json, model_base64, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                workspace_id,
                id,
                model.lineage_id,
                model.kind,
                version,
                model.source_task_id,
                model.model_sha256,
                manifest_json,
                model.artifact_json,
                model.model_base64,
                is_active,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    super::research_artifacts::registered_model(&transaction, workspace_id, &id)?;
    transaction.commit().map_err(|error| error.to_string())?;
    get(connection, workspace_id, &id)?
        .ok_or_else(|| "created model version could not be read back".to_string())
}

/// Read the same persisted card for UI/report/Agent, with artifact/SHA checks.
/// Legacy registration remains read-only and explicitly derived.
pub fn scientific_cards(
    connection: &Connection,
    workspace_id: &str,
    id: &str,
) -> Result<serde_json::Value, String> {
    let model = get(connection, workspace_id, id)?.ok_or("model_not_found")?;
    scientific_cards_from_record(&model)
}

pub fn scientific_cards_from_record(model: &SteelModelRecord) -> Result<serde_json::Value, String> {
    let artifact = model
        .artifact_json
        .as_deref()
        .ok_or("model_card_training_artifact_required")?;
    let manifest: serde_json::Value =
        serde_json::from_str(&model.manifest_json).map_err(|_| "model_card_invalid_manifest")?;
    let data = &manifest["scientific_cards"]["data_card"];
    // Only a bounded metadata projection supplies context; never raw source rows.
    let context = serde_json::json!({"id":model.id,"version":model.version,"source_task_id":model.source_task_id,
        "dataset_id":data["dataset_id"],"filtered_row_count":data["filtered_row_count"],
        "valid_row_count":data["valid_row_count"],"source_row_count":data["source_row_count"],
        "filter_policy":data["filter_policy"],"grouping_field":data["grouping_field"],"time_field":data["time_field"]});
    crate::steel::model_cards::read_registered_cards(
        artifact,
        &model.model_sha256,
        &model.manifest_json,
        &context,
    )
    .map_err(str::to_owned)
}

pub fn get(
    connection: &Connection,
    workspace_id: &str,
    id: &str,
) -> Result<Option<SteelModelRecord>, String> {
    connection
        .query_row(SELECT_MODEL, params![workspace_id, id], row_to_model)
        .optional()
        .map_err(|error| error.to_string())
}

pub fn list(
    connection: &Connection,
    workspace_id: &str,
    lineage_id: &str,
) -> Result<Vec<SteelModelRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, lineage_id, kind, version, source_task_id,
                    model_sha256, manifest_json, artifact_json, model_base64, is_active, created_at
             FROM steel_models
             WHERE workspace_id = ?1 AND lineage_id = ?2
             ORDER BY version DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id, lineage_id], row_to_model)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn list_all(
    connection: &Connection,
    workspace_id: &str,
) -> Result<Vec<SteelModelRecord>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, lineage_id, kind, version, source_task_id,
                    model_sha256, manifest_json, artifact_json, model_base64, is_active, created_at
             FROM steel_models
             WHERE workspace_id = ?1
             ORDER BY lineage_id, version DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], row_to_model)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn set_active(
    connection: &mut Connection,
    workspace_id: &str,
    id: &str,
) -> Result<SteelModelRecord, String> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let target = transaction
        .query_row(SELECT_MODEL, params![workspace_id, id], row_to_model)
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "model version was not found".to_string())?;
    transaction
        .execute(
            "UPDATE steel_models SET is_active = 0
             WHERE workspace_id = ?1 AND lineage_id = ?2",
            params![workspace_id, target.lineage_id],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "UPDATE steel_models SET is_active = 1 WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    get(connection, workspace_id, id)?
        .ok_or_else(|| "activated model version could not be read back".to_string())
}

/// Release rule: the active model version of a lineage cannot be deleted.
pub fn delete(connection: &mut Connection, workspace_id: &str, id: &str) -> Result<(), String> {
    let target = get(connection, workspace_id, id)?
        .ok_or_else(|| "model version was not found".to_string())?;
    if target.is_active {
        return Err("active model versions cannot be deleted".to_string());
    }
    connection
        .execute(
            "DELETE FROM steel_models WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "steel_models/tests.rs"]
mod tests;
