use crate::db::{current_workspace_id, with_conn, DbState};
use crate::storage::repositories::material_rules as rules;

#[tauri::command]
pub fn get_material_registry(
    db: tauri::State<DbState>,
    project_id: Option<String>,
) -> Result<crate::steel::materials::Registry, String> {
    with_conn(&db, |c| {
        rules::registry(c, current_workspace_id(), project_id.as_deref())
    })
}
#[tauri::command]
pub fn suggest_material_fields(
    db: tauri::State<DbState>,
    project_id: Option<String>,
    names: Vec<String>,
) -> Result<Vec<Vec<crate::steel::materials::Match>>, String> {
    if names.len() > 512 {
        return Err("too_many_columns".into());
    }
    with_conn(&db, |c| {
        let registry = rules::registry(c, current_workspace_id(), project_id.as_deref())?;
        Ok(names
            .iter()
            .map(|name| crate::steel::materials::suggest(&registry, name))
            .collect())
    })
}
#[tauri::command]
pub fn save_material_schema_overlay(
    db: tauri::State<DbState>,
    project_id: String,
    fields: Vec<crate::steel::materials::Field>,
    overrides: Vec<String>,
) -> Result<u32, String> {
    with_conn(&db, |c| {
        rules::save_overlay(c, current_workspace_id(), &project_id, fields, overrides)
    })
}
#[tauri::command]
pub fn list_material_rules(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<Vec<rules::VersionedRule>, String> {
    with_conn(&db, |c| rules::list(c, current_workspace_id(), &project_id))
}
#[tauri::command]
pub fn save_material_rule(
    db: tauri::State<DbState>,
    project_id: String,
    rule: crate::steel::materials::Rule,
    expected_version: u32,
) -> Result<rules::VersionedRule, String> {
    with_conn(&db, |c| {
        rules::save(
            c,
            current_workspace_id(),
            &project_id,
            rule,
            expected_version,
        )
    })
}
#[tauri::command]
pub fn preview_material_rules(
    db: tauri::State<DbState>,
    project_id: String,
    variables: Vec<crate::steel::materials::Variable>,
    selected: Vec<String>,
) -> Result<rules::RuleSnapshot, String> {
    with_conn(&db, |c| {
        rules::compile(c, current_workspace_id(), &project_id, variables, &selected)
    })
}
#[tauri::command]
pub fn normalize_material_dataset(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: rules::NormalizeRequest,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = crate::db::app_data_directory(&app)?;
    with_conn(&db, |c| {
        rules::normalize_dataset(c, current_workspace_id(), &root, request)
    })
}
