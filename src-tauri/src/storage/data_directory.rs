use crate::diagnostics::{available_disk_bytes, is_link_or_reparse_point};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct DirectoryConfig {
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub previous_directories: Vec<PathBuf>,
    pub pending: Option<PathBuf>,
    pub message: Option<String>,
}

pub struct DataDirectoryState {
    pub(crate) root: PathBuf,
    config_path: PathBuf,
    config: Mutex<DirectoryConfig>,
}

#[derive(Clone, Serialize)]
pub struct DataDirectoryChange {
    pub pending_directory: Option<String>,
    pub message: Option<String>,
    pub environment_override: bool,
}

impl DataDirectoryState {
    pub(crate) fn initialize(app: &tauri::AppHandle) -> Result<Self, String> {
        let config_path = match std::env::var_os("SUNA_STORAGE_CONFIG_FILE") {
            Some(path) => crate::db::configured_data_directory(PathBuf::new(), Some(path.into()))?,
            None => app
                .path()
                .app_config_dir()
                .map_err(|e| e.to_string())?
                .join("data-directory.json"),
        };
        let default = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let mut config = read_config(&config_path)?;
        let environment = std::env::var_os("SUNA_DATA_DIR").map(PathBuf::from);
        let mut root = crate::db::configured_data_directory(
            default,
            environment.clone().or(config.path.clone()),
        )?;
        if environment.is_none() {
            complete_pending(&config_path, &mut config, &mut root)?;
        }
        fs::create_dir_all(&root).map_err(|e| format!("创建数据目录失败: {e}"))?;
        Ok(Self {
            root,
            config_path,
            config: Mutex::new(config),
        })
    }

    pub(crate) fn relocate(&self, path: &Path) -> PathBuf {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        config
            .previous_directories
            .iter()
            .find_map(|previous| relocate_path(path, previous, &self.root))
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn read_config(path: &Path) -> Result<DirectoryConfig, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("数据目录配置损坏: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DirectoryConfig::default()),
        Err(e) => Err(format!("读取数据目录配置失败: {e}")),
    }
}

fn write_config(path: &Path, config: &DirectoryConfig) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("配置目录无效")?).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(config).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temporary, path).map_err(|e| format!("保存数据目录配置失败: {e}"))
}

fn display_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            if let Prefix::VerbatimDisk(drive) = prefix.kind() {
                let mut local = PathBuf::from(format!("{}:\\", drive as char));
                for component in components {
                    if let Component::Normal(name) = component {
                        local.push(name);
                    } else if !matches!(component, Component::RootDir) {
                        return path.to_path_buf();
                    }
                }
                return local;
            }
        }
    }
    path.to_path_buf()
}

fn normalized_directory(path: &Path) -> Result<PathBuf, String> {
    let authorized =
        crate::permissions::path::authorize_output_path(path).map_err(|e| e.to_string())?;
    Ok(display_path(authorized.canonical_path()))
}

fn path_key(path: &Path) -> String {
    let value = path_identity(path).unwrap_or_else(|| path.to_path_buf());
    let value = value.to_string_lossy().into_owned();
    #[cfg(windows)]
    let value = value.replace('\\', "/").to_ascii_lowercase();
    value.trim_end_matches('/').to_string()
}

// Only local filesystem identity is compared here; this is not an authorization
// API. Resolve existing ancestors so short aliases and missing tails agree.
fn path_identity(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut cursor = display_path(path);
    if !cursor.is_absolute()
        || cursor
            .components()
            .any(|c| matches!(c, Component::ParentDir))
    {
        return None;
    }
    #[cfg(windows)]
    if !matches!(cursor.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
        || cursor
            .components()
            .any(|c| matches!(c, Component::Normal(name) if name.to_string_lossy().contains(':')))
    {
        return None;
    }
    let mut missing = Vec::new();
    loop {
        if let Ok(canonical) = fs::canonicalize(&cursor) {
            let mut canonical = display_path(&canonical);
            #[cfg(windows)]
            if !matches!(canonical.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), std::path::Prefix::Disk(_)))
            {
                return None;
            }
            for name in missing.iter().rev() {
                canonical.push(name);
            }
            return Some(canonical);
        }
        missing.push(cursor.file_name()?.to_os_string());
        cursor = cursor.parent()?.to_path_buf();
    }
}

fn relocate_path(path: &Path, source: &Path, target: &Path) -> Option<PathBuf> {
    let path = path_identity(path)?;
    let source = path_identity(source)?;
    let components: Vec<_> = path.components().collect();
    let source_components: Vec<_> = source.components().collect();
    if components.len() < source_components.len() {
        return None;
    }
    for (candidate, root) in components.iter().zip(&source_components) {
        #[cfg(windows)]
        let same = candidate
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&root.as_os_str().to_string_lossy());
        #[cfg(not(windows))]
        let same = candidate == root;
        if !same {
            return None;
        }
    }
    let mut relocated = target.to_path_buf();
    for component in components.iter().skip(source_components.len()) {
        if let std::path::Component::Normal(name) = component {
            relocated.push(name);
        } else {
            return None;
        }
    }
    Some(relocated)
}

