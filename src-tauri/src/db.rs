use chrono::Utc;
use rusqlite::Connection;
use std::time::Duration;
use std::{path::PathBuf, sync::Arc, sync::Mutex};
use tauri::Manager;

use crate::tasks::scheduler::SchedulerState;
use crate::tasks::scheduler::TaskHandler;
pub struct DbState {
    conn: Mutex<Option<Connection>>,
}

impl Default for DbState {
    fn default() -> Self {
        Self {
            conn: Mutex::new(None),
        }
    }
}

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn current_workspace_id() -> &'static str {
    crate::app::identity::LocalIdentity.workspace_id()
}

pub(crate) fn with_conn<T>(
    db: &tauri::State<'_, DbState>,
    operation: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = db.conn.lock().map_err(|_| "db state poisoned")?;
    let conn = guard.as_ref().ok_or("database not initialized")?;
    operation(conn)
}

pub(crate) fn with_conn_ref<T>(
    db: &DbState,
    operation: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    let guard = db.conn.lock().map_err(|_| "db state poisoned")?;
    let conn = guard.as_ref().ok_or("database not initialized")?;
    operation(conn)
}

pub(crate) fn with_conn_mut<T>(
    db: &tauri::State<'_, DbState>,
    operation: impl FnOnce(&mut Connection) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = db.conn.lock().map_err(|_| "db state poisoned")?;
    let conn = guard.as_mut().ok_or("database not initialized")?;
    operation(conn)
}

pub(crate) fn configured_data_directory(
    default: PathBuf,
    override_path: Option<PathBuf>,
) -> Result<PathBuf, String> {
    let directory = override_path.unwrap_or(default);
    if directory.as_os_str().is_empty() {
        return Err("SUNA_DATA_DIR must not be empty".to_string());
    }
    if !directory.is_absolute() {
        return Err("SUNA_DATA_DIR must be an absolute path".to_string());
    }
    Ok(directory)
}

pub(crate) fn app_data_directory(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .state::<crate::storage::data_directory::DataDirectoryState>()
        .root
        .clone())
}

pub(crate) fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_directory(app)?.join("suna.sqlite3"))
}

#[tauri::command]
pub fn db_init(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    agent_state: tauri::State<crate::agent::runtime::RuntimeHost>,
    scheduler_state: tauri::State<SchedulerState>,
) -> Result<(), String> {
    let path = database_path(&app)?;
    let (mut connection, _migration_report) =
        crate::storage::database::open(&path).map_err(|error| error.to_string())?;
    crate::storage::repositories::child_turns::interrupt_orphans(
        &mut connection,
        current_workspace_id(),
        Utc::now(),
    )
    .map_err(|error| format!("interrupt orphan child turns failed: {error}"))?;
    let recovered_runs = crate::agent::desktop::recover_active_runs_if_allowed(
        &mut connection,
        current_workspace_id(),
    )?;
    *db.conn.lock().map_err(|_| "db state poisoned")? = Some(connection);
    crate::knowledge_db::auto_initialize_knowledge_database(app.clone());
    let bundled_result = crate::app::bundled_domain::ensure_bundled_steel_package(&app, &db);
    let status_result = with_conn_mut(&db, |connection| {
        let existing = crate::storage::repositories::settings::get(
            connection,
            current_workspace_id(),
            "onboarding.completed",
        )?;
        let status = bundled_result.as_ref().map(|_| ()).map_err(String::as_str);
        let value =
            crate::app::bundled_domain::merge_bundled_steel_status(existing.as_deref(), status)?;
        crate::storage::repositories::settings::set(
            connection,
            current_workspace_id(),
            "onboarding.completed",
            &value,
        )
    });
    if let Err(error) = status_result {
        log::error!("persist bundled steel domain status failed: {error}");
    }
    if let Err(error) = bundled_result {
        log::error!("ensure bundled steel domain package failed: {error}");
    }
    if let Some(seed_result) = crate::skills::bundled::seed_bundled_user_skills() {
        match seed_result {
            Ok(count) if count > 0 => {
                log::info!("seeded {count} bundled skills into the user skill directory")
            }
            Ok(_) => {}
            Err(error) => log::error!("seed bundled skills failed: {error}"),
        }
    }
    // Start scheduler with Tauri event sink for real progress updates
    use crate::app::event_sink::TauriEventSink;
    use crate::tasks::scheduler::{Scheduler, SchedulerConfig, SystemClock};
    let sink = Arc::new(TauriEventSink::new(app.clone()));
    let content_root = crate::app::agent_automation::content_root_for(&path)?;
    let scheduler = Scheduler::new(
        path.clone(),
        current_workspace_id().to_string(),
        SchedulerConfig::default(),
        Arc::new(SystemClock),
        rag_task_handlers_with_compute(path.clone(), content_root, compute_worker_config(&app)),
        sink,
    )
    .map_err(|error| error.to_string())?;
    scheduler_state
        .start(scheduler)
        .map_err(|error| error.to_string())?;
    let runtime_host = agent_state.inner().clone();
    let app_for_recovery = app.clone();
    for recovered in recovered_runs {
        if recovered.events.is_empty() && runtime_host.snapshot(recovered.run.id).is_ok() {
            continue;
        }
        let waits = match &recovered.action {
            crate::agent::runtime::RecoveryAction::AwaitPermissions(_) => {
                match crate::app::desktop_agent_runtime::restore_recovered_permissions(
                    &runtime_host,
                    &recovered,
                ) {
                    Ok(waits) => waits,
                    Err(error) => {
                        log::error!("restore agent permissions after startup failed: {error}");
                        None
                    }
                }
            }
            _ => None,
        };
        let should_resume = matches!(
            &recovered.action,
            crate::agent::runtime::RecoveryAction::ResumeFromCheckpoint(_)
                | crate::agent::runtime::RecoveryAction::ResumeTools(_)
                | crate::agent::runtime::RecoveryAction::AwaitPermissions(_)
        );
        if should_resume
            && (!matches!(
                &recovered.action,
                crate::agent::runtime::RecoveryAction::AwaitPermissions(_)
            ) || waits.is_some())
        {
            let app_for_run = app_for_recovery.clone();
            let runtime_host_for_run = runtime_host.clone();
            tauri::async_runtime::spawn(async move {
                let result = if let Some(waits) = waits {
                    crate::app::desktop_agent_runtime::resume_recovered_agent_with_permissions(
                        &app_for_run,
                        &runtime_host_for_run,
                        current_workspace_id(),
                        recovered,
                        waits,
                    )
                    .await
                } else {
                    crate::app::desktop_agent_runtime::resume_recovered_agent(
                        &app_for_run,
                        &runtime_host_for_run,
                        current_workspace_id(),
                        recovered,
                    )
                    .await
                };
                if let Err(error) = result {
                    log::error!("resume agent run after startup failed: {error}");
                }
            });
        }
    }
    crate::app::agent_automation::start_for_database(&app, path)?;
    Ok(())
}

