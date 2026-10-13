use projects::SaveResearchProject;
use rusqlite::{params, Connection};
use serde_json::json;
use suna::storage::repositories::{
    reports, research_artifacts as artifacts, research_delivery::*, research_projects as projects,
};
fn fixture() -> (Connection, String) {
    let mut c = Connection::open_in_memory().unwrap();
    suna::storage::migrations::migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "ws",
        SaveResearchProject {
            id: None,
            name: "合成报告课题".into(),
            description: "验证成果来源".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    c.execute(
        "INSERT INTO settings VALUES('ws','suna.research.current_project',?1,'now')",
        [json!(p.id).to_string()],
    )
    .unwrap();
    (c, p.id)
}
fn run(c: &Connection, id: &str) {
    c.execute("INSERT OR IGNORE INTO conversations(id,workspace_id,title,created_at,updated_at) VALUES('chat','ws','测试','now','now')",[]).unwrap();
    c.execute("INSERT OR IGNORE INTO messages(id,workspace_id,conversation_id,role,content,created_at) VALUES('msg','ws','chat','user','测试','now')",[]).unwrap();
    c.execute("INSERT INTO agent_runs(id,workspace_id,conversation_id,user_message_id,state,created_at,updated_at) VALUES(?1,'ws','chat','msg','created','now','now')",[id]).unwrap();
}
#[test]
fn agent_context_is_frozen_scoped_and_survives_project_switch() {
    let (c, p) = fixture();
    run(&c, "run1");
    c.execute(
        "UPDATE settings SET value_json='null' WHERE key='suna.research.current_project'",
        [],
    )
    .unwrap();
    run(&c, "run2");
    assert_eq!(context(&c, "ws", "run1").unwrap(), Some(p.clone()));
    assert_eq!(context(&c, "other", "run1").unwrap(), None);
    assert_eq!(context(&c, "ws", "run2").unwrap(), None);
    assert!(c
        .execute("UPDATE research_agent_contexts SET project_id=NULL", [])
        .is_err());
    assert_eq!(projects::objects(&c, "ws", &p, 25, 0).unwrap().len(), 2);
    scoped(&c, Some(&p), || {
        assert_eq!(artifacts::current(&c, "ws")?, Some(p.clone()));
        let e: Result<(), String> = scoped(&c, None, || {
            assert_eq!(artifacts::current(&c, "ws")?, None);
            Err("failure".into())
        });
        assert!(e.is_err());
        assert_eq!(artifacts::current(&c, "ws")?, Some(p.clone()));
        Ok(())
    })
    .unwrap();
    assert_eq!(artifacts::current(&c, "ws").unwrap(), None);
}
#[test]
fn report_is_persisted_with_verified_sources_and_export_contains_no_raw_payload() {
    let (mut c, p) = fixture();
    let task=suna::tasks::repository::create(&c,suna::tasks::NewTask{workspace_id:"ws".into(),kind:"compute_train_linear_regression".into(),payload_json:json!({"operation":"test","payload":{"features":[[99.]],"api_key":"sentinel-secret","source_path":"/sentinel/private"}}).to_string(),checkpoint_json:None,next_run_at:None,progress:0}).unwrap();
    c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",params![task.id.to_string(),json!({"result":{"artifact":{"model_base64":"sentinel-blob","feature_names":["C"]},"metrics":{"rmse":1.}}}).to_string()]).unwrap();
    artifacts::reconcile_task(&c, "ws", &task.id.to_string()).unwrap();
    let m = manifest(&c, "ws", &p).unwrap();
    let raw = m.to_string();
    for s in [
        "sentinel-secret",
        "sentinel-blob",
        "/sentinel/private",
        "[[99",
    ] {
        assert!(!raw.contains(s))
    }
    assert_eq!(m["containsRawData"], false);
    assert!(manifest(&c, "other", &p).is_err());
    let r = report(&mut c, "ws", &p).unwrap();
    assert!(r.content_markdown.contains("feature_names"));
    assert_eq!(reports::get(&c, "ws", &r.id).unwrap().unwrap(), r);
    assert_eq!(artifacts::edges(&c, "ws", &p, 25, 0).unwrap().len(), 1);
    assert_eq!(
        manifest(&c, "ws", &p).unwrap()["artifacts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn report_and_links_roll_back_together() {
    let (mut c, p) = fixture();
    c.execute_batch("CREATE TRIGGER reject_test_artifact BEFORE INSERT ON research_artifacts BEGIN SELECT RAISE(ABORT,'simulated_disk_failure'); END").unwrap();
    assert!(report(&mut c, "ws", &p).is_err());
    assert!(reports::list(&c, "ws").unwrap().is_empty());
    assert!(projects::objects(&c, "ws", &p, 25, 0).unwrap().is_empty());
}
#[test]
fn attachments_cannot_guess_cross_workspace_objects_or_change_project_context() {
    let (mut c, p) = fixture();
    run(&c, "run");
    let r = reports::create(
        &mut c,
        "other",
        reports::NewReport {
            kind: "测试",
            title: "他区报告",
            topic: "",
            format: "markdown",
            content_markdown: "",
            dataset_id: None,
            knowledge_base_ids_json: "[]",
            metadata_json: "{}",
        },
    )
    .unwrap();
    assert!(agent_call(
        &mut c,
        "ws",
        Some("run"),
        "research_attach_result",
        json!({"kind":"report","id":r.id})
    )
    .is_err());
    let ctx = agent_call(
        &mut c,
        "ws",
        Some("run"),
        "research_get_project_context",
        json!({"projectId":"other"}),
    )
    .unwrap();
    assert_eq!(ctx["project"]["id"], p);
    projects::archive(&mut c, "ws", &p, true).unwrap();
    assert!(report(&mut c, "ws", &p).is_err());
}
#[test]
fn agent_and_project_views_reference_the_same_task_after_switch() {
    let (mut c, p) = fixture();
    let id = uuid::Uuid::new_v4();
    run(&c, &id.to_string());
    c.execute(
        "UPDATE agent_runs SET state='executing_tools' WHERE id=?1",
        [id.to_string()],
    )
    .unwrap();
    c.execute(
        "UPDATE settings SET value_json='null' WHERE key='suna.research.current_project'",
        [],
    )
    .unwrap();
    let frozen = context(&c, "ws", &id.to_string()).unwrap();
    let source = suna::tasks::sources::AgentTaskSource {
        workspace_id: "ws".into(),
        run_id: id,
        tool_call_id: uuid::Uuid::new_v4(),
    };
    let task = suna::tasks::sources::submit_with(&mut c, source.clone(), |c| {
        scoped(c, frozen.as_deref(), || {
            suna::tasks::repository::create(
                c,
                suna::tasks::NewTask {
                    workspace_id: "ws".into(),
                    kind: "compute_optimize_constrained".into(),
                    payload_json: json!({"operation":"optimize_constrained","payload":{"seed":42}})
                        .to_string(),
                    checkpoint_json: None,
                    next_run_at: None,
                    progress: 0,
                },
            )
            .map_err(|e| e.to_string())
        })
    })
    .unwrap();
    let project_view = artifacts::runs(&c, "ws", &p, 25, 0).unwrap();
    assert_eq!(project_view.len(), 1);
    assert_eq!(
        project_view[0].background_task_id,
        Some(task.id.to_string())
    );
    assert_eq!(project_view[0].agent_run_id, Some(id.to_string()));
    assert_eq!(project_view[0].origin, "agent");
    let replay =
        suna::tasks::sources::submit_with(&mut c, source, |_| panic!("must reuse original task"))
            .unwrap();
    assert_eq!(replay.id, task.id);
    assert_eq!(artifacts::current(&c, "ws").unwrap(), None);
}
