use crate::storage::{
    migrations::{migrate, MIGRATIONS},
    repositories::{research_artifacts as artifacts, research_projects as projects},
};
use rusqlite::Connection;
use serde_json::json;
#[test]
fn upgrade_from_real_version_43_preserves_configuration_frozen_hashes_and_foreign_keys() {
    let mut c = Connection::open_in_memory().unwrap();
    // Build the actual historical schema from its original migration sources.
    for m in MIGRATIONS.iter().filter(|m| m.version <= 43) {
        c.execute_batch(m.sql).unwrap();
        c.execute("INSERT INTO schema_migrations(version,applied_at) VALUES(?1,'synthetic-original-time')",[m.version]).unwrap();
    }
    c.pragma_update(None, "user_version", 43).unwrap();
    let p = projects::save(
        &mut c,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "合成旧课题".into(),
            description: "迁移回归".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    c.execute("INSERT INTO settings VALUES('local','synthetic.preferences','{\"selected\":\"原配置\"}','old-time')",[]).unwrap();
    let id = artifacts::create(
        &c,
        "local",
        &p.id,
        "analysis_result",
        "旧冻结成果",
        "synthetic:old-result",
        None,
        json!({"schemaVersion":"synthetic-result.v1","value":0.1,"unit":"wt%"}),
    )
    .unwrap();
    let before = artifacts::get(&c, "local", &id).unwrap();
    let old_rows: u32 = c
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(old_rows, 43);
    let report = migrate(&mut c).unwrap();
    assert_eq!(report.applied_versions, vec![44]);
    let after = artifacts::get(&c, "local", &id).unwrap();
    assert_eq!(before.content_sha256, after.content_sha256);
    assert_eq!(before.payload, after.payload);
    assert_eq!(
        c.query_row(
            "SELECT value_json,updated_at FROM settings WHERE key='synthetic.preferences'",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        ("{\"selected\":\"原配置\"}".into(), "old-time".into())
    );
    assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(c
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query([])
        .unwrap()
        .next()
        .unwrap()
        .is_none());
    assert_eq!(
        artifacts::get(&c, "local", &id).unwrap().content_sha256,
        before.content_sha256
    );
}