pub(crate) mod backup_commands;

fn rag_task_handlers_with_compute(
    database: PathBuf,
    content_root: PathBuf,
    compute_worker: Option<crate::compute::worker::WorkerConfig>,
) -> Vec<Arc<dyn TaskHandler>> {
    use crate::rag::index::rebuild::IndexRebuildHandler;
    use crate::rag::tasks::{LocalRagPostprocessor, MinerUTaskHandler, RuntimeProviderFactory};
    use crate::storage::secrets::KeyringSecretStore;

    let providers = Arc::new(RuntimeProviderFactory::new(
        database.clone(),
        Arc::new(KeyringSecretStore),
    ));
    let embedding_factory: Arc<dyn crate::rag::index::EmbeddingRemoteFactory> = providers.clone();
    let remote_factory: Arc<dyn crate::rag::tasks::MinerURemoteFactory> = providers;
    let postprocessor = Arc::new(LocalRagPostprocessor::new(
        database.clone(),
        content_root.clone(),
        embedding_factory,
    ));
    let mut handlers = crate::compute::handler::compute_task_handlers(compute_worker);
    let domain_handlers: Vec<Arc<dyn TaskHandler>> = vec![
        Arc::new(crate::research_workflow::runtime::ModelingWorkflowHandler::new(database.clone())),
        Arc::new(crate::app::literature_discovery::MetadataSearchHandler::new(database.clone())),
        Arc::new(MinerUTaskHandler::new(
            content_root.clone(),
            remote_factory,
            postprocessor,
            std::time::Duration::from_secs(2),
        )),
        Arc::new(IndexRebuildHandler::new(database.clone(), content_root)),
        Arc::new(crate::database::query_task::DatabaseQueryTaskHandler::new(
            database,
        )),
    ];
    handlers.extend(domain_handlers);
    handlers
}

pub(crate) fn compute_worker_config(
    app: &tauri::AppHandle,
) -> Option<crate::compute::worker::WorkerConfig> {
    let resource_worker = app
        .path()
        .resource_dir()
        .ok()
        .map(|directory| {
            directory
                .join("compute-worker")
                .join("suna-compute-worker.exe")
        })
        .filter(|path| path.is_file());
    if let Some(executable) = resource_worker {
        let manifest = executable.with_file_name("worker-artifact-manifest.json");
        return Some(
            crate::compute::worker::WorkerConfig::new(executable)
                .with_artifact_manifest(manifest)
                .with_process_tree_isolation(),
        );
    }

    let executable = std::env::var_os("SUNA_COMPUTE_WORKER_PYTHON")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_file())?;
    let working_directory = std::env::var_os("SUNA_COMPUTE_WORKER_DIR")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_dir())?;
    Some(python_worker_config(executable, working_directory))
}

fn python_worker_config(
    executable: PathBuf,
    working_directory: PathBuf,
) -> crate::compute::worker::WorkerConfig {
    let mut config =
        crate::compute::worker::WorkerConfig::new(executable).with_process_tree_isolation();
    config.args = vec!["-m".into(), "suna_worker".into()];
    config.working_directory = Some(working_directory);
    config
}

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
