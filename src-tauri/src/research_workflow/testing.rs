use super::*;
use crate::storage::repositories::steel::DatasetColumnMapping;
use std::path::PathBuf;

pub(crate) struct Fixture {
    pub(crate) root: PathBuf,
    pub(crate) connection: Connection,
    pub(crate) input: schema::FrozenInput,
    pub(crate) template: schema::Template,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Windows cannot remove an open SQLite database or its WAL files.
        let old = std::mem::replace(
            &mut self.connection,
            Connection::open_in_memory().expect("cleanup connection"),
        );
        old.close().expect("close fixture database before cleanup");
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
pub(crate) fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "suna-workflow-dry-run-中文-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut connection = Connection::open(root.join("工作流.sqlite3")).unwrap();
    crate::storage::migrations::migrate(&mut connection).unwrap();
    let project = projects::save(
        &mut connection,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "合成工作流".into(),
            description: "非真实测量".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    let source = root.join("合成 数据.csv");
    std::fs::write(&source,"C,temp,HV,J,time\n0.1,700,100,50,1\n0.2,710,110,52,2\n0.3,720,120,54,3\n0.4,730,130,56,4\n0.5,740,140,58,5\n0.6,750,150,60,6\n").unwrap();
    let path = source.to_string_lossy().to_string();
    let sha = steel::hash_dataset_source(&path).unwrap();
    let table = steel::preview_dataset(&steel::DatasetPreviewRequest {
        source_path: path.clone(),
        sheet: None,
    })
    .unwrap();
    let mappings = vec![
        ("c", "wt%"),
        ("temperature", "degC"),
        ("hv", "HV"),
        ("j", "J"),
        ("time", "s"),
    ]
    .iter()
    .enumerate()
    .map(|(ordinal, (name, unit))| DatasetColumnMapping {
        ordinal,
        canonical_field: Some((*name).into()),
        unit: Some((*unit).into()),
    })
    .collect::<Vec<_>>();
    let dataset =
        datasets::save_preview(&mut connection, "local", &path, &sha, &table, &mappings).unwrap();
    datasets::activate(&mut connection, "local", &dataset.id).unwrap();
    let template: schema::Template=serde_json::from_value(json!({"schemaVersion":"research-workflow-template.v1","id":uuid::Uuid::new_v4(),"revision":1,"name":"双模型优化实验计划",
        "models":[{"name":"HV","algorithm":"linear_regression","featureColumns":[0,1],"targetColumn":2,"direction":"maximize","unit":"HV","splitPolicy":{"kind":"time","timeColumn":4,"validationFraction":0.2}},
            {"name":"J","algorithm":"random_forest","featureColumns":[1,0],"targetColumn":3,"direction":"maximize","unit":"J","splitPolicy":{"kind":"time","timeColumn":4,"validationFraction":0.2}}],
        "variables":[{"name":"c","unit":"fraction","min":0.001,"max":0.006},{"name":"temperature","unit":"K","min":973.15,"max":1023.15}],"constraints":[],"trials":20,"seed":42,"timeoutSeconds":300})).unwrap();
    Fixture {
        root,
        connection,
        input: schema::FrozenInput {
            project_id: project.id,
            dataset_id: dataset.id,
            dataset_sha256: sha,
        },
        template,
    }
}
#[test]
fn actual_dataset_and_models_are_validated_without_creating_jobs_or_mutating_selection() {
    let f = fixture();
    let before: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    let preview = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    let after: i64 = f
        .connection
        .query_row("SELECT total_changes()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        preview["validationStatus"],
        "repository_validated_read_only"
    );
    assert_eq!(preview["networkRequired"], false);
    assert_eq!(
        preview["validatedModels"][0]["featureNames"],
        json!(["c", "temperature"])
    );
    assert_eq!(
        preview["validatedModels"][1]["featureNames"],
        json!(["temperature", "c"])
    );
    assert_eq!(preview["nodes"][3]["state"], "proposed");
    assert_eq!(preview["nodes"][3]["measurementsCreated"], false);
    assert_eq!(preview["nodes"][3]["confirmationRequiredToAccept"], true);
    assert!(!preview
        .to_string()
        .contains(&f.root.to_string_lossy().to_string()));
    assert_eq!(
        f.connection
            .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn missing_model_variables_and_false_units_cannot_pass_business_dry_run() {
    let mut f = fixture();
    f.template.variables.pop();
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
    let mut f = fixture();
    f.template.models[0].unit = Some("HRC".into());
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
}
#[test]
fn changed_csv_and_wrong_project_or_workspace_are_rejected() {
    let f = fixture();
    assert!(
        dry_run_on_connection(&f.connection, "another-workspace", &f.template, &f.input).is_err()
    );
    let mut f = fixture();
    f.input.project_id = uuid::Uuid::new_v4().to_string();
    assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
    let f = fixture();
    std::fs::write(f.root.join("合成 数据.csv"), "changed").unwrap();
    assert_eq!(
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap_err(),
        "workflow_source_changed"
    );
}
#[test]
fn identical_source_is_reproducible_and_configuration_changes_are_hash_bound() {
    let mut f = fixture();
    let first = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    let second = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
    assert_eq!(first["dryRunSha256"], second["dryRunSha256"]);
    f.template.models[1].direction = "minimize".into();
    assert_ne!(
        first["dryRunSha256"],
        dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap()
            ["dryRunSha256"]
    );
}
