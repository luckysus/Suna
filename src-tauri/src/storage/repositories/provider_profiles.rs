use crate::providers::profiles::{
    ProviderCapability, ProviderKind, ProviderProfile, ProviderProfileRecord,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use std::str::FromStr;
use uuid::Uuid;

struct StoredProfile {
    id: String,
    kind: String,
    display_name: String,
    base_url: String,
    model_id: Option<String>,
    secret_ref: Option<String>,
    enabled: bool,
    revision: i64,
    secret_generation: i64,
    model_type: Option<String>,
    source: Option<String>,
    status: Option<String>,
    metadata_json: Option<String>,
}

pub fn infer_model_type(profile: &ProviderProfile) -> String {
    if profile.kind == ProviderKind::MinerU {
        return "document_parser".to_string();
    }
    let model_id = profile
        .model_id
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    if model_id.contains("whisper") || model_id.contains("paraformer") || model_id.contains("asr") {
        return "asr".to_string();
    }
    if model_id.contains("vl-")
        || model_id.contains("-vl")
        || model_id.contains("vision")
        || model_id.contains("llava")
    {
        return "vllm".to_string();
    }
    if model_id.contains("rerank") {
        return "rerank".to_string();
    }
    if model_id.contains("embedding") || model_id.contains("bge-m3") || model_id.contains("embed") {
        return "embedding".to_string();
    }
    "chat".to_string()
}

impl StoredProfile {
    fn into_record(self) -> Result<ProviderProfileRecord, String> {
        let profile = ProviderProfile {
            id: Uuid::parse_str(&self.id)
                .map_err(|error| format!("invalid stored provider profile ID: {error}"))?,
            kind: ProviderKind::from_str(&self.kind)?,
            display_name: self.display_name,
            base_url: self.base_url,
            model_id: self.model_id,
            secret_ref: self.secret_ref,
            enabled: self.enabled,
        };
        let model_type = self
            .model_type
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| infer_model_type(&profile));
        Ok(ProviderProfileRecord {
            profile,
            model_type,
            source: self
                .source
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "remote".to_string()),
            status: self
                .status
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "active".to_string()),
            metadata_json: self
                .metadata_json
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "{}".to_string()),
            revision: u64::try_from(self.revision)
                .map_err(|_| "invalid stored provider profile revision".to_string())?,
            secret_generation: u64::try_from(self.secret_generation)
                .map_err(|_| "invalid stored provider secret generation".to_string())?,
        })
    }
}

fn stored(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredProfile> {
    Ok(StoredProfile {
        id: row.get(0)?,
        kind: row.get(1)?,
        display_name: row.get(2)?,
        base_url: row.get(3)?,
        model_id: row.get(4)?,
        secret_ref: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        revision: row.get(7)?,
        secret_generation: row.get(8)?,
        model_type: row.get(9)?,
        source: row.get(10)?,
        status: row.get(11)?,
        metadata_json: row.get(12)?,
    })
}

pub fn list_records(
    conn: &Connection,
    workspace_id: &str,
) -> Result<Vec<ProviderProfileRecord>, String> {
    let mut statement = conn
        .prepare(
            "SELECT id, kind, display_name, base_url, model_id, secret_ref, enabled,
                    revision, secret_generation, model_type, source, status, metadata_json
             FROM provider_profiles
             WHERE workspace_id = ?1
             ORDER BY enabled DESC, display_name ASC, id ASC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], stored)
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(StoredProfile::into_record)
        .collect()
}

pub fn list(conn: &Connection, workspace_id: &str) -> Result<Vec<ProviderProfile>, String> {
    list_records(conn, workspace_id)
        .map(|records| records.into_iter().map(|record| record.profile).collect())
}

pub fn get_record(
    conn: &Connection,
    workspace_id: &str,
    id: Uuid,
) -> Result<Option<ProviderProfileRecord>, String> {
    conn.query_row(
        "SELECT id, kind, display_name, base_url, model_id, secret_ref, enabled,
                revision, secret_generation, model_type, source, status, metadata_json
         FROM provider_profiles
         WHERE workspace_id = ?1 AND id = ?2",
        params![workspace_id, id.to_string()],
        stored,
    )
    .optional()
    .map_err(|error| error.to_string())?
    .map(StoredProfile::into_record)
    .transpose()
}

