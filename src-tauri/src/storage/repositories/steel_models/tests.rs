use super::{create, delete, get, list, list_all, set_active, NewSteelModel};
use crate::storage::migrations::migrate;
use rusqlite::Connection;

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().expect("open database");
    migrate(&mut connection).expect("migrate database");
    connection
}

fn sample(kind: &'static str, manifest: &'static str) -> NewSteelModel<'static> {
    NewSteelModel {
        lineage_id: "sklearn:dataset-1",
        kind,
        source_task_id: Some("task-1"),
        model_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        manifest_json: manifest,
        artifact_json: Some(r#"{"model_type":"mlp","metrics":{}}"#),
        model_base64: None,
    }
}

#[test]
fn accepts_transformer_artifacts_and_lists_across_lineages() {
    let mut connection = database();
    let transformer = create(
        &mut connection,
        "ws",
        NewSteelModel {
            lineage_id: "transformer:dataset-1",
            ..sample("transformer_artifact", r#"{"model_id":"t1"}"#)
        },
    )
    .expect("create transformer model");
    assert_eq!(transformer.kind, "transformer_artifact");
    assert!(transformer.is_active);

    create(
        &mut connection,
        "ws",
        sample("sklearn_artifact", r#"{"model_id":"s1"}"#),
    )
    .expect("create sklearn model");

    let all = list_all(&connection, "ws").expect("list all models");
    assert_eq!(all.len(), 2);
    assert!(all
        .iter()
        .any(|model| model.lineage_id == "transformer:dataset-1"));
    assert!(all
        .iter()
        .any(|model| model.lineage_id == "sklearn:dataset-1"));
    // 同 lineage 内版本倒序。
    let scoped = list(&connection, "ws", "transformer:dataset-1").expect("list lineage");
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, transformer.id);
}

#[test]
fn transformer_versions_require_an_artifact_and_reject_blobs() {
    let mut connection = database();
    let mut model = sample("transformer_artifact", r#"{"model_id":"t1"}"#);
    model.artifact_json = None;
    assert!(create(&mut connection, "ws", model).is_err());

    let mut blobbed = sample("transformer_artifact", r#"{"model_id":"t1"}"#);
    blobbed.model_base64 = Some("AAAA");
    assert!(create(&mut connection, "ws", blobbed).is_err());
}

#[test]
fn active_versions_switch_within_a_lineage_and_block_deletion() {
    let mut connection = database();
    let first = create(
        &mut connection,
        "ws",
        sample("sklearn_artifact", r#"{"v":1}"#),
    )
    .expect("create first version");
    let mut second_manifest = sample("sklearn_artifact", r#"{"v":2}"#);
    second_manifest.lineage_id = "sklearn:dataset-1";
    let second = create(&mut connection, "ws", second_manifest).expect("create second version");
    assert_eq!(second.version, 2);
    // 只有 version 1 自动激活；后续版本要显式切换。
    assert!(first.is_active);
    assert!(!second.is_active);

    let activated = set_active(&mut connection, "ws", &second.id).expect("activate second");
    assert!(activated.is_active);
    assert!(
        !get(&connection, "ws", &first.id)
            .unwrap()
            .unwrap()
            .is_active
    );

    // 活动版本不可删除，切换后旧版本可以。
    assert!(delete(&mut connection, "ws", &second.id).is_err());
    set_active(&mut connection, "ws", &first.id).expect("reactivate first");
    assert!(delete(&mut connection, "ws", &second.id).is_ok());
}

#[test]
fn registration_fence_reads_cancellation_inside_transaction_without_creating_a_model() {
    use crate::tasks::{repository, NewTask};
    let mut c = database();
    let parent = repository::create(
        &c,
        NewTask {
            workspace_id: "ws".into(),
            kind: "research_modeling_workflow".into(),
            payload_json: "{}".into(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .unwrap();
    let claimed = repository::claim_next(&mut c, "ws", "2099-01-01T00:00:00Z")
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, parent.id);
    // A cached preflight said running; cancellation won before registration.
    repository::request_running_cancellation(&mut c, "ws", parent.id, claimed.attempt).unwrap();
    let guard = |c: &Connection| {
        assert!(!c.is_autocommit());
        let current = repository::get(c, "ws", parent.id).unwrap().unwrap();
        if current.cancel_requested {
            Err("workflow_checkpoint_stale".into())
        } else {
            Ok(())
        }
    };
    let result = super::create_inner(
        &mut c,
        "ws",
        sample("sklearn_artifact", r#"{"v":1}"#),
        None,
        super::RegistrationPolicy {
            activate_first: false,
            guard: Some(&guard),
        },
    );
    assert_eq!(result.unwrap_err(), "workflow_checkpoint_stale");
    assert!(list_all(&c, "ws").unwrap().is_empty());
    assert!(c.is_autocommit());
}

#[test]
fn workflow_registration_remains_inactive_and_rechecks_permission_before_insert() {
    let mut c = database();
    let old = create(&mut c, "ws", sample("sklearn_artifact", r#"{"v":1}"#)).unwrap();
    let calls = std::cell::Cell::new(0);
    let guard = |c: &Connection| {
        assert!(!c.is_autocommit());
        calls.set(calls.get() + 1);
        Ok(())
    };
    let mut next = sample("sklearn_artifact", r#"{"v":2}"#);
    next.lineage_id = "workflow:synthetic";
    let new = super::create_inner(
        &mut c,
        "ws",
        next,
        None,
        super::RegistrationPolicy {
            activate_first: false,
            guard: Some(&guard),
        },
    )
    .unwrap();
    assert!(!new.is_active);
    assert_eq!(calls.get(), 2);
    assert!(get(&c, "ws", &old.id).unwrap().unwrap().is_active);
    let deny = |_: &Connection| {
        calls.set(calls.get() + 1);
        if calls.get() == 4 {
            Err("workflow_timeout_requires_new_confirmed_run".into())
        } else {
            Ok(())
        }
    };
    let error = super::create_inner(
        &mut c,
        "ws",
        sample("sklearn_artifact", r#"{"v":3}"#),
        None,
        super::RegistrationPolicy {
            activate_first: false,
            guard: Some(&deny),
        },
    )
    .unwrap_err();
    assert_eq!(error, "workflow_timeout_requires_new_confirmed_run");
    assert_eq!(list_all(&c, "ws").unwrap().len(), 2);
}
