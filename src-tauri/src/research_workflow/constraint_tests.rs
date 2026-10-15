use super::{dry_run_on_connection, runtime_support::*, schema, templates};
use crate::storage::repositories::{experiment_measurements, experiments, research_artifacts};
use rusqlite::Connection;
use serde_json::{json, Value};

#[test]
fn actual_fixed_mixed_unit_workflow_keeps_constraints_and_plan_units_after_reopen() {
    let (mut f, path) = setup();
    let original = templates::get(&f.connection, "local", &f.template.id, 1).unwrap();
    f.template.variables = vec![
        json!({"name":"c","unit":"fraction","min":0.001,"max":0.006,"fixedValue":0.003}),
        json!({"name":"temperature","unit":"K","min":973.15,"max":1023.15}),
    ];
    f.template.constraints = vec![
        json!({"kind":"equality","coefficients":{"c":1000,"temperature":1},"value":998.15,"tolerance":1e-6}),
        json!({"kind":"inequality","coefficients":{"temperature":-1},"value":-1000,"tolerance":1e-6}),
    ];
    let saved = templates::save(&mut f.connection, "local", &f.template, 1).unwrap();
    f.template = serde_json::from_value::<schema::Template>(saved["template"].clone()).unwrap();
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    assert_eq!(
        preview["nodes"][2]["constraints"],
        json!(f.template.constraints)
    );
    assert_eq!(
        preview["nodes"][2]["variables"],
        json!(f.template.variables)
    );
    let parent = submit(&mut f, uuid::Uuid::new_v4());
    let mut scheduler = new_scheduler(&path);
    let done = completed(&mut scheduler, &f.connection, parent.id);
    drain(&mut scheduler);
    drop(scheduler);
    let checkpoint: Value = serde_json::from_str(done.checkpoint_json.as_deref().unwrap()).unwrap();
    let optimization = record(
        &f.connection,
        uuid::Uuid::parse_str(checkpoint["optimization"].as_str().unwrap()).unwrap(),
    );
    let input: Value = serde_json::from_str(&optimization.payload_json).unwrap();
    assert_eq!(
        input["payload"]["constraints"],
        json!(f.template.constraints)
    );
    assert_eq!(input["payload"]["fixed_values"]["c"], 0.003);
    let result: Value =
        serde_json::from_str(optimization.checkpoint_json.as_deref().unwrap()).unwrap();
    let front = result["result"]["pareto_front"].as_array().unwrap();
    assert!(!front.is_empty());
    for candidate in front {
        let c = candidate["values"]["c"].as_f64().unwrap();
        let kelvin = candidate["values"]["temperature"].as_f64().unwrap();
        assert_eq!(c, 0.003);
        assert!((kelvin + 1000. * c - 998.15).abs() <= 1e-6);
        assert!(kelvin <= 1000. + 1e-6);
        assert_eq!(candidate["feasible"], true);
    }
    let artifact_id: String = f
        .connection
        .query_row(
            "SELECT id FROM research_artifacts WHERE kind='optimization_result' AND source_key=?1",
            [optimization.id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    let frozen = research_artifacts::get(&f.connection, "local", &artifact_id).unwrap();
    assert_eq!(
        frozen.payload.as_ref().unwrap()["request"]["variables"],
        json!([
            {"name":"c","unit":"fraction"},{"name":"temperature","unit":"K"}
        ])
    );
    let plan_id = checkpoint["plan"].as_str().unwrap();
    let plan = experiments::get(&f.connection, "local", plan_id)
        .unwrap()
        .unwrap();
    let variables: Value = serde_json::from_str(&plan.variables_json).unwrap();
    for (name, unit) in [("c", "fraction"), ("temperature", "K")] {
        assert_eq!(
            variables
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == name)
                .unwrap()["unit"],
            unit
        );
    }
    assert_eq!(plan.state, "proposed");
    assert_eq!(
        templates::get(&f.connection, "local", &f.template.id, 1).unwrap(),
        original
    );
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM experiment_measurements", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        f.connection
            .query_row(
                "SELECT COUNT(*) FROM steel_models WHERE is_active=1",
                [],
                |r| r.get::<_, u64>(0)
            )
            .unwrap(),
        0
    );
    // Repeat from the frozen artifact after the source DB connection is closed.
    let old = std::mem::replace(&mut f.connection, Connection::open_in_memory().unwrap());
    old.close().unwrap();
    let reopened = Connection::open(&path).unwrap();
    assert_eq!(
        templates::get(&reopened, "local", &f.template.id, 2).unwrap(),
        saved
    );
    assert_eq!(
        research_artifacts::get(&reopened, "local", &artifact_id)
            .unwrap()
            .content_sha256,
        frozen.content_sha256
    );
    assert_eq!(
        experiments::get(&reopened, "local", plan_id)
            .unwrap()
            .unwrap()
            .variables_json,
        plan.variables_json
    );
    let copy = experiment_measurements::candidate_plan(
        &reopened,
        "local",
        &f.input.project_id,
        &artifact_id,
        0,
        "合成单位复核",
    )
    .unwrap();
    assert_eq!(copy.variables_json, plan.variables_json);
    // New frozen units must not be guessed for old records that never had them.
    let mut legacy = frozen.payload.unwrap();
    legacy["request"]
        .as_object_mut()
        .unwrap()
        .remove("variables");
    let legacy_id = research_artifacts::create(
        &reopened,
        "local",
        &f.input.project_id,
        "optimization_result",
        "合成历史元数据",
        "synthetic-legacy-no-units",
        None,
        legacy,
    )
    .unwrap();
    let legacy_plan = experiment_measurements::candidate_plan(
        &reopened,
        "local",
        &f.input.project_id,
        &legacy_id,
        0,
        "合成旧单位未知",
    )
    .unwrap();
    let legacy_variables: Value = serde_json::from_str(&legacy_plan.variables_json).unwrap();
    assert!(legacy_variables
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v["unit"].is_null()));
    drop(reopened);
}