fn validate_target(source: &Path, target: &Path) -> Result<PathBuf, String> {
    if let Ok(metadata) = fs::symlink_metadata(target) {
        if is_link_or_reparse_point(&metadata) {
            return Err("目标不能是符号链接或 Windows 联接点".into());
        }
    }
    let target = normalized_directory(target)?;
    let source = normalized_directory(source)?;
    if relocate_path(&target, &source, &source).is_some()
        || relocate_path(&source, &target, &target).is_some()
    {
        return Err("目标目录不能与当前目录相同或互相嵌套".into());
    }
    if target.exists() {
        let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || is_link_or_reparse_point(&metadata) {
            return Err("目标必须是普通目录".into());
        }
        if fs::read_dir(&target)
            .map_err(|e| e.to_string())?
            .next()
            .transpose()
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Err("目标数据目录必须为空，已有数据不会被覆盖".into());
        }
    }
    Ok(target)
}

fn collect_tree(root: &Path, files: &mut Vec<(PathBuf, bool, u64)>) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if is_link_or_reparse_point(&metadata) {
        return Err("数据目录包含符号链接或 Windows 联接点，无法迁移".into());
    }
    files.push((root.to_path_buf(), metadata.is_dir(), metadata.len()));
    if metadata.is_dir() {
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            collect_tree(&entry.map_err(|e| e.to_string())?.path(), files)?;
        }
    } else if !metadata.is_file() {
        return Err("数据目录包含非常规文件，无法迁移".into());
    }
    Ok(())
}

fn digest(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().to_vec())
}

fn relocate_json(value: &mut serde_json::Value, source: &Path, target: &Path) {
    match value {
        serde_json::Value::String(text) => {
            if let Some(path) = relocate_path(Path::new(text), source, target) {
                *text = path.to_string_lossy().into_owned();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                relocate_json(item, source, target);
            }
        }
        serde_json::Value::Object(items) => {
            for item in items.values_mut() {
                relocate_json(item, source, target);
            }
        }
        _ => {}
    }
}

fn relocate_database(
    connection: &mut Connection,
    source: &Path,
    target: &Path,
) -> Result<(), String> {
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let tables = transaction
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let immutable_tables = transaction
        .prepare("SELECT DISTINCT tbl_name FROM sqlite_master WHERE type='trigger' AND upper(sql) LIKE '%BEFORE UPDATE%' AND lower(sql) LIKE '%immutable%'")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?
        .collect::<Result<std::collections::HashSet<_>, _>>().map_err(|e| e.to_string())?;
    for table in tables {
        // Frozen results, reviews and audit payloads retain their original paths
        // and content hashes. Only operational references are relocated.
        if immutable_tables.contains(&table) || table == "settings_audit_log" {
            continue;
        }
        let columns = transaction
            .prepare(&format!(
                "PRAGMA table_info(\"{}\")",
                table.replace('"', "\"\"")
            ))
            .map_err(|e| e.to_string())?
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for column in columns.into_iter().filter(|name| {
            name == "path"
                || name.ends_with("_path")
                || name.ends_with("_directory")
                || name.ends_with("_json")
        }) {
            if column == "title_path" {
                continue;
            }
            let quoted_table = table.replace('"', "\"\"");
            let quoted_column = column.replace('"', "\"\"");
            let values = transaction.prepare(&format!("SELECT DISTINCT \"{quoted_column}\" FROM \"{quoted_table}\" WHERE typeof(\"{quoted_column}\")='text'"))
                .map_err(|e| e.to_string())?.query_map([], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
            for value in values {
                let next = if column.ends_with("_json") {
                    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&value) {
                        let previous = json.clone();
                        relocate_json(&mut json, source, target);
                        if previous == json {
                            continue;
                        }
                        serde_json::to_string(&json).map_err(|e| e.to_string())?
                    } else {
                        continue;
                    }
                } else if let Some(path) = relocate_path(Path::new(&value), source, target) {
                    path.to_string_lossy().into_owned()
                } else {
                    continue;
                };
                transaction.execute(&format!("UPDATE \"{quoted_table}\" SET \"{quoted_column}\"=?1 WHERE \"{quoted_column}\"=?2"), rusqlite::params![next, value]).map_err(|e| e.to_string())?;
            }
        }
    }
    transaction.commit().map_err(|e| e.to_string())
}

