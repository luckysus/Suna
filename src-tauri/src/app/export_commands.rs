use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReceipt {
    pub path: String,
    pub bytes: usize,
}

// The native dialog owns path selection. The webview cannot submit an arbitrary
// absolute destination to this command. Cancellation returns None.
#[tauri::command]
pub async fn save_export_file(
    app: tauri::AppHandle,
    filename: String,
    bytes: Vec<u8>,
) -> Result<Option<ExportReceipt>, String> {
    tauri::async_runtime::spawn_blocking(move || save_selected_file(&app, &filename, &bytes))
        .await
        .map_err(|e| e.to_string())?
}

pub(crate) fn save_selected_file(
    app: &tauri::AppHandle,
    filename: &str,
    bytes: &[u8],
) -> Result<Option<ExportReceipt>, String> {
    if filename.is_empty() || filename.contains(['/', '\\', ':']) || bytes.len() > 64 * 1024 * 1024
    {
        return Err("invalid export name or size (maximum 64 MiB)".into());
    }
    let Some(selected) = app
        .dialog()
        .file()
        .set_file_name(filename)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = selected.into_path().map_err(|e| e.to_string())?;
    if path.exists()
        && !app
            .dialog()
            .message("文件已存在，确认覆盖？")
            .title("导出文件")
            .buttons(MessageDialogButtons::OkCancel)
            .blocking_show()
    {
        return Ok(None);
    }
    write_verified_export(&path, bytes)?;
    Ok(Some(ExportReceipt {
        path: path.to_string_lossy().into_owned(),
        bytes: bytes.len(),
    }))
}

pub fn write_verified_export(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let roots = crate::permissions::path::AuthorizedRoots::new(vec![path
        .parent()
        .ok_or("export parent required")?
        .to_path_buf()])
    .map_err(|e| e.to_string())?;
    let authorized = roots.authorize(path).map_err(|e| e.to_string())?;
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() || !meta.is_file() {
            return Err("export target must be a regular file".into());
        }
    }
    let temporary = authorized
        .local_path()
        .with_file_name(format!(".suna-export-{}.tmp", uuid::Uuid::new_v4()));
    let temporary_authorized = roots.authorize(&temporary).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| format!("创建导出临时文件失败：{e}"))?;
    let result = (|| {
        #[cfg(windows)]
        roots
            .verify_opened_file(&temporary_authorized, &file)
            .map_err(|e| e.to_string())?;
        roots
            .verify_target(&temporary_authorized, &temporary)
            .map_err(|e| e.to_string())?;
        roots
            .verify_target(&authorized, path)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("写入导出文件失败：{e}"))?;
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut saved = Vec::new();
        file.read_to_end(&mut saved).map_err(|e| e.to_string())?;
        if saved != bytes {
            return Err("导出内容校验失败".into());
        }
        #[cfg(windows)]
        roots
            .verify_opened_file(&temporary_authorized, &file)
            .map_err(|e| e.to_string())?;
        roots
            .verify_target(&authorized, path)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    drop(file);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, authorized.canonical_path()) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("保存导出文件失败：{error}"));
    }
    if !authorized.canonical_path().is_file()
        || fs::read(authorized.canonical_path()).map_err(|e| e.to_string())? != bytes
    {
        return Err("导出文件校验失败".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_writes_selected_unicode_path_and_truncates_on_overwrite() {
        let dir = std::env::temp_dir().join(format!("suna-export-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("研究结果.json");
        write_verified_export(&path, b"123456").unwrap();
        write_verified_export(&path, b"{}").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"{}");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn relative_destinations_are_rejected() {
        assert!(write_verified_export(Path::new("report.json"), b"{}").is_err());
    }
}
