use super::{DatasetTable, MAX_PREVIEW_COLUMNS, MAX_PREVIEW_ROWS};
use serde_json::Value;
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};
/// Object-array JSON retains nested provenance as JSON cells, never executes formulas.
pub(super) fn read_dataset_table(
    file: File,
    path: &Path,
    sheet: Option<&str>,
) -> Result<DatasetTable, String> {
    let mut bytes = vec![];
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("json_dataset_byte_limit".into());
    }
    read_dataset_table_from_bytes(&bytes, path, sheet)
}

pub(super) fn read_dataset_table_from_bytes(
    bytes: &[u8],
    path: &Path,
    sheet: Option<&str>,
) -> Result<DatasetTable, String> {
    if sheet.is_some_and(|s| s != "JSON") {
        return Err("requested dataset sheet was not found".into());
    }
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("json_dataset_byte_limit".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid_json_dataset")?;
    let all = value
        .as_array()
        .or_else(|| value["rows"].as_array())
        .ok_or("json_dataset_requires_object_array")?;
    if all.is_empty() || all.iter().any(|r| !r.is_object()) {
        return Err("json_dataset_requires_object_array".into());
    }
    let headers = all
        .iter()
        .flat_map(|r| r.as_object().unwrap().keys().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if headers.is_empty() || headers.len() > MAX_PREVIEW_COLUMNS {
        return Err("json_dataset_column_limit".into());
    }
    let rows = all
        .iter()
        .take(MAX_PREVIEW_ROWS)
        .map(|r| {
            headers
                .iter()
                .map(|h| match &r[h] {
                    Value::Null => "".into(),
                    Value::String(s) => s.clone(),
                    v => v.to_string(),
                })
                .collect()
        })
        .collect();
    let truncated = all.len() > MAX_PREVIEW_ROWS;
    Ok(DatasetTable {
        source_name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        format: "json".into(),
        sheets: vec!["JSON".into()],
        selected_sheet: "JSON".into(),
        headers,
        rows,
        row_count: all.len(),
        truncated,
        warnings: if truncated {
            vec!["JSON 预览已截断；原文件保留".into()]
        } else {
            vec![]
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_numeric_rows_nested_provenance_and_missing_values() {
        let path = std::env::temp_dir().join(format!("suna-json-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"[{"HV":500,"sample":"合成","location":{"page":1}},{"HV":null,"sample":"第二个"}]"#,
        )
        .unwrap();
        let table = read_dataset_table(File::open(&path).unwrap(), &path, None).unwrap();
        assert_eq!(table.row_count, 2);
        assert_eq!(table.headers, vec!["HV", "location", "sample"]);
        assert_eq!(table.rows[0][0], "500");
        assert_eq!(table.rows[1][0], "");
        assert!(table.rows[0][1].contains("page"));
        std::fs::remove_file(path).unwrap();
    }
}
