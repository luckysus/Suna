use super::{entry::*, runtime_support::*, worker_validation};
use crate::storage::repositories::research_projects as projects;
use crate::tasks::sources::AgentTaskSource;
use rusqlite::{params, Connection};
use serde_json::json;
use uuid::Uuid;
fn source(c: &Connection, project: &str) -> AgentTaskSource {
    c.execute(
        "INSERT INTO settings VALUES('local','suna.research.current_project',?1,'synthetic-now')",
        [json!(project).to_string()],
    )
    .unwrap();
    let run = Uuid::new_v4();
    let chat = Uuid::new_v4();
    let message = Uuid::new_v4();
    c.execute("INSERT INTO conversations(id,workspace_id,title,created_at,updated_at) VALUES(?1,'local','合成会话','now','now')",[chat.to_string()]).unwrap();
    c.execute("INSERT INTO messages(id,workspace_id,conversation_id,role,content,created_at) VALUES(?1,'local',?2,'user','合成确认','now')",params![message.to_string(),chat.to_string()]).unwrap();
    c.execute("INSERT INTO agent_runs(id,workspace_id,conversation_id,user_message_id,state,created_at,updated_at) VALUES(?1,'local',?2,?3,'executing_tools','now','now')",params![run.to_string(),chat.to_string(),message.to_string()]).unwrap();
    AgentTaskSource {
        workspace_id: "local".into(),
        run_id: run,
        tool_call_id: Uuid::new_v4(),
    }
}
fn counts(c: &Connection) -> Vec<u32> {
    [
        "background_tasks",
        "research_runs",
        "research_artifacts",
        "research_artifact_edges",
        "agent_task_sources",
    ]
    .iter()
    .map(|t| {
        c.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
            .unwrap()
    })
    .collect()
}
#[test]
fn agent_start_is_atomic_idempotent_frozen_to_its_project_and_preserves_existing_configuration() {
    let (mut f, _) = setup();
    let source = source(&f.connection, &f.input.project_id);
    let other = projects::save(
        &mut f.connection,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "另一合成课题".into(),
            description: String::new(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    f.connection
        .execute(
            "UPDATE settings SET value_json=?1 WHERE key='suna.research.current_project'",
            [json!(other.id).to_string()],
        )
        .unwrap();
    let selection = Selection {
        template_id: f.template.id.clone(),
        template_revision: 1,
        project_id: f.input.project_id.clone(),
        dataset_id: f.input.dataset_id.clone(),
        dataset_sha256: f.input.dataset_sha256.clone(),
    };
    let worker = worker_validation::probe(&f.template, Some(worker_config())).unwrap();
    let approved = preview(&f.connection, "local", &selection, &worker).unwrap();
    let mut r = StartRequest {
        template_id: selection.template_id,
        template_revision: 1,
        project_id: selection.project_id,
        dataset_id: selection.dataset_id,
        dataset_sha256: selection.dataset_sha256,
        dry_run_sha256: approved["dryRunSha256"].as_str().unwrap().into(),
        nonce: Uuid::new_v4(),
        confirmed: true,
    };
    let baseline = counts(&f.connection);
    let cancelled = std::cell::Cell::new(0);
    assert_eq!(
        start_for_agent(
            &mut f.connection,
            "local",
            &r,
            &worker,
            source.clone(),
            || {
                let n = cancelled.get();
                cancelled.set(n + 1);
                n > 0
            }
        )
        .unwrap_err(),
        "workflow_cancelled"
    );
    assert_eq!(counts(&f.connection), baseline);
    let first = start_for_agent(
        &mut f.connection,
        "local",
        &r,
        &worker,
        source.clone(),
        || false,
    )
    .unwrap();
    let once = counts(&f.connection);
    assert_eq!(
        start_for_agent(
            &mut f.connection,
            "local",
            &r,
            &worker,
            source.clone(),
            || false
        )
        .unwrap(),
        first
    );
    assert_eq!(counts(&f.connection), once);
    assert_eq!(first["projectId"], f.input.project_id);
    let task = crate::tasks::sources::for_tool_call(&f.connection, &source)
        .unwrap()
        .unwrap();
    assert_eq!(task.id.to_string(), first["task"]["id"]);
    r.nonce = Uuid::new_v4();
    assert_eq!(
        start_for_agent(&mut f.connection, "local", &r, &worker, source, || false).unwrap_err(),
        "workflow_agent_task_source_conflict"
    );
    assert_eq!(counts(&f.connection), once);
    assert_eq!(
        f.connection
            .query_row(
                "SELECT value_json FROM settings WHERE key='suna.research.current_project'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        json!(other.id).to_string()
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, u32>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        super::status::get(&f.connection, "local", &f.input.project_id, task.id).unwrap(),
        first
    );
    assert!(super::status::get(&f.connection, "local", &other.id, task.id).is_err());
}
#[test]
fn desktop_start_rejects_unknown_fields_self_supplied_permits_and_nil_nonce() {
    let r = json!({"templateId":Uuid::new_v4(),"templateRevision":1,"projectId":Uuid::new_v4(),"datasetId":Uuid::new_v4(),"datasetSha256":"a".repeat(64),"dryRunSha256":"b".repeat(64),"nonce":Uuid::new_v4(),"confirmed":true});
    assert!(serde_json::from_value::<StartRequest>(r.clone()).is_ok());
    let mut invalid = r;
    invalid["workerValidation"] = json!({"dependencies":"ready"});
    assert!(serde_json::from_value::<StartRequest>(invalid).is_err());
    let (mut f, _) = setup();
    let worker = worker_validation::probe(&f.template, Some(worker_config())).unwrap();
    let r = StartRequest {
        template_id: f.template.id.clone(),
        template_revision: 1,
        project_id: f.input.project_id.clone(),
        dataset_id: f.input.dataset_id.clone(),
        dataset_sha256: f.input.dataset_sha256.clone(),
        dry_run_sha256: "b".repeat(64),
        nonce: Uuid::nil(),
        confirmed: true,
    };
    assert_eq!(
        start(&mut f.connection, "local", &r, &worker).unwrap_err(),
        "workflow_nonce_invalid"
    );
    assert_eq!(counts(&f.connection)[0], 0);
}
