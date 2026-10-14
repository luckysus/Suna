//! A reviewed metadata export binds to the exact bytes. No destination or live writes.
use crate::research_package::{archive, digest, projection::Projection, valid_sha};
use serde_json::{json, Value};

pub struct PreparedExport {
    bytes: Vec<u8>,
    summary: Value,
}

pub fn prepare(projection: Projection) -> Result<PreparedExport, String> {
    let bytes = archive::export(projection)?;
    let checked = archive::inspect(&bytes)?;
    let mut summary = checked.summary();
    summary["packageSha256"] = json!(digest(&bytes));
    summary["bytes"] = json!(bytes.len());
    Ok(PreparedExport { bytes, summary })
}

impl PreparedExport {
    pub fn summary(&self) -> &Value {
        &self.summary
    }
    /// The caller must read the current project again before this comparison.
    /// A changed source requires another preview before opening the save dialog.
    pub fn confirm(self, expected_sha256: &str) -> Result<Self, String> {
        if !valid_sha(expected_sha256) || self.summary["packageSha256"] != expected_sha256 {
            return Err("package_preview_changed".into());
        }
        Ok(self)
    }
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        migrations,
        repositories::{research_artifacts, research_projects},
    };

    #[test]
    fn reviewed_source_cannot_change_silently_and_preview_is_read_only() {
        let mut c = rusqlite::Connection::open_in_memory().unwrap();
        migrations::migrate(&mut c).unwrap();
        let p = research_projects::save(
            &mut c,
            "local",
            research_projects::SaveResearchProject {
                id: None,
                name: "中文合成预览".into(),
                description: "synthetic-private-description".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        let changes: i64 = c
            .query_row("SELECT total_changes()", [], |r| r.get(0))
            .unwrap();
        let first = prepare(
            crate::research_package::projection::from_connection(&c, "local", &p.id).unwrap(),
        )
        .unwrap();
        let sha = first.summary()["packageSha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let again = prepare(
            crate::research_package::projection::from_connection(&c, "local", &p.id).unwrap(),
        )
        .unwrap();
        assert_eq!(first.summary(), again.summary());
        assert_eq!(
            first.into_bytes(),
            again.confirm(&sha).unwrap().into_bytes()
        );
        assert_eq!(
            c.query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            changes
        );
        assert!(c.is_autocommit());
        research_artifacts::create(
            &c,
            "local",
            &p.id,
            "analysis_result",
            "新增合成成果",
            "synthetic-new-result",
            None,
            json!({"raw_rows":[987.123456],"source_path":"C:/synthetic-private.csv"}),
        )
        .unwrap();
        let fresh = prepare(
            crate::research_package::projection::from_connection(&c, "local", &p.id).unwrap(),
        )
        .unwrap();
        assert_eq!(fresh.summary()["metadataRecordCount"], 2);
        assert_eq!(
            fresh.confirm(&sha).err().unwrap(),
            "package_preview_changed"
        );
        assert!(
            crate::research_package::projection::from_connection(&c, "other-workspace", &p.id)
                .is_err()
        );
        assert!(c.is_autocommit());
    }
}