pub fn update_status(
    conn: &Connection,
    workspace_id: &str,
    id: Uuid,
    status: &str,
) -> Result<(), String> {
    let updated = conn
        .execute(
            "UPDATE provider_profiles SET status = ?1, updated_at = ?2
             WHERE workspace_id = ?3 AND id = ?4",
            params![
                status,
                Utc::now().to_rfc3339(),
                workspace_id,
                id.to_string()
            ],
        )
        .map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err("provider profile not found".to_string());
    }
    Ok(())
}

pub fn get(
    conn: &Connection,
    workspace_id: &str,
    id: Uuid,
) -> Result<Option<ProviderProfile>, String> {
    get_record(conn, workspace_id, id).map(|record| record.map(|record| record.profile))
}

pub fn list_default_capabilities(
    conn: &Connection,
    workspace_id: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut statement = conn
        .prepare("SELECT capability, profile_id FROM provider_defaults WHERE workspace_id = ?1")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<std::collections::HashMap<_, _>, _>>()
        .map_err(|error| error.to_string())
}

pub fn save_record(
    conn: &Connection,
    workspace_id: &str,
    profile: ProviderProfile,
    model_type: &str,
    source: &str,
    status: &str,
    metadata_json: &str,
) -> Result<ProviderProfileRecord, String> {
    let profile = profile.validate()?;
    let id = profile.id.to_string();
    let owner = conn
        .query_row(
            "SELECT workspace_id FROM provider_profiles WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if owner.as_deref().is_some_and(|owner| owner != workspace_id) {
        return Err("provider profile belongs to another workspace".to_string());
    }

    let timestamp = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO provider_profiles
         (id, workspace_id, kind, display_name, base_url, model_id, secret_ref,
          enabled, created_at, updated_at, model_type, source, status, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(id) DO UPDATE SET
           kind = excluded.kind,
           display_name = excluded.display_name,
           base_url = excluded.base_url,
           model_id = excluded.model_id,
           secret_ref = excluded.secret_ref,
           enabled = excluded.enabled,
           model_type = excluded.model_type,
           source = excluded.source,
           status = excluded.status,
           metadata_json = excluded.metadata_json,
           revision = provider_profiles.revision +
             CASE WHEN provider_profiles.kind IS NOT excluded.kind
                    OR provider_profiles.base_url IS NOT excluded.base_url
                    OR provider_profiles.model_id IS NOT excluded.model_id
                    OR provider_profiles.secret_ref IS NOT excluded.secret_ref
                  THEN 1 ELSE 0 END,
           secret_generation =
             CASE WHEN provider_profiles.secret_ref IS NOT excluded.secret_ref
                  THEN 0 ELSE provider_profiles.secret_generation END,
           updated_at = excluded.updated_at",
        params![
            id,
            workspace_id,
            profile.kind.as_str(),
            profile.display_name,
            profile.base_url,
            profile.model_id,
            profile.secret_ref,
            i64::from(profile.enabled),
            timestamp,
            model_type,
            source,
            status,
            metadata_json
        ],
    )
    .map_err(|error| error.to_string())?;
    get_record(conn, workspace_id, profile.id)?
        .ok_or_else(|| "provider profile not found after save".to_string())
}

pub fn save_record_simple(
    conn: &Connection,
    workspace_id: &str,
    profile: ProviderProfile,
    model_type: &str,
) -> Result<ProviderProfileRecord, String> {
    save_record(
        conn,
        workspace_id,
        profile,
        model_type,
        "remote",
        "active",
        "{}",
    )
}

pub fn save(
    conn: &Connection,
    workspace_id: &str,
    profile: ProviderProfile,
    model_type: &str,
) -> Result<ProviderProfile, String> {
    save_record_simple(conn, workspace_id, profile, model_type).map(|record| record.profile)
}

