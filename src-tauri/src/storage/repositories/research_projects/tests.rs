use super::*;
use crate::storage::migrations::migrate;
fn db() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    migrate(&mut c).unwrap();
    c
}
pub fn project(c: &mut Connection, ws: &str) -> ResearchProject {
    save(
        c,
        ws,
        SaveResearchProject {
            id: None,
            name: "耐磨钢课题".into(),
            description: "联合优化".into(),
            material_system: Some("steel".into()),
            tags: vec!["磨损".into()],
            expected_updated_at: None,
        },
    )
    .unwrap()
}
fn sources(c: &Connection) {
    c.execute_batch("INSERT INTO steel_datasets VALUES('ws','data','合成数据.csv','/fixture','sha','csv','',2,2,0,'ready','{}','now','now'); INSERT INTO steel_models(workspace_id,id,lineage_id,kind,version,model_sha256,manifest_json,artifact_json,is_active,created_at) VALUES('ws','model','lineage','linear_artifact',1,'sha','{}','{}',1,'now'); INSERT INTO background_tasks(id,workspace_id,kind,state,payload_json,created_at,updated_at) VALUES('task','ws','compute.train','queued','{}','now','now');").unwrap();
}
#[test]
fn links_are_idempotent_scoped_and_non_destructive() {
    let mut c = db();
    sources(&c);
    let p = project(&mut c, "ws");
    for (k, id) in [
        ("dataset", "data"),
        ("model", "model"),
        ("background_task", "task"),
    ] {
        attach(&mut c, "ws", &p.id, k, id, "attached").unwrap();
        attach(&mut c, "ws", &p.id, k, id, "attached").unwrap();
    }
    assert_eq!(
        objects(&c, "ws", &p.id, 50, 0)
            .unwrap()
            .iter()
            .map(|o| (&o.kind, &o.id))
            .collect::<std::collections::HashSet<_>>()
            .len(),
        3
    );
    assert!(attach(&mut c, "other", &p.id, "dataset", "data", "attached").is_err());
    let other = project(&mut c, "other");
    assert!(attach(&mut c, "other", &other.id, "dataset", "data", "attached").is_err());
    assert!(get(&c, "ws", "missing").is_err());
    assert!(attach(&mut c, "ws", &p.id, "dataset", "missing", "attached").is_err());
    detach(&mut c, "ws", &p.id, "dataset", "data", "attached").unwrap();
    assert!(target_title(&c, "ws", "dataset", "data").unwrap().is_some());
    archive(&mut c, "ws", &p.id, true).unwrap();
    assert!(attach(&mut c, "ws", &p.id, "dataset", "data", "attached").is_err());
    assert!(target_title(&c, "ws", "model", "model").unwrap().is_some());
    c.execute("DELETE FROM background_tasks WHERE id='task'", [])
        .unwrap();
    assert_eq!(summary(&c, "ws", &p.id).unwrap().missing_sources, 1);
    assert!(c.execute("DELETE FROM research_audit_events", []).is_err());
}
#[test]
fn survives_reopen_and_conflicting_edit() {
    let directory = std::env::temp_dir().join(format!("suna-project-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("研究.sqlite");
    let id = {
        let mut c = Connection::open(&path).unwrap();
        migrate(&mut c).unwrap();
        let p = project(&mut c, "ws");
        assert!(save(
            &mut c,
            "ws",
            SaveResearchProject {
                id: Some(p.id.clone()),
                name: "覆盖".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: Some("stale".into())
            }
        )
        .is_err());
        p.id
    };
    let mut c = Connection::open(&path).unwrap();
    assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
    assert_eq!(get(&c, "ws", &id).unwrap().name, "耐磨钢课题");
    drop(c);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn upgrades_v35_preserving_user_configuration() {
    let mut c = Connection::open_in_memory().unwrap();
    // Execute actual pre-project migrations, including their production migration bookkeeping.
    for m in crate::storage::migrations::MIGRATIONS
        .iter()
        .filter(|m| m.version <= 35)
    {
        let tx = c.transaction().unwrap();
        tx.execute_batch(m.sql).unwrap();
        tx.pragma_update(None, "user_version", m.version).unwrap();
        tx.commit().unwrap();
    }
    c.execute("INSERT INTO settings(workspace_id,key,value_json,updated_at) VALUES('ws','user-theme','\"dark\"','now')",[]).unwrap();
    let report = migrate(&mut c).unwrap();
    assert_eq!(
        report.applied_versions,
        (36..=crate::storage::migrations::latest_version()).collect::<Vec<_>>()
    );
    assert_eq!(
        c.query_row(
            "SELECT value_json FROM settings WHERE key='user-theme'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "\"dark\""
    );
    project(&mut c, "ws");
}
#[test]
fn paging_is_bounded_stable_searchable_and_indexed() {
    let mut c = db();
    let p = project(&mut c, "ws");
    for n in 0..275 {
        let id = format!("sample-{n:04}");
        c.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at) VALUES(?1,'ws',?1,'now','now')",[&id]).unwrap();
        attach(&mut c, "ws", &p.id, "conversation", &id, "attached").unwrap();
    }
    let first = objects(&c, "ws", &p.id, 1000, 0).unwrap();
    assert_eq!(first.len(), 100);
    let second = objects(&c, "ws", &p.id, 100, 100).unwrap();
    assert!(first.iter().all(|a| second.iter().all(|b| a.id != b.id)));
    assert_eq!(
        available(&c, "ws", "conversation", "sample-025", 25, 0)
            .unwrap()
            .len(),
        10
    );
    assert!(available(&c, "other", "conversation", "", 25, 0)
        .unwrap()
        .is_empty());
    let plan:String=c.query_row("EXPLAIN QUERY PLAN SELECT * FROM research_project_links WHERE workspace_id='ws' AND project_id=?1",[&p.id],|r|r.get(3)).unwrap();
    assert!(plan.contains("INDEX"));
    assert_eq!(
        summary(&c, "ws", &p.id).unwrap().counts["conversation"],
        275
    );
}
#[test]
fn failed_v40_upgrade_rolls_back_and_keeps_old_settings() {
    let mut c = Connection::open_in_memory().unwrap();
    for m in crate::storage::migrations::MIGRATIONS
        .iter()
        .filter(|m| m.version < 40)
    {
        let tx = c.transaction().unwrap();
        tx.execute_batch(m.sql).unwrap();
        tx.pragma_update(None, "user_version", m.version).unwrap();
        tx.commit().unwrap();
    }
    c.execute(
        "INSERT INTO settings VALUES('ws','user-theme','\"dark\"','now')",
        [],
    )
    .unwrap();
    c.execute_batch("CREATE TABLE research_agent_contexts(broken TEXT)")
        .unwrap();
    assert!(migrate(&mut c).is_err());
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        39
    );
    assert_eq!(
        c.query_row(
            "SELECT value_json FROM settings WHERE key='user-theme'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "\"dark\""
    );
    c.execute_batch("DROP TABLE research_agent_contexts")
        .unwrap();
    migrate(&mut c).unwrap();
    assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
}
