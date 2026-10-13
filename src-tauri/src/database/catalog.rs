use crate::database::SqlClient;

/// 把数据库名转成安全的 `[name]` 标识符(右括号翻倍转义)。
pub fn escape_identifier(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("database name is required".to_string());
    }
    if trimmed.len() > 128 {
        return Err("database name is too long".to_string());
    }
    Ok(format!("[{}]", trimmed.replace(']', "]]")))
}

pub async fn list_databases(client: &mut SqlClient) -> Result<Vec<String>, String> {
    let stream = client
        .query("SELECT name FROM sys.databases ORDER BY name", &[])
        .await
        .map_err(|error| error.to_string())?;
    let rows = stream
        .into_first_result()
        .await
        .map_err(|error| error.to_string())?;
    Ok(rows
        .into_iter()
        .filter_map(|row| row.get::<&str, _>(0).map(|value| value.to_string()))
        .collect())
}

pub async fn table_names(
    client: &mut SqlClient,
    database: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(name) = database {
        let use_statement = format!("USE {}", escape_identifier(name)?);
        client
            .simple_query(use_statement)
            .await
            .map_err(|error| format!("cannot switch to database {name}: {error}"))?;
    }
    crate::database::table_names(client).await
}

pub struct QueryRows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

pub async fn execute_read(client: &mut SqlClient, sql: &str) -> Result<QueryRows, String> {
    execute_read_with_parameters(client, sql, &[]).await
}

pub async fn execute_read_with_parameters(
    client: &mut SqlClient,
    sql: &str,
    parameters: &[String],
) -> Result<QueryRows, String> {
    use futures_util::TryStreamExt;
    let args: Vec<&dyn tiberius::ToSql> = parameters
        .iter()
        .map(|s| s as &dyn tiberius::ToSql)
        .collect();
    let mut stream = client
        .query(sql, &args)
        .await
        .map_err(|_| "query_execution_failed")?;
    let columns: Vec<String> = stream
        .columns()
        .await
        .map_err(|_| "query_metadata_failed")?
        .map(|c| c.iter().map(|m| m.name().to_string()).collect())
        .unwrap_or_default();
    if columns.len() > 256 {
        return Err("query_column_limit".into());
    }
    let mut rows = vec![];
    let mut bytes = 0;
    while let Some(item) = stream.try_next().await.map_err(|_| "query_stream_failed")? {
        if let tiberius::QueryItem::Row(row) = item {
            if rows.len() >= 10000 {
                return Err("query_row_limit".into());
            }
            let cells = row
                .cells()
                .map(|(_, value)| cell_text(value))
                .collect::<Result<Vec<_>, _>>()?;
            bytes += cells
                .iter()
                .map(|c| c.as_ref().map_or(0, |s| s.len()))
                .sum::<usize>();
            if bytes > 32 * 1024 * 1024 {
                return Err("query_return_byte_limit".into());
            }
            rows.push(cells);
        }
    }
    Ok(QueryRows { columns, rows })
}