#[test]
fn manual_constraints_are_checked_together_with_reviewed_project_rules() {
    let (mut f, _) = setup();
    f.template.variables[0]["fieldId"] = json!("chem.c");
    f.template.constraints =
        vec![json!({"kind":"inequality","coefficients":{"c":1},"value":0.4,"tolerance":1e-6})];
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_ok());
    let rule: crate::steel::materials::Rule = serde_json::from_value(json!({
        "id":"synthetic-carbon-bound","name":"合成成分上限","severity":"hard","enabled":true,
        "penalty":1,"provenance":{"type":"equipment","sourceRef":"synthetic","sourceVersion":"1","reviewed":true},
        "expression":{"kind":"range","fieldId":"chem.c","unit":"wt%","max":0.35}
    })).unwrap();
    crate::storage::repositories::material_rules::save(
        &f.connection,
        "local",
        &f.input.project_id,
        rule,
        0,
    )
    .unwrap();
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_system_infeasible"
    );
    f.template.constraints.clear();
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_ok());
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn constraint_preflight_rejects_unsupported_budgets_and_overflow_without_truncation() {
    let (mut f, _) = setup();
    let constraint =
        json!({"kind":"inequality","coefficients":{"c":1},"value":0.1,"tolerance":1e-6});
    f.template.constraints = vec![constraint.clone(); 65];
    // Historical versions remain readable; a non-executable system cannot start.
    let historical = templates::save(&mut f.connection, "local", &f.template, 1).unwrap();
    assert_eq!(
        historical["template"]["constraints"]
            .as_array()
            .unwrap()
            .len(),
        65
    );
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_budget"
    );
    f.template.constraints = vec![constraint];
    let original = f.template.variables.clone();
    for i in 0..23 {
        f.template
            .variables
            .push(json!({"name":format!("aux-{i}"),"min":0,"max":1}));
    }
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_preflight_variable_budget"
    );
    f.template.variables = original;
    f.template.constraints[0]["value"] = json!(-f64::MAX);
    f.template.constraints[0]["tolerance"] = json!(f64::MAX);
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_preflight_overflow"
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
fn contradictory_constraints_and_fixed_values_fail_before_any_jobs() {
    let (mut f, _) = setup();
    f.template.variables[0]["fixedValue"] = json!(0.3);
    f.template.constraints =
        vec![json!({"kind":"inequality","coefficients":{"c":1},"value":0.4,"tolerance":1e-6})];
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_system_infeasible"
    );
    f.template.constraints[0]["value"] = json!(0.3);
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_ok());
    // Feasibility uses the same tolerance convention as the Worker, not exact equality.
    f.template.constraints =
        vec![json!({"kind":"equality","coefficients":{"c":0},"value":5e-7,"tolerance":1e-6})];
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_ok());
    f.template.constraints[0]["value"] = json!(2e-6);
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_constraint_system_infeasible"
    );
    f.template.constraints[0]["coefficients"] = json!({"removed_variable":1});
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
    f.template.constraints.clear();
    f.template.variables[0]["fixedValue"] = json!(0.7);
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        templates::get(&f.connection, "local", &f.template.id, 1).unwrap()["template"]
            ["constraints"],
        json!([])
    );
}
