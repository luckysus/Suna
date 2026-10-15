use super::{dry_run_on_connection, runtime_support::*, templates, testing::fixture_with_csv};
use crate::storage::repositories::{research_artifacts, steel_models};
use serde_json::{json, Value};

fn configured() -> super::testing::Fixture {
    let mut csv = String::from("C,temp,HV,J,time\n");
    for i in 0..120 {
        let x = f64::from(i);
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            0.1 + x * 0.001,
            700.0 + x * 0.2,
            100.0 + x * 1.5 + x.sin(),
            50.0 + x * 0.4 + x.cos(),
            i + 1
        ));
    }
    let mut f = fixture_with_csv(&csv);
    f.connection
        .execute_batch("PRAGMA foreign_keys=ON;PRAGMA journal_mode=WAL")
        .unwrap();
    f.template.variables = vec![
        json!({"name":"c","unit":"wt%","min":0.1,"max":0.22}),
        json!({"name":"temperature","unit":"degC","min":700,"max":724}),
    ];
    f.template.models[0].split_policy = json!({"kind":"random","seed":17,"validationFraction":0.2,"folds":3,"testFraction":0.2,"calibrationFraction":0.2,"searchTrials":0,"learningCurve":true});
    f.template.models[1].split_policy = json!({"kind":"time","timeColumn":4,"seed":31,"validationFraction":0.2,"folds":2,"testFraction":0.2,"calibrationFraction":0.2,"searchTrials":1,"learningCurve":true});
    f
}

#[test]
fn independent_scientific_policies_reach_real_worker_and_verified_cards_after_reopen() {
    let mut f = configured();
    let saved = templates::save(&mut f.connection, "local", &f.template, 0).unwrap();
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    let mut changed = f.template.clone();
    changed.models[0].split_policy["calibrationFraction"] = json!(0.15);
    let changed_preview =
        dry_run_on_connection(&f.connection, "local", &changed, &f.input).unwrap();
    assert_ne!(changed_preview["dryRunSha256"], preview["dryRunSha256"]);
    assert_ne!(
        changed_preview["validatedModels"][0]["trainingPayloadSha256"],
        preview["validatedModels"][0]["trainingPayloadSha256"]
    );
    let root = submit(&mut f, uuid::Uuid::new_v4());
    let path = f.root.join("工作流.sqlite3");
    let mut scheduler = new_scheduler(&path);
    let done = completed(&mut scheduler, &f.connection, root.id);
    drain(&mut scheduler);
    drop(scheduler);
    let cp: Value = serde_json::from_str(done.checkpoint_json.as_deref().unwrap()).unwrap();
    let reopened = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        templates::get(&reopened, "local", &f.template.id, 1).unwrap(),
        saved
    );
    for index in 0..2 {
        let task = record(
            &reopened,
            uuid::Uuid::parse_str(cp["children"][index].as_str().unwrap()).unwrap(),
        );
        let request: Value = serde_json::from_str(&task.payload_json).unwrap();
        let policy = &f.template.models[index].split_policy;
        assert_eq!(request["payload"]["split_policy"]["kind"], policy["kind"]);
        assert_eq!(request["payload"]["split_policy"]["seed"], policy["seed"]);
        for (worker, frontend) in [
            ("folds", "folds"),
            ("test_fraction", "testFraction"),
            ("calibration_fraction", "calibrationFraction"),
            ("search_trials", "searchTrials"),
            ("learning_curve", "learningCurve"),
        ] {
            assert_eq!(request["payload"]["evaluation"][worker], policy[frontend]);
        }
        let id = cp["models"][index].as_str().unwrap();
        let model = steel_models::get(&reopened, "local", id).unwrap().unwrap();
        assert!(!model.is_active);
        let a: Value = serde_json::from_str(model.artifact_json.as_deref().unwrap()).unwrap();
        let split = &a["split"];
        let mut rows = std::collections::HashSet::new();
        for partition in [
            "train_indices",
            "validation_indices",
            "test_indices",
            "calibration_indices",
        ] {
            for row in split[partition].as_array().unwrap() {
                assert!(
                    rows.insert(row.as_u64().unwrap()),
                    "fit/validation/test/calibration partitions must never overlap"
                );
            }
        }
        assert_eq!(rows.len(), 120);
        assert_eq!(split["test_indices"].as_array().unwrap().len(), 24);
        assert_eq!(split["calibration_indices"].as_array().unwrap().len(), 24);
        let held_out = split["test_indices"]
            .as_array()
            .unwrap()
            .iter()
            .chain(split["calibration_indices"].as_array().unwrap())
            .map(|v| v.as_u64().unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            a["evaluation"]["folds"].as_array().unwrap().len(),
            policy["folds"].as_u64().unwrap() as usize
        );
        for fold in a["evaluation"]["folds"].as_array().unwrap() {
            for key in ["train_indices", "validation_indices"] {
                assert!(fold[key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|v| !held_out.contains(&v.as_u64().unwrap())));
            }
        }
        assert_eq!(
            a["evaluation"]["learning_curve"].as_array().unwrap().len(),
            3
        );
        let cards = steel_models::scientific_cards(&reopened, "local", id).unwrap();
        let card = &cards["cards"]["model_card"];
        assert_eq!(card["random_seed"], policy["seed"]);
        assert_eq!(card["split_kind"], policy["kind"]);
        assert_eq!(card["split_counts"]["test"], 24);
        assert_eq!(card["split_counts"]["calibration"], 24);
        assert_eq!(card["metrics"]["test"]["sample_count"], 24);
        assert_eq!(card["interval"]["exchangeability_verified"], false);
        assert_eq!(
            card["interval"]["observed_test_coverage"],
            a["evaluation"]["test_interval_coverage"]
        );
        assert_eq!(
            cards["cards"]["data_card"]["dataset_sha256"],
            f.input.dataset_sha256
        );
    }
    let summary =
        research_artifacts::get(&reopened, "local", cp["resultArtifactId"].as_str().unwrap())
            .unwrap();
    assert_eq!(
        summary.payload.as_ref().unwrap()["templateContentSha256"],
        saved["contentSha256"]
    );
    assert_eq!(preview["validatedModels"][0]["validRowCount"], 120);
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    drop(reopened);
}

#[test]
fn invalid_evaluation_blocks_preview_without_erasing_saved_history_or_creating_tasks() {
    let mut f = configured();
    f.template.models[1].split_policy["searchTrials"] = json!(14);
    // Older schema-only saves allowed this value. They must remain inspectable;
    // execution preflight applies the Worker's actual limit before any jobs.
    let saved = templates::save(&mut f.connection, "local", &f.template, 0).unwrap();
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_training_configuration_invalid"
    );
    assert_eq!(
        templates::get(&f.connection, "local", &f.template.id, 1).unwrap(),
        saved
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn advanced_preview_counts_valid_targets_instead_of_raw_source_rows() {
    let mut csv = String::from("C,temp,HV,J,time\n");
    for i in 0..12 {
        let hv = if i % 3 == 0 {
            String::new()
        } else {
            (100 + i).to_string()
        };
        csv.push_str(&format!(
            "{},{},{},{},{}\n",
            0.1 + f64::from(i) * 0.01,
            700 + i,
            hv,
            50 + i,
            i + 1
        ));
    }
    let mut f = fixture_with_csv(&csv);
    f.template.models[0].split_policy["folds"] = json!(2);
    let saved = templates::save(&mut f.connection, "local", &f.template, 0).unwrap();
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_scientific_evaluation_requires_10_valid_rows"
    );
    assert_eq!(
        templates::get(&f.connection, "local", &f.template.id, 1).unwrap(),
        saved
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}
