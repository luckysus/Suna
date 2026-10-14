pub(crate) use super::runtime_support::*;
use crate::storage::repositories::research_projects as projects;
use rusqlite::Connection;
use serde_json::{json, Value};
#[test]
fn actual_scheduler_runs_two_models_joint_optimization_and_only_proposed_plan() {
    let (mut f, path) = setup();
    let nonce = uuid::Uuid::new_v4();
    // Use the same typed boundary and actual offline diagnostic as the desktop.
    let worker =
        crate::research_workflow::worker_validation::probe(&f.template, Some(worker_config()))
            .unwrap();
    let selection = crate::research_workflow::entry::Selection {
        template_id: f.template.id.clone(),
        template_revision: 1,
        project_id: f.input.project_id.clone(),
        dataset_id: f.input.dataset_id.clone(),
        dataset_sha256: f.input.dataset_sha256.clone(),
    };
    let preview =
        crate::research_workflow::entry::preview(&f.connection, "local", &selection, &worker)
            .unwrap();
    let request = crate::research_workflow::entry::StartRequest {
        template_id: selection.template_id,
        template_revision: 1,
        project_id: selection.project_id,
        dataset_id: selection.dataset_id,
        dataset_sha256: selection.dataset_sha256,
        dry_run_sha256: preview["dryRunSha256"].as_str().unwrap().into(),
        nonce,
        confirmed: true,
    };
    let started =
        crate::research_workflow::entry::start(&mut f.connection, "local", &request, &worker)
            .unwrap();
    let id = uuid::Uuid::parse_str(started["task"]["id"].as_str().unwrap()).unwrap();
    let parent = record(&f.connection, id);
    assert_eq!(
        crate::research_workflow::entry::start(&mut f.connection, "local", &request, &worker)
            .unwrap()["task"]["id"],
        id.to_string()
    );
    let payload: Value = serde_json::from_str(&parent.payload_json).unwrap();
    assert_eq!(
        payload["worker_validation"]["validationStatus"],
        "actual_offline_diagnostic"
    );
    assert!(!payload
        .to_string()
        .contains(&f.root.to_string_lossy().to_string()));
    let approved_template =
        crate::research_workflow::templates::get(&f.connection, "local", &f.template.id, 1)
            .unwrap();
    let mut next_version = f.template.clone();
    next_version.name = "新修订（不改变已确认运行）".into();
    next_version.seed += 1;
    crate::research_workflow::templates::save(&mut f.connection, "local", &next_version, 1)
        .unwrap();
    let other = projects::save(
        &mut f.connection,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "另一个合成项目".into(),
            description: String::new(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    f.connection
        .execute(
            "INSERT INTO settings VALUES('local','suna.research.current_project',?1,'now')",
            [json!(other.id).to_string()],
        )
        .unwrap();
    let mut scheduler = new_scheduler(&path);
    let done = completed(&mut scheduler, &f.connection, parent.id);
    let cp: Value = serde_json::from_str(done.checkpoint_json.as_deref().unwrap()).unwrap();
    assert_eq!(cp["children"].as_array().unwrap().len(), 2);
    assert_eq!(cp["models"].as_array().unwrap().len(), 2);
    for id in cp["models"].as_array().unwrap() {
        let model = crate::storage::repositories::steel_models::get(
            &f.connection,
            "local",
            id.as_str().unwrap(),
        )
        .unwrap()
        .unwrap();
        assert!(
            !model.is_active,
            "workflow versions must wait for explicit activation"
        );
    }
    assert_eq!(cp["result"]["confirmationRequiredToAccept"], true);
    assert_eq!(cp["result"]["measurementsCreated"], false);
    let owned = crate::research_workflow::provenance::owned_nodes(&f.connection, &done).unwrap();
    assert_eq!(owned.len(), 3);
    let summary_id = cp["resultArtifactId"].as_str().unwrap();
    let summary =
        crate::storage::repositories::research_artifacts::get(&f.connection, "local", summary_id)
            .unwrap();
    let snapshot = summary.payload.as_ref().unwrap();
    let public_status =
        crate::research_workflow::status::get(&f.connection, "local", &f.input.project_id, done.id)
            .unwrap();
    assert_eq!(public_status["task"]["state"], "completed");
    assert_eq!(public_status["nodes"].as_array().unwrap().len(), 3);
    assert_eq!(public_status["result"]["artifactId"], summary_id);
    assert_eq!(public_status["result"]["registeredModelIds"], cp["models"]);
    assert_eq!(snapshot["templateRevision"], 1);
    assert_eq!(
        snapshot["templateContentSha256"],
        approved_template["contentSha256"]
    );
    assert_eq!(snapshot["measurementsCreated"], false);
    assert!(!snapshot
        .to_string()
        .contains(f.root.to_string_lossy().as_ref()));
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_artifact_edges WHERE to_artifact_id=?1",
                [summary_id],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        5
    );
    let reopened = Connection::open(&path).unwrap();
    assert_eq!(
        crate::storage::repositories::research_artifacts::get(&reopened, "local", summary_id)
            .unwrap()
            .content_sha256,
        summary.content_sha256
    );
    drop(reopened);
    assert_eq!(
        f.connection
            .query_row(
                "SELECT state FROM experiments WHERE id=?1",
                [cp["plan"].as_str().unwrap()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "proposed"
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM steel_models", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_runs WHERE project_id=?1",
                [&other.id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM research_runs WHERE project_id=?1",
                [&f.input.project_id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
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
        crate::research_workflow::entry::start(&mut f.connection, "local", &request, &worker)
            .unwrap()["task"]["id"],
        parent.id.to_string()
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiments", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drain(&mut scheduler);
    drop(scheduler);
}
