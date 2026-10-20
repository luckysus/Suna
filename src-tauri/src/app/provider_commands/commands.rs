use super::catalog::list_providers;
use super::credentials::{
    delete_auxiliary_secret, list_credential_fields, set_auxiliary_secret, CredentialFieldView,
};
use super::debug_profile::{debug_provider, ProviderDebugInput, ProviderDebugResult};
use super::logic::{
    copy_profile, delete_profile, list_profile_responses, probe_provider, profile_credential,
    save_profile, set_default_profile, ProviderProbeResponse, ProviderProfileInput,
    ProviderProfileResponse,
};
use super::ollama_admin::{
    check_status, download_model_with_events, list_models, OllamaModelTag, OllamaStatus,
};
use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::providers::profiles::ProviderCapability;
use crate::storage::repositories::provider_profiles;
use crate::storage::secrets::{SecretState, SecretValue};
use uuid::Uuid;

#[tauri::command]
pub fn list_provider_profiles(
    db: tauri::State<DbState>,
    secrets: tauri::State<SecretState>,
) -> Result<Vec<ProviderProfileResponse>, String> {
    with_conn(&db, |connection| {
        list_profile_responses(connection, current_workspace_id(), secrets.store())
    })
}

#[tauri::command]
pub fn save_provider_profile(
    db: tauri::State<DbState>,
    secrets: tauri::State<SecretState>,
    profile: ProviderProfileInput,
) -> Result<ProviderProfileResponse, String> {
    with_conn_mut(&db, |connection| {
        save_profile(connection, current_workspace_id(), secrets.store(), profile)
    })
}

#[tauri::command]
pub fn set_default_provider_profile(
    db: tauri::State<DbState>,
    capability: ProviderCapability,
    profile_id: Option<String>,
) -> Result<(), String> {
    let profile_id = profile_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|error| format!("invalid provider profile ID: {error}"))?;
    with_conn_mut(&db, |connection| {
        set_default_profile(connection, current_workspace_id(), capability, profile_id)
    })
}

#[tauri::command]
pub async fn test_provider_profile(
    db: tauri::State<'_, DbState>,
    secrets: tauri::State<'_, SecretState>,
    id: String,
    capability: Option<ProviderCapability>,
) -> Result<ProviderProbeResponse, String> {
    let id =
        Uuid::parse_str(&id).map_err(|error| format!("invalid provider profile ID: {error}"))?;
    let record = with_conn(&db, |connection| {
        provider_profiles::get_record(connection, current_workspace_id(), id)?
            .ok_or_else(|| "provider profile not found".to_string())
    })?;
    let credential =
        profile_credential(&record.profile, record.secret_generation, secrets.store())?;
    Ok(probe_provider(record.profile, credential, capability).await)
}

#[tauri::command]
pub fn copy_provider_profile(
    db: tauri::State<DbState>,
    secrets: tauri::State<SecretState>,
    id: String,
    display_name: Option<String>,
) -> Result<ProviderProfileResponse, String> {
    let id =
        Uuid::parse_str(&id).map_err(|error| format!("invalid provider profile ID: {error}"))?;
    with_conn_mut(&db, |connection| {
        copy_profile(
            connection,
            current_workspace_id(),
            secrets.store(),
            id,
            display_name,
        )
    })
}

#[tauri::command]
pub fn list_model_provider_catalog(
    model_type: Option<String>,
) -> Result<Vec<super::catalog::CatalogProvider>, String> {
    list_providers(model_type.as_deref())
}

#[tauri::command]
pub fn resolve_model_catalog(
    provider_id: Option<String>,
    model_id: String,
    model_type: String,
) -> Result<serde_json::Value, String> {
    super::catalog::resolve_capabilities(provider_id.as_deref(), model_id.trim(), model_type.trim())
}

#[tauri::command]
pub fn list_ollama_download_tasks() -> Vec<super::ollama_admin::OllamaDownloadTaskRecord> {
    super::ollama_admin::list_tasks()
}

#[tauri::command]
pub async fn check_ollama_status(base_url: Option<String>) -> OllamaStatus {
    check_status(base_url).await
}

#[tauri::command]
pub async fn list_ollama_models(base_url: Option<String>) -> Result<Vec<OllamaModelTag>, String> {
    list_models(base_url).await
}

#[tauri::command]
pub async fn download_ollama_model(
    app: tauri::AppHandle,
    base_url: Option<String>,
    model_name: String,
) -> Result<(), String> {
    download_model_with_events(app, base_url, model_name).await
}

#[tauri::command]
pub fn list_provider_credential_fields(
    db: tauri::State<DbState>,
    secrets: tauri::State<SecretState>,
    profile_id: String,
) -> Result<Vec<CredentialFieldView>, String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|error| format!("invalid provider profile ID: {error}"))?;
    with_conn(&db, |connection| {
        list_credential_fields(connection, current_workspace_id(), secrets.store(), id)
    })
}

#[tauri::command]
pub fn set_provider_auxiliary_secret(
    secrets: tauri::State<SecretState>,
    profile_id: String,
    credential_name: String,
    value: String,
) -> Result<crate::storage::secrets::SecretStatus, String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|error| format!("invalid provider profile ID: {error}"))?;
    if credential_name.trim() == "api_key" {
        return Err("use secret_set for api_key".to_string());
    }
    let value = SecretValue::new(value).map_err(|error| error.to_string())?;
    set_auxiliary_secret(secrets.store(), id, credential_name.trim(), value)
}

#[tauri::command]
pub fn delete_provider_auxiliary_secret(
    secrets: tauri::State<SecretState>,
    profile_id: String,
    credential_name: String,
) -> Result<crate::storage::secrets::SecretStatus, String> {
    let id = Uuid::parse_str(&profile_id)
        .map_err(|error| format!("invalid provider profile ID: {error}"))?;
    if credential_name.trim() == "api_key" {
        return Err("use secret_delete for api_key".to_string());
    }
    delete_auxiliary_secret(secrets.store(), id, credential_name.trim())
}

#[tauri::command]
pub async fn debug_provider_profile(
    db: tauri::State<'_, DbState>,
    secrets: tauri::State<'_, SecretState>,
    payload: ProviderDebugInput,
) -> Result<ProviderDebugResult, String> {
    let id = Uuid::parse_str(payload.profile_id.trim())
        .map_err(|error| format!("invalid provider profile ID: {error}"))?;
    let record = with_conn(&db, |connection| {
        provider_profiles::get_record(connection, current_workspace_id(), id)?
            .ok_or_else(|| "provider profile not found".to_string())
    })?;
    let credential =
        profile_credential(&record.profile, record.secret_generation, secrets.store())?;
    Ok(debug_provider(record, payload, credential).await)
}

#[tauri::command]
pub fn delete_provider_profile(
    db: tauri::State<DbState>,
    secrets: tauri::State<SecretState>,
    id: String,
) -> Result<(), String> {
    let id =
        Uuid::parse_str(&id).map_err(|error| format!("invalid provider profile ID: {error}"))?;
    with_conn_mut(&db, |connection| {
        delete_profile(connection, current_workspace_id(), secrets.store(), id)
    })
}