fn migrate_tree(source: &Path, target: &Path, config_path: &Path) -> Result<u64, String> {
    let target = validate_target(source, target)?;
    let mut files = Vec::new();
    collect_tree(source, &mut files)?;
    let required = files
        .iter()
        .filter(|(_, directory, _)| !directory)
        .map(|(_, _, size)| size)
        .sum::<u64>();
    if available_disk_bytes(target.parent().ok_or("目标父目录无效")?)?
        .is_some_and(|available| available < required.saturating_add(16 * 1024 * 1024))
    {
        return Err("目标磁盘剩余空间不足，原目录保持不变".into());
    }
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    let mut count = 0;
    for (path, directory, _) in files {
        let relative = path.strip_prefix(source).map_err(|e| e.to_string())?;
        if path_key(&path) == path_key(config_path)
            || matches!(
                relative.to_str(),
                Some("suna.sqlite3" | "suna.sqlite3-wal" | "suna.sqlite3-shm")
            )
        {
            continue;
        }
        let destination = target.join(relative);
        if directory {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            continue;
        }
        let mut input = File::open(&path).map_err(|e| e.to_string())?;
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&destination)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        if digest(&path)? != digest(&destination)? {
            return Err("文件复制校验失败，原目录保持不变".into());
        }
        count += 1;
    }
    // VACUUM INTO includes committed WAL data without copying a stale shm/WAL pair.
    let database = source.join("suna.sqlite3");
    if database.exists() {
        let connection = Connection::open(&database).map_err(|e| e.to_string())?;
        connection
            .execute(
                "VACUUM INTO ?1",
                [target.join("suna.sqlite3").to_string_lossy().as_ref()],
            )
            .map_err(|e| e.to_string())?;
        let mut migrated =
            Connection::open(target.join("suna.sqlite3")).map_err(|e| e.to_string())?;
        relocate_database(&mut migrated, source, &target)?;
        let integrity: String = migrated
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if integrity != "ok" {
            return Err(format!("迁移后的数据库校验失败: {integrity}"));
        }
        count += 1;
    }
    Ok(count)
}

fn complete_pending(
    config_path: &Path,
    config: &mut DirectoryConfig,
    root: &mut PathBuf,
) -> Result<(), String> {
    let Some(target) = config.pending.take() else {
        return Ok(());
    };
    let source = normalized_directory(root)?;
    match migrate_tree(&source, &target, config_path) {
        Ok(count) => {
            let mut next = config.clone();
            next.path = Some(target.clone());
            next.previous_directories.push(source.clone());
            next.message = Some(format!(
                "数据目录已迁移，校验 {count} 个文件。旧目录保留在 {}",
                source.display()
            ));
            match write_config(config_path, &next) {
                Ok(()) => {
                    *config = next;
                    *root = target;
                }
                Err(error) => {
                    config.message = Some(format!(
                        "目录副本已校验，但保存配置失败，继续使用原目录：{error}"
                    ));
                }
            }
        }
        Err(error) => {
            config.message = Some(format!(
                "迁移失败，继续使用原目录：{error}。已复制的目标文件保留，请选择另一个空目录重试。"
            ));
            if let Err(error) = write_config(config_path, config) {
                config.message = Some(format!(
                    "{}；保存迁移结果失败：{error}",
                    config.message.as_deref().unwrap_or_default()
                ));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_data_directory_change(
    state: tauri::State<DataDirectoryState>,
) -> Result<DataDirectoryChange, String> {
    let config = state.config.lock().map_err(|_| "数据目录状态不可用")?;
    Ok(DataDirectoryChange {
        pending_directory: config
            .pending
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        message: config.message.clone(),
        environment_override: std::env::var_os("SUNA_DATA_DIR").is_some(),
    })
}

#[tauri::command]
pub fn change_data_directory(
    state: tauri::State<DataDirectoryState>,
    directory: String,
) -> Result<DataDirectoryChange, String> {
    if std::env::var_os("SUNA_DATA_DIR").is_some() {
        return Err("SUNA_DATA_DIR 环境变量固定了数据目录，请移除该变量后再切换".into());
    }
    let target = validate_target(&state.root, Path::new(directory.trim()))?;
    let mut config = state.config.lock().map_err(|_| "数据目录状态不可用")?;
    let mut next = config.clone();
    next.path = Some(state.root.clone());
    next.pending = Some(target.clone());
    next.message = None;
    write_config(&state.config_path, &next)?;
    *config = next;
    Ok(DataDirectoryChange {
        pending_directory: Some(target.to_string_lossy().into_owned()),
        message: None,
        environment_override: false,
    })
}

#[tauri::command]
pub fn cancel_data_directory_change(state: tauri::State<DataDirectoryState>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|_| "数据目录状态不可用")?;
    let mut next = config.clone();
    next.pending = None;
    write_config(&state.config_path, &next)?;
    *config = next;
    Ok(())
}

#[tauri::command]
pub fn restart_for_data_directory(app: tauri::AppHandle) -> Result<(), String> {
    if !app
        .state::<crate::agent::runtime::RuntimeHost>()
        .active_turns()?
        .is_empty()
    {
        return Err("请等待或取消运行中的 Agent 后再重启".into());
    }
    if !app
        .state::<crate::tasks::scheduler::SchedulerState>()
        .shutdown(std::time::Duration::from_secs(10))
    {
        return Err("后台任务尚未停止，请稍后重试".into());
    }
    app.restart();
}

#[cfg(test)]
#[path = "data_directory_tests.rs"]
mod tests;