/// Preserve SQL numeric precision and sampling timestamps. Unsupported non-null
/// values fail visibly rather than silently becoming missing measurements.
fn cell_text(value: &tiberius::ColumnData<'_>) -> Result<Option<String>, String> {
    use tiberius::{ColumnData as D, FromSql};
    let v = match value {
        D::U8(v) => v.map(|v| v.to_string()),
        D::I16(v) => v.map(|v| v.to_string()),
        D::I32(v) => v.map(|v| v.to_string()),
        D::I64(v) => v.map(|v| v.to_string()),
        D::F32(v) => {
            if v.is_some_and(|v| !v.is_finite()) {
                return Err("nonfinite_query_number".into());
            }
            v.map(|v| v.to_string())
        }
        D::F64(v) => {
            if v.is_some_and(|v| !v.is_finite()) {
                return Err("nonfinite_query_number".into());
            }
            v.map(|v| v.to_string())
        }
        D::Numeric(v) => v.map(|v| v.to_string()),
        D::Bit(v) => v.map(|v| if v { "1".into() } else { "0".into() }),
        D::String(v) => v.as_ref().map(|v| v.to_string()),
        D::Guid(v) => v.map(|v| v.to_string()),
        D::DateTime(_) | D::SmallDateTime(_) | D::DateTime2(_) => {
            chrono::NaiveDateTime::from_sql(&match value {
                D::DateTime(v) => D::DateTime(*v),
                D::SmallDateTime(v) => D::SmallDateTime(*v),
                D::DateTime2(v) => D::DateTime2(*v),
                _ => unreachable!(),
            })
            .map_err(|_| "invalid_query_timestamp")?
            .map(|v| v.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
        }
        D::DateTimeOffset(v) => {
            chrono::DateTime::<chrono::FixedOffset>::from_sql(&D::DateTimeOffset(*v))
                .map_err(|_| "invalid_query_timestamp")?
                .map(|v| v.to_rfc3339())
        }
        D::Date(v) => chrono::NaiveDate::from_sql(&D::Date(*v))
            .map_err(|_| "invalid_query_date")?
            .map(|v| v.to_string()),
        D::Time(v) => chrono::NaiveTime::from_sql(&D::Time(*v))
            .map_err(|_| "invalid_query_time")?
            .map(|v| v.to_string()),
        D::Binary(None) | D::Xml(None) => None,
        D::Binary(Some(_)) | D::Xml(Some(_)) => return Err("unsupported_query_column_type".into()),
    };
    Ok(v)
}

/// 最小 CSV 单元格转义;供查询结果缓存文件使用。
pub fn csv_cell(value: Option<&str>) -> String {
    match value {
        None => String::new(),
        Some(text) => {
            if text.contains(',')
                || text.contains('"')
                || text.contains('\n')
                || text.contains('\r')
            {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_and_timestamp_cells_are_preserved_and_unsupported_values_are_visible() {
        use tiberius::{numeric::Numeric, ColumnData as D, ToSql};
        assert_eq!(
            cell_text(&D::Numeric(Some(Numeric::new_with_scale(12345, 2)))).unwrap(),
            Some("123.45".into())
        );
        assert_eq!(cell_text(&D::I16(Some(300))).unwrap(), Some("300".into()));
        assert_eq!(cell_text(&D::F32(Some(1.25))).unwrap(), Some("1.25".into()));
        assert_eq!(cell_text(&D::Numeric(None)).unwrap(), None);
        let when = chrono::NaiveDate::from_ymd_opt(2026, 1, 2)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap();
        assert_eq!(
            cell_text(&when.to_sql()).unwrap(),
            Some("2026-01-02T03:04:05".into())
        );
        assert!(cell_text(&D::F64(Some(f64::NAN))).is_err());
        assert!(cell_text(&D::Binary(Some(vec![1].into()))).is_err());
    }

    #[test]
    fn escape_identifier_brackets_and_doubles_closing_brackets() {
        assert_eq!(
            escape_identifier("SteelWorks").expect("plain"),
            "[SteelWorks]"
        );
        assert_eq!(escape_identifier(" my]db ").expect("trimmed"), "[my]]db]");
    }

    #[test]
    fn escape_identifier_rejects_empty() {
        assert!(escape_identifier("").is_err());
        assert!(escape_identifier("   ").is_err());
    }

    #[test]
    fn escape_identifier_rejects_long_names() {
        assert!(escape_identifier(&"a".repeat(129)).is_err());
    }

    #[test]
    fn csv_cell_quotes_specials() {
        assert_eq!(csv_cell(Some("plain")), "plain");
        assert_eq!(csv_cell(Some("a,b")), "\"a,b\"");
        assert_eq!(csv_cell(Some("say \"hi\"")), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell(Some("line\nbreak")), "\"line\nbreak\"");
        assert_eq!(csv_cell(None), "");
    }
}
