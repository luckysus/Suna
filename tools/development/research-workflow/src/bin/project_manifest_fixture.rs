//! Produce a manifest with the actual Suna repositories for isolated archive tests.
use rusqlite::Connection;
use serde_json::json;
use suna::storage::repositories::{
    research_artifacts as artifacts, research_delivery as delivery, research_projects as projects,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .ok_or("synthetic output path required")?;
    let mut connection = Connection::open_in_memory()?;
    suna::storage::migrations::migrate(&mut connection)?;
    let project = projects::save(
        &mut connection,
        "local",
        projects::SaveResearchProject {
            id: None,
            name: "千对象合成归档".into(),
            description: "synthetic-private-description".into(),
            material_system: None,
            tags: vec![],
            expected_updated_at: None,
        },
    )?;
    let mut previous = None;
    for i in 0..1100 {
        let artifact = artifacts::create(
            &connection,
            "local",
            &project.id,
            "analysis_result",
            "合成成果",
            &format!("synthetic-{i}"),
            None,
            json!({"notes":"synthetic-only","source_path":"C:/synthetic-private/raw.csv","raw_rows":[987.123456],"api_key":"synthetic-secret"}),
        )?;
        if i < 5 {
            if let Some(from) = previous.as_deref() {
                artifacts::edge(&connection, "local", from, &artifact, "generated_by_run")?;
            }
        }
        previous = Some(artifact);
    }
    let manifest = delivery::manifest(&connection, "local", &project.id)?;
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    use std::io::Write;
    file.write_all(&bytes)?;
    println!("Actual repository fixture exported: 1100 artifacts, project links and 4 DAG edges");
    Ok(())
}
