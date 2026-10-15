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

fn metadata_record(c: &Connection, ws: &str, provider: &str, title: &str) -> Value {
    let r = super::super::literature_discovery::store(
        c,
        ws,
        crate::literature_metadata::Metadata {
            provider: provider.into(),
            external_id: "synthetic-revision".into(),
            doi: Some("10.1234/revision-fixture".into()),
            title: title.into(),
            authors: vec![],
            journal: None,
            year: Some(2020),
            abstract_text: None,
            source_url: "https://doi.org/10.1234/revision-fixture".into(),
            license: None,
            fulltext_state: "link_only".into(),
        },
    )
    .unwrap();
    json!(r)
}
fn literature_task(c: &Connection, request: Value, result: Value) -> String {
    let t = crate::tasks::repository::create(
        c,
        crate::tasks::NewTask {
            workspace_id: "ws".into(),
            kind: "literature_metadata_search".into(),
            payload_json: request.to_string(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .unwrap();
    c.execute(
        "UPDATE background_tasks SET state='completed',progress=100,next_run_at=NULL,checkpoint_json=?2 WHERE id=?1",
        params![t.id.to_string(), json!({"result":result}).to_string()],
    )
    .unwrap();
    t.id.to_string()
}

#[test]
fn literature_freezes_the_actual_revision_with_legacy_default_and_preserves_newer_current_revision()
{
    for source in ["crossref", "europe_pmc"] {
        let (c, p) = fixture();
        let original = metadata_record(&c, "ws", source, "Original synthetic metadata");
        let mut request = json!({"query":"synthetic query","fromYear":2019,"toYear":2021});
        let mut result = json!({"records":[original.clone()]});
        if source != "crossref" {
            request["provider"] = json!(source);
            result["provider"] = json!(source);
        }
        let task = literature_task(&c, request.clone(), result);
        let newer = metadata_record(&c, "ws", "crossref", "Another newer revision");
        assert_eq!(newer["id"], original["id"]);
        assert_ne!(newer["revisionId"], original["revisionId"]);
        reconcile_task(&c, "ws", &task).unwrap();
        let list = list(&c, "ws", &p, 50, 0).unwrap();
        assert_eq!(list.len(), 1);
        let frozen = get(&c, "ws", &list[0].id).unwrap();
        let body = frozen.payload.as_ref().unwrap();
        assert_eq!(body["provider"], source);
        assert_eq!(body["records"][0]["revisionId"], original["revisionId"]);
        assert_eq!(
            body["records"][0]["metadataSha256"],
            original["revisionSha256"]
        );
        assert_eq!(body["fromYear"], 2019);
        assert_eq!(body["toYear"], 2021);
        assert!(!body.to_string().contains("synthetic query"));
        reconcile_task(&c, "ws", &task).unwrap();
        assert_eq!(
            get(&c, "ws", &frozen.id).unwrap().content_sha256,
            frozen.content_sha256
        );
        assert_eq!(
            super::super::literature_discovery::get(&c, "ws", original["id"].as_str().unwrap())
                .unwrap()
                .revision_id,
            newer["revisionId"]
        );
    }
}

#[test]
fn literature_source_or_revision_forgery_cannot_create_a_frozen_result() {
    let (c, p) = fixture();
    let row = metadata_record(&c, "ws", "europe_pmc", "Synthetic EU fixture");
    let foreign = metadata_record(&c, "other", "europe_pmc", "Synthetic foreign fixture");
    let request = json!({"provider":"europe_pmc","query":"synthetic query"});
    let valid = json!({"provider":"europe_pmc","records":[row]});
    for variant in 0..10 {
        let mut req = request.clone();
        let mut result = valid.clone();
        match variant {
            0 => result["provider"] = json!("crossref"),
            1 => req["provider"] = json!("unregistered"),
            2 => result["records"][0]["revisionSha256"] = json!("0".repeat(64)),
            3 => result["records"][0]["metadata"]["provider"] = json!("crossref"),
            4 => result["records"][0]["metadata"]["title"] = json!("forged"),
            5 => result["records"][0] = foreign.clone(),
            6 => result["records"][0]["revisionId"] = json!(Uuid::new_v4()),
            7 => {
                result["records"] =
                    json!([result["records"][0].clone(), result["records"][0].clone()])
            }
            8 => result["provider"] = Value::Null,
            _ => {
                result.as_object_mut().unwrap().remove("provider");
            }
        }
        let task = literature_task(&c, req, result);
        assert_eq!(
            reconcile_task(&c, "ws", &task).unwrap_err(),
            "literature_source_mismatch",
            "variant {variant}"
        );
        assert!(list(&c, "ws", &p, 50, 0).unwrap().is_empty());
    }
}

#[test]
fn legacy_literature_snapshot_is_read_verified_without_rewriting_history() {
    let (c, p) = fixture();
    let row = metadata_record(&c, "ws", "europe_pmc", "Synthetic legacy fixture");
    let task = literature_task(
        &c,
        json!({"provider":"europe_pmc","query":"synthetic"}),
        json!({"provider":"europe_pmc","records":[row]}),
    );
    let run = runs(&c, "ws", &p, 50, 0)
        .unwrap()
        .into_iter()
        .find(|r| r.background_task_id.as_deref() == Some(task.as_str()))
        .unwrap();
    let legacy = json!({"provider":"crossref","records":[],"legacyMarker":"preserve"});
    let id = create(
        &c,
        "ws",
        &p,
        "analysis_result",
        "Legacy search",
        &task,
        Some(&run.id),
        legacy.clone(),
    )
    .unwrap();
    let before = get(&c, "ws", &id).unwrap();
    reconcile_project(&c, "ws", &p).unwrap();
    let after = get(&c, "ws", &id).unwrap();
    assert_eq!(after.id, before.id);
    assert_eq!(after.content_sha256, before.content_sha256);
    assert_eq!(after.payload, Some(legacy));
    assert_eq!(list(&c, "ws", &p, 50, 0).unwrap().len(), 1);
    assert!(c
        .execute(
            "UPDATE research_artifacts SET title='rewritten' WHERE id=?1",
            [id]
        )
        .is_err());
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
