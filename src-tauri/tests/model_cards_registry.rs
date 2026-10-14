use rusqlite::Connection;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use suna::storage::repositories::steel_models::{self as models, NewSteelModel};

fn database(path: &std::path::Path) -> Connection {
    let mut c = Connection::open(path).unwrap();
    suna::storage::migrations::migrate(&mut c).unwrap();
    c
}

fn artifact() -> String {
    json!({"model_type":"linear_regression","artifact_version":"linear-regression.v1","feature_names":["c"],"feature_schema":{"fields":[{"name":"c","unit":"wt%"}]},"target_schema":{"name":"hv","unit":"HV"},"data_version":"b".repeat(64),"split":{"kind":"time","train_indices":[0,1,2],"validation_indices":[3]},"metrics":{"test":{"rmse":2.5,"sample_count":3}},"private_path":"C:/private/raw.csv","model_pickle_base64":"synthetic-sensitive-blob"}).to_string()
}

fn new_model<'a>(artifact: &'a str, sha: &'a str) -> NewSteelModel<'a> {
    NewSteelModel {
        lineage_id: "linear:synthetic-dataset",
        kind: "linear_artifact",
        source_task_id: Some("11111111-1111-4111-8111-111111111111"),
        model_sha256: sha,
        manifest_json: artifact,
        artifact_json: Some(artifact),
        model_base64: None,
    }
}

#[test]
fn actual_registry_schema_persists_distinct_cards_and_reopens_without_raw_metadata() {
    let path = std::env::temp_dir().join(format!(
        "suna-isolated-card-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let mut c = database(&path);
    let artifact = artifact();
    let sha = format!("{:x}", Sha256::digest(artifact.as_bytes()));
    let context = json!({"dataset_id":"22222222-2222-4222-8222-222222222222","filtered_row_count":1,"valid_row_count":4,"source_row_count":5,"filter_policy":"exclude_missing_target_reject_invalid_numbers","time_field":"source_time"});
    let first =
        models::create_with_scientific_cards(&mut c, "ws", new_model(&artifact, &sha), &context)
            .unwrap();
    let second =
        models::create_with_scientific_cards(&mut c, "ws", new_model(&artifact, &sha), &context)
            .unwrap();
    assert_eq!((first.version, second.version), (1, 2));
    assert!(first.is_active);
    assert!(!second.is_active);
    let before = models::scientific_cards(&c, "ws", &first.id).unwrap();
    assert_eq!(before["cards"]["model_card"]["registry_id"], first.id);
    assert_eq!(before["cards"]["data_card"]["filtered_row_count"], 1);
    assert_eq!(before["persisted"], true);
    assert!(models::scientific_cards(&c, "another-workspace", &first.id).is_err());
    let version_two = models::scientific_cards(&c, "ws", &second.id).unwrap();
    assert_ne!(
        before["cards"]["cards_sha256"],
        version_two["cards"]["cards_sha256"]
    );
    assert_eq!(first.artifact_json.as_deref(), Some(artifact.as_str()));
    c.close().unwrap();
    let c = Connection::open(&path).unwrap();
    assert_eq!(
        models::scientific_cards(&c, "ws", &first.id).unwrap(),
        before
    );
    for forbidden in ["C:/private", "synthetic-sensitive-blob", "train_indices"] {
        assert!(!before.to_string().contains(forbidden));
    }
    c.close().unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn mismatched_artifact_sha_rolls_back_registration_and_version_allocation() {
    let mut c = database(std::path::Path::new(":memory:"));
    let artifact = artifact();
    let bad_sha = "0".repeat(64);
    assert_eq!(
        models::create_with_scientific_cards(
            &mut c,
            "ws",
            new_model(&artifact, &bad_sha),
            &json!({})
        )
        .unwrap_err(),
        "model_card_hash_mismatch"
    );
    assert!(models::list_all(&c, "ws").unwrap().is_empty());
    let sha = format!("{:x}", Sha256::digest(artifact.as_bytes()));
    assert_eq!(
        models::create_with_scientific_cards(&mut c, "ws", new_model(&artifact, &sha), &json!({}))
            .unwrap()
            .version,
        1
    );
}

#[test]
fn legacy_read_does_not_change_its_manifest_and_source_tampering_is_rejected() {
    let mut c = database(std::path::Path::new(":memory:"));
    let artifact = artifact();
    let sha = format!("{:x}", Sha256::digest(artifact.as_bytes()));
    let model = models::create(&mut c, "ws", new_model(&artifact, &sha)).unwrap();
    assert_eq!(
        models::scientific_cards(&c, "ws", &model.id).unwrap()["status"],
        "legacy_derived"
    );
    assert_eq!(
        models::get(&c, "ws", &model.id)
            .unwrap()
            .unwrap()
            .manifest_json,
        artifact
    );
    let mut tampered: Value = serde_json::from_str(&artifact).unwrap();
    tampered["metrics"]["test"]["rmse"] = json!(0.0);
    c.execute(
        "UPDATE steel_models SET artifact_json=?1 WHERE workspace_id='ws' AND id=?2",
        [tampered.to_string(), model.id.clone()],
    )
    .unwrap();
    assert_eq!(
        models::scientific_cards(&c, "ws", &model.id).unwrap_err(),
        "model_card_hash_mismatch"
    );
}

#[test]
fn existing_project_snapshot_keeps_its_historical_hash_when_registered_again() {
    use suna::storage::repositories::{
        research_artifacts as artifacts, research_projects as projects,
    };
    let mut c = database(std::path::Path::new(":memory:"));
    let body = artifact();
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    let model = models::create(
        &mut c,
        "ws",
        NewSteelModel {
            source_task_id: None,
            ..new_model(&body, &sha)
        },
    )
    .unwrap();
    let project = projects::save(
        &mut c,
        "ws",
        projects::SaveResearchProject {
            id: None,
            name: "合成旧成果".into(),
            description: String::new(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    c.execute(
        "INSERT INTO settings VALUES('ws','suna.research.current_project',?1,'now')",
        [json!(project.id).to_string()],
    )
    .unwrap();
    let payload = json!({"model_id":model.id,"model_sha256":model.model_sha256,"kind":model.kind,"version":model.version,"training_task_id":null,"manifest":artifacts::safe_metadata(&serde_json::from_str::<Value>(&body).unwrap())});
    let key = format!("registered:{}", model.id);
    let id = artifacts::create(
        &c,
        "ws",
        &project.id,
        "model_snapshot",
        "注册模型版本",
        &key,
        None,
        payload.clone(),
    )
    .unwrap();
    let old_hash = artifacts::get(&c, "ws", &id).unwrap().content_sha256;
    artifacts::registered_model(&c, "ws", &model.id).unwrap();
    artifacts::registered_model(&c, "ws", &model.id).unwrap();
    let saved = artifacts::get(&c, "ws", &id).unwrap();
    assert_eq!(saved.content_sha256, old_hash);
    assert_eq!(saved.payload.unwrap(), payload);
}
