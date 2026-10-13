//! Opt-in synthetic workspace benchmark. No user data, network or model blobs.
use rusqlite::Connection;
use serde_json::json;
use std::time::Instant;
use suna::storage::{
    migrations::migrate,
    repositories::{research_artifacts as artifacts, research_projects as projects},
};
use suna::tasks::{repository, NewTask};

#[test]
#[ignore = "explicit synthetic performance benchmark"]
fn project_pages_and_durable_task_lookup_with_one_thousand_artifacts() {
    let mut c = Connection::open_in_memory().unwrap();
    migrate(&mut c).unwrap();
    let p = projects::save(
        &mut c,
        "benchmark",
        projects::SaveResearchProject {
            id: None,
            name: "合成性能基准".into(),
            description: "非科研事实".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )
    .unwrap();
    for n in 0..1000 {
        artifacts::create(
            &c,
            "benchmark",
            &p.id,
            "analysis_result",
            "合成成果",
            &format!("fixture:{n}"),
            None,
            json!({"synthetic":true,"ordinal":n}),
        )
        .unwrap();
    }
    let task = repository::create(
        &c,
        NewTask {
            workspace_id: "benchmark".into(),
            kind: "compute_analyze_microstructure".into(),
            payload_json: "{}".into(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .unwrap();
    // Simulate persisted age, not an actual 24-hour Worker execution.
    c.execute(
        "UPDATE background_tasks SET created_at='2026-01-01T00:00:00Z' WHERE id=?1",
        [task.id.to_string()],
    )
    .unwrap();
    let mut pages = vec![];
    let mut reads = vec![];
    for n in 0..100 {
        let t = Instant::now();
        let rows = artifacts::list(&c, "benchmark", &p.id, 25, (n % 40) * 25).unwrap();
        assert_eq!(rows.len(), 25);
        pages.push(t.elapsed().as_secs_f64() * 1000.);
        let t = Instant::now();
        assert_eq!(
            repository::get(&c, "benchmark", task.id)
                .unwrap()
                .unwrap()
                .state,
            suna::tasks::TaskState::Queued
        );
        reads.push(t.elapsed().as_secs_f64() * 1000.);
    }
    fn quantiles(mut values: Vec<f64>) -> serde_json::Value {
        values.sort_by(f64::total_cmp);
        json!({"p50Ms":values[49],"p95Ms":values[94],"maxMs":values[99]})
    }
    let report = json!({"fixture":"synthetic_in_memory_sqlite","os":std::env::consts::OS,"artifacts":1000,"pageSize":25,"iterations":100,"projectPage":quantiles(pages),"taskById":quantiles(reads),"simulatedTaskAgeOnly":true});
    println!("SUNA_RESEARCH_BENCHMARK={report}");
    if let Some(path) = std::env::var_os("SUNA_RESEARCH_BENCHMARK_REPORT") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
}
