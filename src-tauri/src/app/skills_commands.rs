use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::skills::{self, SkillCatalog};
use crate::storage::repositories::settings;

#[tauri::command]
pub fn list_skills(db: tauri::State<DbState>) -> Result<SkillCatalog, String> {
    with_conn(&db, |connection| {
        skills::catalog(
            connection,
            current_workspace_id(),
            env!("CARGO_PKG_VERSION"),
        )
    })
}

#[tauri::command]
pub fn set_skill_enabled(
    db: tauri::State<DbState>,
    name: String,
    enabled: bool,
) -> Result<SkillCatalog, String> {
    with_conn_mut(&db, |connection| {
        let catalog = skills::set_enabled(
            connection,
            current_workspace_id(),
            &name,
            enabled,
            env!("CARGO_PKG_VERSION"),
        )?;
        settings::record_audit(
            connection,
            current_workspace_id(),
            "skill.update",
            &name,
            "success",
        )?;
        Ok(catalog)
    })
}

// 第 53 章：SkillDraft 自带 camelCase 反序列化，命令层直接接收。

#[tauri::command]
pub fn create_skill(
    db: tauri::State<DbState>,
    draft: skills::SkillDraft,
) -> Result<SkillCatalog, String> {
    with_conn_mut(&db, |connection| {
        let catalog = skills::create_skill(
            connection,
            current_workspace_id(),
            draft,
            env!("CARGO_PKG_VERSION"),
        )?;
        settings::record_audit(
            connection,
            current_workspace_id(),
            "skill.create",
            &catalog
                .skills
                .last()
                .map(|skill| skill.name.clone())
                .unwrap_or_default(),
            "success",
        )?;
        Ok(catalog)
    })
}

#[tauri::command]
pub fn update_skill(
    db: tauri::State<DbState>,
    name: String,
    draft: skills::SkillDraft,
) -> Result<SkillCatalog, String> {
    with_conn_mut(&db, |connection| {
        let catalog = skills::update_skill(
            connection,
            current_workspace_id(),
            &name,
            draft,
            env!("CARGO_PKG_VERSION"),
        )?;
        settings::record_audit(
            connection,
            current_workspace_id(),
            "skill.update",
            &name,
            "success",
        )?;
        Ok(catalog)
    })
}

#[tauri::command]
pub fn duplicate_skill(db: tauri::State<DbState>, name: String) -> Result<SkillCatalog, String> {
    with_conn_mut(&db, |connection| {
        let catalog = skills::duplicate_skill(
            connection,
            current_workspace_id(),
            &name,
            env!("CARGO_PKG_VERSION"),
        )?;
        settings::record_audit(
            connection,
            current_workspace_id(),
            "skill.duplicate",
            &name,
            "success",
        )?;
        Ok(catalog)
    })
}

#[tauri::command]
pub fn delete_skill(db: tauri::State<DbState>, name: String) -> Result<SkillCatalog, String> {
    with_conn_mut(&db, |connection| {
        let catalog = skills::delete_skill(
            connection,
            current_workspace_id(),
            &name,
            env!("CARGO_PKG_VERSION"),
        )?;
        settings::record_audit(
            connection,
            current_workspace_id(),
            "skill.delete",
            &name,
            "success",
        )?;
        Ok(catalog)
    })
}
