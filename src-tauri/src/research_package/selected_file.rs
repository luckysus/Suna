//! Host-selected file only. IPC must obtain the path from the native picker.
use crate::research_package::{archive, digest, ValidatedPackage, MAX_TOTAL};
use std::{io::Read, path::Path};
pub struct SelectedPackage {
    pub bytes_sha256: String,
    pub bytes: usize,
    pub package: ValidatedPackage,
}
pub(crate) fn read_bytes(path: &Path, max_total: usize) -> Result<Vec<u8>, String> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_none_or(|s| !s.eq_ignore_ascii_case("zip"))
    {
        return Err("package_selected_file_type_invalid".into());
    }
    let original =
        std::fs::symlink_metadata(path).map_err(|_| "package_selected_file_unavailable")?;
    if original.file_type().is_symlink() || !original.is_file() {
        return Err("package_selected_file_not_regular".into());
    }
    let (_, mut file) = crate::permissions::path::authorize_existing_file_with_handle(path)
        .map_err(|_| "package_selected_path_not_authorized")?;
    let before = file
        .metadata()
        .map_err(|_| "package_selected_file_unavailable")?;
    if before.len() > max_total as u64 {
        return Err("package_size_budget".into());
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_total as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "package_selected_file_read_failed")?;
    let after = file
        .metadata()
        .map_err(|_| "package_selected_file_unavailable")?;
    if bytes.len() != before.len() as usize
        || bytes.len() > max_total
        || after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
    {
        return Err("package_selected_file_changed".into());
    }
    Ok(bytes)
}
pub fn read(path: &Path) -> Result<SelectedPackage, String> {
    let bytes = read_bytes(path, MAX_TOTAL)?;
    Ok(SelectedPackage {
        bytes_sha256: digest(&bytes),
        bytes: bytes.len(),
        package: archive::inspect(&bytes)?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_selected_path_reads_verified_bytes_and_preserves_the_source_file() {
        let root = std::env::temp_dir().join(format!("suna-中文科研包-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("中文 合成项目.zip");
        let bytes = crate::research_package::archive::export(
            crate::research_package::projection::Projection {
                project_id: uuid::Uuid::new_v4().to_string(),
                records: vec![],
                dependencies: vec![],
                edges: vec![],
                app_version: "1.0.0".into(),
            },
        );
        assert!(bytes.is_err());
        let id = uuid::Uuid::new_v4().to_string();
        let bytes = crate::research_package::archive::export(
            crate::research_package::projection::Projection {
                project_id: id.clone(),
                records: vec![serde_json::json!({"id":id,"kind":"project","name":"合成中文项目"})],
                dependencies: vec![],
                edges: vec![],
                app_version: "1.0.0".into(),
            },
        )
        .unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let result = read(&path).unwrap();
        assert_eq!(result.bytes_sha256, digest(&bytes));
        assert_eq!(result.bytes, bytes.len());
        assert_eq!(result.package.summary()["metadataRecordCount"], 1);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(read(&root.join("missing.zip")).is_err());
        assert!(read(&root.join("private.sqlite3")).is_err());
        drop(result);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn selected_symlinks_and_sparse_oversized_files_are_rejected() {
        let root = std::env::temp_dir().join(format!("suna-package-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("source.zip");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_TOTAL as u64 + 1).unwrap();
        drop(file);
        assert!(read(&path).is_err());
        let link = root.join("link.zip");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(read(&link).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
