use super::super::research_projects::SaveResearchProject;
use super::*;
fn fixture() -> (Connection, String) {
    let mut c = Connection::open_in_memory().unwrap();
    crate::storage::migrations::migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "ws",
        SaveResearchProject {
            id: None,
            name: "合成测试项目".into(),
            description: "".into(),
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
fn task(c: &Connection, kind: &str, payload: Value) -> String {
    crate::tasks::repository::create(
        c,
        crate::tasks::NewTask {
            workspace_id: "ws".into(),
            kind: kind.into(),
            payload_json: json!({"operation":"test","payload":payload}).to_string(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .unwrap()
    .id
    .to_string()
}
fn complete(c: &Connection, id: &str, result: Value) {
    c.execute(
        "UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",
        params![id, json!({"result":result}).to_string()],
    )
    .unwrap();
    reconcile_task(c, "ws", id).unwrap();
}
#[test]
fn maps_single_state_and_reconciles_immutable_outputs() {
    let (c, p) = fixture();
    let t = task(
        &c,
        "compute_train_linear_regression",
        json!({"seed":42,"untrusted_field":"secret","features":[[1,2]]}),
    );
    assert_eq!(runs(&c, "ws", &p, 50, 0).unwrap()[0].state, "queued");
    complete(
        &c,
        &t,
        json!({"artifact":{"feature_names":["C"],"model_base64":"secret-model"},"metrics":{"rmse":1},"access_token":"secret"}),
    );
    reconcile_task(&c, "ws", &t).unwrap();
    let a = list(&c, "ws", &p, 50, 0).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(runs(&c, "ws", &p, 50, 0).unwrap()[0].state, "completed");
    let body = get(&c, "ws", &a[0].id)
        .unwrap()
        .payload
        .unwrap()
        .to_string();
    assert!(!body.contains("secret"));
    assert!(body.contains("feature_names"));
    assert!(c
        .execute("UPDATE research_artifacts SET title='changed'", [])
        .is_err());
    assert!(get(&c, "other", &a[0].id).is_err());
}
#[test]
fn lineage_rejects_cycles_self_cross_project_and_duplicates() {
    let (c, p) = fixture();
    let a = create(&c, "ws", &p, "analysis_result", "A", "a", None, json!({})).unwrap();
    let b = create(&c, "ws", &p, "analysis_result", "B", "b", None, json!({})).unwrap();
    edge(&c, "ws", &a, &b, "uses_dataset").unwrap();
    edge(&c, "ws", &a, &b, "uses_dataset").unwrap();
    assert_eq!(edges(&c, "ws", &p, 100, 0).unwrap().len(), 1);
    assert!(edge(&c, "ws", &b, &a, "uses_model").is_err());
    assert!(edge(&c, "ws", &a, &a, "uses_model").is_err());
    assert!(edge(&c, "other", &a, &b, "uses_model").is_err());
    assert!(create(
        &c,
        "ws",
        &p,
        "analysis_result",
        "A",
        "a",
        None,
        json!({"revision":1})
    )
    .is_err());
}
#[test]
fn failed_cancelled_and_missing_tasks_never_fabricate_outputs() {
    let (c, p) = fixture();
    let t = task(&c, "compute_train_linear_regression", json!({}));
    c.execute(
        "UPDATE background_tasks SET state='failed',error_code='compute_failed' WHERE id=?1",
        [&t],
    )
    .unwrap();
    reconcile_project(&c, "ws", &p).unwrap();
    assert_eq!(
        runs(&c, "ws", &p, 10, 0).unwrap()[0].error_code.as_deref(),
        Some("compute_failed")
    );
    assert!(list(&c, "ws", &p, 10, 0).unwrap().is_empty());
    c.execute(
        "UPDATE background_tasks SET state='cancelled',error_code=NULL WHERE id=?1",
        [&t],
    )
    .unwrap();
    assert_eq!(runs(&c, "ws", &p, 10, 0).unwrap()[0].state, "cancelled");
    c.execute("DELETE FROM background_tasks WHERE id=?1", [t])
        .unwrap();
    assert_eq!(runs(&c, "ws", &p, 10, 0).unwrap()[0].state, "unavailable");
}
#[test]
fn submission_rollback_works_inside_an_agent_transaction() {
    let (mut c, p) = fixture();
    projects::archive(&mut c, "ws", &p, true).unwrap();
    let tx = c.transaction().unwrap();
    let result = crate::tasks::repository::create(
        &tx,
        crate::tasks::NewTask {
            workspace_id: "ws".into(),
            kind: "compute_train_linear_regression".into(),
            payload_json: "{}".into(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    );
    assert!(result.is_err());
    assert_eq!(
        tx.query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    tx.commit().unwrap();
}
#[test]
fn joint_output_links_two_independent_models() {
    let (c, p) = fixture();
    let m1 = task(&c, "compute_train_linear_regression", json!({"seed":1}));
    let m2 = task(&c, "compute_train_sklearn_model", json!({"seed":2}));
    complete(&c, &m1, json!({"artifact":{"feature_names":["C"]}}));
    complete(&c, &m2, json!({"artifact":{"feature_names":["C"]}}));
    let t = task(
        &c,
        "compute_optimize_constrained",
        json!({"objective_specs":[{"model_id":m1},{"model_id":m2}],"bounds":[{"min":0,"max":1}]}),
    );
    complete(
        &c,
        &t,
        json!({"recommendations":[{"features":{"C":0.4},"objective_values":[100,50]}]}),
    );
    let edges = edges(&c, "ws", &p, 100, 0).unwrap();
    assert_eq!(
        edges.iter().filter(|e| e.relation == "uses_model").count(),
        2
    );
}
