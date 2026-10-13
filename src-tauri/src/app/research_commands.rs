use crate::db::{current_workspace_id, with_conn, with_conn_mut, DbState};
use crate::storage::repositories::{research_projects as projects, settings};
use projects::{ResearchObject, ResearchProject, ResearchSummary, SaveResearchProject};
use serde::Deserialize;

#[tauri::command]
pub fn save_research_project(
    db: tauri::State<DbState>,
    request: SaveResearchProject,
) -> Result<ResearchProject, String> {
    with_conn_mut(&db, |c| projects::save(c, current_workspace_id(), request))
}
#[tauri::command]
pub fn get_research_project(
    db: tauri::State<DbState>,
    id: String,
) -> Result<ResearchProject, String> {
    with_conn(&db, |c| projects::get(c, current_workspace_id(), &id))
}
#[tauri::command]
pub fn list_research_projects(
    db: tauri::State<DbState>,
    search: Option<String>,
    status: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchProject>, String> {
    with_conn(&db, |c| {
        projects::list(
            c,
            current_workspace_id(),
            search.as_deref().unwrap_or(""),
            status.as_deref(),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn archive_research_project(
    db: tauri::State<DbState>,
    id: String,
    archived: bool,
) -> Result<ResearchProject, String> {
    with_conn_mut(&db, |c| {
        projects::archive(c, current_workspace_id(), &id, archived)
    })
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchObjectRequest {
    pub project_id: String,
    pub kind: String,
    pub id: String,
    #[serde(default = "attached")]
    pub relation: String,
}
fn attached() -> String {
    "attached".into()
}
#[tauri::command]
pub fn attach_research_object(
    db: tauri::State<DbState>,
    request: ResearchObjectRequest,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        projects::attach(
            c,
            current_workspace_id(),
            &request.project_id,
            &request.kind,
            &request.id,
            &request.relation,
        )
    })
}
#[tauri::command]
pub fn detach_research_object(
    db: tauri::State<DbState>,
    request: ResearchObjectRequest,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        projects::detach(
            c,
            current_workspace_id(),
            &request.project_id,
            &request.kind,
            &request.id,
            &request.relation,
        )
    })
}
#[tauri::command]
pub fn list_research_project_objects(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchObject>, String> {
    with_conn(&db, |c| {
        projects::objects(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn list_research_attachable_objects(
    db: tauri::State<DbState>,
    kind: String,
    search: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<ResearchObject>, String> {
    with_conn(&db, |c| {
        projects::available(
            c,
            current_workspace_id(),
            &kind,
            search.as_deref().unwrap_or(""),
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_project_summary(
    db: tauri::State<DbState>,
    id: String,
) -> Result<ResearchSummary, String> {
    with_conn(&db, |c| projects::summary(c, current_workspace_id(), &id))
}
const CURRENT_PROJECT: &str = "suna.research.current_project";
#[tauri::command]
pub fn get_current_research_project(
    db: tauri::State<DbState>,
) -> Result<Option<ResearchProject>, String> {
    with_conn(&db, |c| {
        let saved = settings::get(c, current_workspace_id(), CURRENT_PROJECT)?;
        let id: Option<String> = saved.and_then(|s| serde_json::from_str(&s).ok());
        id.map(|id| projects::get(c, current_workspace_id(), &id))
            .transpose()
    })
}
#[tauri::command]
pub fn set_current_research_project(
    db: tauri::State<DbState>,
    id: Option<String>,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        if let Some(id) = &id {
            projects::require_active(c, current_workspace_id(), id)?;
        }
        settings::set(
            c,
            current_workspace_id(),
            CURRENT_PROJECT,
            &serde_json::to_string(&id).map_err(|e| e.to_string())?,
        )
    })
}

use crate::storage::repositories::research_artifacts as artifacts;
#[tauri::command]
pub fn list_research_runs(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Run>, String> {
    with_conn(&db, |c| {
        artifacts::reconcile_project(c, current_workspace_id(), &project_id)?;
        artifacts::runs(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn list_research_artifacts(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Artifact>, String> {
    with_conn(&db, |c| {
        artifacts::list(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn get_research_artifact(
    db: tauri::State<DbState>,
    id: String,
) -> Result<artifacts::Artifact, String> {
    with_conn(&db, |c| artifacts::get(c, current_workspace_id(), &id))
}
#[tauri::command]
pub fn list_research_artifact_edges(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<artifacts::Edge>, String> {
    with_conn(&db, |c| {
        artifacts::edges(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
        )
    })
}

use crate::storage::repositories::experiment_measurements as measurements;
#[tauri::command]
pub fn record_experiment_execution(
    db: tauri::State<DbState>,
    request: measurements::RecordExecution,
) -> Result<measurements::Execution, String> {
    with_conn(&db, |c| {
        measurements::record(c, current_workspace_id(), request)
    })
}
#[tauri::command]
pub fn list_experiment_executions(
    db: tauri::State<DbState>,
    project_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<measurements::Execution>, String> {
    with_conn(&db, |c| {
        measurements::list(
            c,
            current_workspace_id(),
            &project_id,
            limit.unwrap_or(25),
            offset.unwrap_or(0),
        )
    })
}
#[tauri::command]
pub fn review_experiment_execution(
    db: tauri::State<DbState>,
    id: String,
    state: String,
    reason: String,
    expected_state: String,
) -> Result<measurements::Execution, String> {
    with_conn(&db, |c| {
        measurements::review(
            c,
            current_workspace_id(),
            &id,
            &state,
            &reason,
            &expected_state,
        )
    })
}
#[tauri::command]
pub fn create_research_candidate_plan(
    db: tauri::State<DbState>,
    project_id: String,
    artifact_id: String,
    index: usize,
    title: String,
) -> Result<crate::storage::repositories::experiments::ExperimentRecord, String> {
    with_conn(&db, |c| {
        measurements::candidate_plan(
            c,
            current_workspace_id(),
            &project_id,
            &artifact_id,
            index,
            &title,
        )
    })
}
#[tauri::command]
pub fn compare_experiment_predictions(
    db: tauri::State<DbState>,
    id: String,
    artifact_id: String,
    candidate: usize,
) -> Result<Vec<measurements::Comparison>, String> {
    with_conn(&db, |c| {
        measurements::compare(c, current_workspace_id(), &id, &artifact_id, candidate)
    })
}
#[tauri::command]
pub fn create_measurement_dataset_version(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: measurements::DatasetVersionRequest,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = crate::db::app_data_directory(&app)?;
    with_conn(&db, |c| {
        measurements::create_dataset_version(c, current_workspace_id(), &root, request)
    })
}

#[tauri::command]
pub fn find_research_task_artifact(
    db: tauri::State<DbState>,
    project_id: String,
    task_id: String,
) -> Result<Option<String>, String> {
    use rusqlite::OptionalExtension;
    with_conn(&db, |c| {
        projects::get(c, current_workspace_id(), &project_id)?;
        artifacts::reconcile_task(c, current_workspace_id(), &task_id)?;
        c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND source_key=?3",rusqlite::params![current_workspace_id(),project_id,task_id],|r|r.get(0)).optional().map_err(|e|e.to_string())
    })
}

use crate::storage::repositories::material_rules as rules;
#[tauri::command]
pub fn get_material_registry(
    db: tauri::State<DbState>,
    project_id: Option<String>,
) -> Result<crate::steel::materials::Registry, String> {
    with_conn(&db, |c| {
        rules::registry(c, current_workspace_id(), project_id.as_deref())
    })
}
#[tauri::command]
pub fn suggest_material_fields(
    db: tauri::State<DbState>,
    project_id: Option<String>,
    names: Vec<String>,
) -> Result<Vec<Vec<crate::steel::materials::Match>>, String> {
    if names.len() > 512 {
        return Err("too_many_columns".into());
    }
    with_conn(&db, |c| {
        let registry = rules::registry(c, current_workspace_id(), project_id.as_deref())?;
        Ok(names
            .iter()
            .map(|name| crate::steel::materials::suggest(&registry, name))
            .collect())
    })
}
#[tauri::command]
pub fn save_material_schema_overlay(
    db: tauri::State<DbState>,
    project_id: String,
    fields: Vec<crate::steel::materials::Field>,
    overrides: Vec<String>,
) -> Result<u32, String> {
    with_conn(&db, |c| {
        rules::save_overlay(c, current_workspace_id(), &project_id, fields, overrides)
    })
}
#[tauri::command]
pub fn list_material_rules(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<Vec<rules::VersionedRule>, String> {
    with_conn(&db, |c| rules::list(c, current_workspace_id(), &project_id))
}
#[tauri::command]
pub fn save_material_rule(
    db: tauri::State<DbState>,
    project_id: String,
    rule: crate::steel::materials::Rule,
    expected_version: u32,
) -> Result<rules::VersionedRule, String> {
    with_conn(&db, |c| {
        rules::save(
            c,
            current_workspace_id(),
            &project_id,
            rule,
            expected_version,
        )
    })
}
#[tauri::command]
pub fn preview_material_rules(
    db: tauri::State<DbState>,
    project_id: String,
    variables: Vec<crate::steel::materials::Variable>,
    selected: Vec<String>,
) -> Result<rules::RuleSnapshot, String> {
    with_conn(&db, |c| {
        rules::compile(c, current_workspace_id(), &project_id, variables, &selected)
    })
}
#[tauri::command]
pub fn normalize_material_dataset(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    request: rules::NormalizeRequest,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = crate::db::app_data_directory(&app)?;
    with_conn(&db, |c| {
        rules::normalize_dataset(c, current_workspace_id(), &root, request)
    })
}