pub fn activate_secret_generation(
    conn: &Connection,
    workspace_id: &str,
    id: Uuid,
    secret_ref: &str,
    expected_generation: u64,
) -> Result<ProviderProfileRecord, String> {
    let expected = i64::try_from(expected_generation)
        .map_err(|_| "provider secret generation is too large".to_string())?;
    let updated = conn
        .execute(
            "UPDATE provider_profiles
             SET secret_generation = secret_generation + 1,
                 updated_at = ?1
             WHERE workspace_id = ?2 AND id = ?3 AND secret_ref = ?4
               AND secret_generation = ?5",
            params![
                Utc::now().to_rfc3339(),
                workspace_id,
                id.to_string(),
                secret_ref,
                expected,
            ],
        )
        .map_err(|error| error.to_string())?;
    if updated != 1 {
        return Err("provider secret generation conflict".to_string());
    }
    get_record(conn, workspace_id, id)?
        .ok_or_else(|| "provider profile not found after secret activation".to_string())
}

pub fn delete(conn: &mut Connection, workspace_id: &str, id: Uuid) -> Result<(), String> {
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM provider_defaults WHERE workspace_id = ?1 AND profile_id = ?2",
            params![workspace_id, id.to_string()],
        )
        .map_err(|error| error.to_string())?;
    let deleted = transaction
        .execute(
            "DELETE FROM provider_profiles WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id.to_string()],
        )
        .map_err(|error| error.to_string())?;
    if deleted == 0 {
        return Err("provider profile not found".to_string());
    }
    transaction.commit().map_err(|error| error.to_string())
}

pub fn set_default(
    conn: &mut Connection,
    workspace_id: &str,
    capability: ProviderCapability,
    profile_id: Option<Uuid>,
) -> Result<(), String> {
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let Some(profile_id) = profile_id else {
        transaction
            .execute(
                "DELETE FROM provider_defaults WHERE workspace_id = ?1 AND capability = ?2",
                params![workspace_id, capability.as_str()],
            )
            .map_err(|error| error.to_string())?;
        return transaction.commit().map_err(|error| error.to_string());
    };
    let kind = transaction
        .query_row(
            "SELECT kind FROM provider_profiles
             WHERE workspace_id = ?1 AND id = ?2 AND enabled = 1",
            params![workspace_id, profile_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "enabled provider profile not found".to_string())?;
    if !ProviderKind::from_str(&kind)?.supports(capability) {
        return Err("provider does not support this capability".to_string());
    }
    transaction
        .execute(
            "INSERT INTO provider_defaults (workspace_id, capability, profile_id, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(workspace_id, capability)
             DO UPDATE SET profile_id = excluded.profile_id, updated_at = excluded.updated_at",
            params![
                workspace_id,
                capability.as_str(),
                profile_id.to_string(),
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
}

pub fn get_default(
    conn: &Connection,
    workspace_id: &str,
    capability: ProviderCapability,
) -> Result<Option<ProviderProfile>, String> {
    get_default_record(conn, workspace_id, capability)
        .map(|record| record.map(|record| record.profile))
}

pub fn get_default_record(
    conn: &Connection,
    workspace_id: &str,
    capability: ProviderCapability,
) -> Result<Option<ProviderProfileRecord>, String> {
    conn.query_row(
        "SELECT p.id, p.kind, p.display_name, p.base_url, p.model_id, p.secret_ref, p.enabled,
                p.revision, p.secret_generation, p.model_type, p.source, p.status, p.metadata_json
         FROM provider_defaults d
         JOIN provider_profiles p
           ON p.workspace_id = d.workspace_id AND p.id = d.profile_id
         WHERE d.workspace_id = ?1 AND d.capability = ?2 AND p.enabled = 1",
        params![workspace_id, capability.as_str()],
        stored,
    )
    .optional()
    .map_err(|error| error.to_string())?
    .map(StoredProfile::into_record)
    .transpose()
}
