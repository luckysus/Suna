use super::*;

#[tauri::command]
pub async fn create_backup_archive(
    app: tauri::AppHandle,
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        create_backup_blocking(app.clone(), app.state::<DbState>(), archive_path)
    })
    .await
    .map_err(|_| "backup_export_worker_failed")?
}
#[tauri::command]
pub async fn preview_backup_archive(
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || preview_backup_blocking(archive_path))
        .await
        .map_err(|_| "backup_preview_worker_failed")?
}
#[tauri::command]
pub async fn restore_backup_archive(
    app: tauri::AppHandle,
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        restore_backup_blocking(
            app.clone(),
            app.state::<DbState>(),
            app.state::<crate::agent::runtime::RuntimeHost>(),
            app.state::<SchedulerState>(),
            archive_path,
        )
    })
    .await
    .map_err(|_| "backup_restore_worker_failed")?
}

fn create_backup_blocking(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    let archive_path = PathBuf::from(archive_path.trim());
    if archive_path.as_os_str().is_empty() {
        return Err("backup archive path is required".to_string());
    }
    let database = database_path(&app)?;
    let content_root = crate::app::agent_automation::content_root_for(&database)?;
    with_conn(&db, |connection| {
        crate::storage::backup::create_backup(connection, &database, &content_root, &archive_path)
    })
}

fn preview_backup_blocking(
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    let archive_path = PathBuf::from(archive_path.trim());
    if archive_path.as_os_str().is_empty() {
        return Err("backup archive path is required".to_string());
    }
    crate::storage::backup::preview_backup(&archive_path)
}

fn restore_backup_blocking(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    agent_state: tauri::State<crate::agent::runtime::RuntimeHost>,
    scheduler_state: tauri::State<SchedulerState>,
    archive_path: String,
) -> Result<crate::storage::backup::BackupSummary, String> {
    let archive_path = PathBuf::from(archive_path.trim());
    if archive_path.as_os_str().is_empty() {
        return Err("backup archive path is required".to_string());
    }
    crate::app::agent_automation::prepare_restore(&app, &agent_state)?;
    if !scheduler_state.shutdown(Duration::from_secs(10)) {
        crate::app::agent_automation::start_for_database(&app, database_path(&app)?)?;
        return Err("background scheduler did not stop before restore".to_string());
    }
    let connection = {
        let mut guard = db.conn.lock().map_err(|_| "db state poisoned")?;
        guard.take().ok_or("database not initialized")?
    };
    drop(connection);

    let database = database_path(&app)?;
    let content_root = crate::app::agent_automation::content_root_for(&database)?;
    let domains_root = app_data_directory(&app)?.join("domains");
    let result = crate::storage::backup::restore_backup_with_domain_validation(
        &archive_path,
        &database,
        &content_root,
        &domains_root,
        env!("CARGO_PKG_VERSION"),
        &crate::domains::official_trust_store(),
    );
    let reinitialized = db_init(app, db, agent_state, scheduler_state);
    match (result, reinitialized) {
        (Ok(summary), Ok(())) => Ok(summary),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(format!(
            "backup restored but database restart failed: {error}"
        )),
        (Err(error), Err(restart_error)) => {
            Err(format!("{error}; database restart failed: {restart_error}"))
        }
    }
}
