//! Prototype orchestrator: existing Scheduler states, child jobs and repositories.
//! Not installed into Suna; template persistence/Agent/desktop gates remain pending.
use crate::{
    dry_run_on_connection,
    schema::{FrozenInput, Template},
};
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::PathBuf;
use suna::app::compute_commands::logic::{
    self, RegisterSteelModelRequest, TrainSteelDatasetRequest,
};
use suna::storage::repositories::{
    experiment_measurements as measurements, research_artifacts as artifacts,
    research_delivery as delivery, research_projects as projects, steel_models as models,
};
use suna::tasks::scheduler::{
    HandlerContext, HandlerError, HandlerFuture, HandlerOutcome, TaskHandler,
};
use suna::tasks::{repository, NewTask, TaskRecord, TaskState};
use uuid::Uuid;

pub const KIND: &str = "research_modeling_workflow";

pub fn submit(
    c: &mut Connection,
    ws: &str,
    template: &Template,
    input: &FrozenInput,
    preview_sha: &str,
    nonce: Uuid,
    confirmed: bool,
) -> Result<TaskRecord, String> {
    if !confirmed {
        return Err("workflow_start_confirmation_required".into());
    }
    let saved = crate::templates::get(c, ws, &template.id, template.revision)?;
    if saved["template"]
        != serde_json::to_value(template).map_err(|_| "workflow_template_invalid")?
    {
        return Err("workflow_template_revision_changed".into());
    }
    let preview = dry_run_on_connection(c, ws, template, input)?;
    if preview["dryRunSha256"] != preview_sha {
        return Err("workflow_preview_changed".into());
    }
    atomic_mut(c, |c| {
        let previous:Option<(String,String)>=c.query_row("SELECT id,payload_json FROM background_tasks WHERE workspace_id=?1 AND kind=?2 AND json_extract(payload_json,'$.nonce')=?3",params![ws,KIND,nonce.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|"workflow_storage_error")?;
        let payload = json!({"template":template,"template_content_sha256":saved["contentSha256"],"project_id":input.project_id,"dataset_id":input.dataset_id,"dataset_sha256":input.dataset_sha256,"dry_run_sha256":preview_sha,"material_rules_hash":preview["materialRules"]["ruleSetHash"],"nonce":nonce,"confirmed":true});
        if let Some((id, old)) = previous {
            if serde_json::from_str::<Value>(&old).map_err(|_| "workflow_payload_invalid")?
                != payload
            {
                return Err("workflow_nonce_conflict".into());
            }
            return repository::get(
                c,
                ws,
                Uuid::parse_str(&id).map_err(|_| "workflow_task_invalid")?,
            )
            .map_err(|_| "workflow_storage_error")?
            .ok_or("workflow_task_missing".into());
        }
        let root = repository::create(
            c,
            NewTask {
                workspace_id: ws.into(),
                kind: KIND.into(),
                payload_json: payload.to_string(),
                checkpoint_json: Some(
                    json!({"children":[],"models":[],"optimization":null,"plan":null,
                        "deadline":(Utc::now()+Duration::seconds(template.timeout_seconds.into())).to_rfc3339()}).to_string(),
                ),
                next_run_at: None,
                progress: 0,
            },
        )
        .map_err(|_| "workflow_storage_error")?;
        crate::provenance::create_definition(c, &root)?;
        Ok(root)
    })
}

fn atomic_mut<T>(
    c: &mut Connection,
    f: impl FnOnce(&mut Connection) -> Result<T, String>,
) -> Result<T, String> {
    // Uses the same SQLite savepoint mechanism as ResearchArtifact::atomic.
    let point = format!("workflow_{}", Uuid::new_v4().simple());
    c.execute_batch(&format!("SAVEPOINT {point}"))
        .map_err(|_| "workflow_storage_error")?;
    let result = f(c);
    match result {
        Ok(v) => {
            c.execute_batch(&format!("RELEASE {point}"))
                .map_err(|_| "workflow_storage_error")?;
            Ok(v)
        }
        Err(e) => {
            c.execute_batch(&format!("ROLLBACK TO {point}; RELEASE {point}"))
                .map_err(|_| "workflow_storage_error")?;
            Err(e)
        }
    }
}

fn scoped_mut<T>(
    c: &mut Connection,
    project: &str,
    f: impl FnOnce(&mut Connection) -> Result<T, String>,
) -> Result<T, String> {
    // Same connection-local override as research_delivery::scoped, including
    // explicit-null restoration. Production should extend that existing helper.
    let old = delivery::scope(c)?;
    c.execute_batch("CREATE TEMP TABLE IF NOT EXISTS research_submission_scope(id INTEGER PRIMARY KEY,project_id TEXT)").map_err(|_|"workflow_storage_error")?;
    c.execute(
        "INSERT OR REPLACE INTO temp.research_submission_scope VALUES(1,?1)",
        [project],
    )
    .map_err(|_| "workflow_storage_error")?;
    let result = f(c);
    c.execute("DELETE FROM temp.research_submission_scope", [])
        .map_err(|_| "workflow_storage_error")?;
    if let Some(p) = old {
        c.execute(
            "INSERT INTO temp.research_submission_scope VALUES(1,?1)",
            [p],
        )
        .map_err(|_| "workflow_storage_error")?;
    }
    result
}

pub struct ModelingWorkflowHandler {
    path: PathBuf,
}
impl ModelingWorkflowHandler {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}
impl TaskHandler for ModelingWorkflowHandler {
    fn kind(&self) -> &str {
        KIND
    }
    fn resumable(&self) -> bool {
        true
    }
    fn run(&self, task: TaskRecord, context: HandlerContext) -> HandlerFuture {
        let path = self.path.clone();
        Box::pin(async move {
            let mut c = Connection::open(path)
                .map_err(|_| HandlerError::permanent("workflow_storage_error"))?;
            c.busy_timeout(std::time::Duration::from_secs(3))
                .map_err(|_| HandlerError::permanent("workflow_storage_error"))?;
            c.execute_batch("PRAGMA foreign_keys=ON")
                .map_err(|_| HandlerError::permanent("workflow_storage_error"))?;
            run_step(&mut c, &task, &context).map_err(|error| HandlerError::permanent(error))
        })
    }
}

fn checkpoint(
    c: &mut Connection,
    task: &TaskRecord,
    cp: &Value,
    progress: u8,
    due: bool,
) -> Result<(), String> {
    let next = due.then(|| (Utc::now() + Duration::milliseconds(100)).to_rfc3339());
    repository::checkpoint(
        c,
        &task.workspace_id,
        task.id,
        task.attempt,
        Some(&cp.to_string()),
        progress,
        next.as_deref(),
    )
    .map_err(|_| "workflow_checkpoint_stale")?;
    Ok(())
}

fn child(
    c: &Connection,
    task: &TaskRecord,
    id: Uuid,
    project: &str,
    dataset: &str,
    kind: &str,
    source_sha: &Value,
) -> Result<TaskRecord, String> {
    let record = repository::get(c, &task.workspace_id, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_child_missing")?;
    let payload: Value =
        serde_json::from_str(&record.payload_json).map_err(|_| "workflow_child_payload")?;
    let owns:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND background_task_id=?3)",params![task.workspace_id,project,id.to_string()],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
    if record.kind != kind || payload["payload"]["dataset_id"] != dataset || !owns {
        return Err("workflow_child_ownership".into());
    }
    if !crate::provenance::owned_nodes(c, task)?
        .iter()
        .any(|owned| owned.id == id)
    {
        return Err("workflow_child_ownership".into());
    }
    if kind.starts_with("compute_train_") && payload["payload"]["data_version"] != *source_sha {
        return Err("workflow_child_source_changed".into());
    }
    Ok(record)
}

fn pending(
    c: &mut Connection,
    task: &TaskRecord,
    cp: &mut Value,
    record: &TaskRecord,
) -> Result<bool, String> {
    if record.state == TaskState::Completed {
        return Ok(false);
    }
    if matches!(
        record.state,
        TaskState::Failed | TaskState::Cancelled | TaskState::Interrupted | TaskState::Paused
    ) {
        if cp["failed_child"] == record.id.to_string() {
            repository::retry_manually(
                c,
                &task.workspace_id,
                record.id,
                record.attempt,
                record.state,
            )
            .map_err(|_| "workflow_child_retry_failed")?;
            cp["failed_child"] = Value::Null;
        } else {
            cp["failed_child"] = json!(record.id);
            checkpoint(c, task, cp, task.progress, false)?;
            return Err("workflow_child_failed".into());
        }
    }
    checkpoint(c, task, cp, task.progress, true)?;
    Ok(true)
}

fn parse_id(value: &Value) -> Result<Uuid, String> {
    Uuid::parse_str(value.as_str().ok_or("workflow_checkpoint_invalid")?)
        .map_err(|_| "workflow_checkpoint_invalid".into())
}

fn ensure_active(c: &Connection, task: &TaskRecord) -> Result<(), String> {
    let current = repository::get(c, &task.workspace_id, task.id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    if current.state != TaskState::Running
        || current.attempt != task.attempt
        || current.cancel_requested
    {
        return Err("workflow_checkpoint_stale".into());
    }
    Ok(())
}

fn run_step(
    c: &mut Connection,
    task: &TaskRecord,
    context: &HandlerContext,
) -> Result<HandlerOutcome, String> {
    let input: Value =
        serde_json::from_str(&task.payload_json).map_err(|_| "workflow_payload_invalid")?;
    if input["confirmed"] != true {
        return Err("workflow_start_confirmation_required".into());
    }
    let template: Template = serde_json::from_value(input["template"].clone())
        .map_err(|_| "workflow_payload_invalid")?;
    let saved = crate::templates::get(c, &task.workspace_id, &template.id, template.revision)?;
    if saved["contentSha256"] != input["template_content_sha256"]
        || saved["template"] != input["template"]
    {
        return Err("workflow_template_revision_changed".into());
    }
    let project = input["project_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    let dataset = input["dataset_id"]
        .as_str()
        .ok_or("workflow_payload_invalid")?;
    let mut cp: Value = serde_json::from_str(
        task.checkpoint_json
            .as_deref()
            .ok_or("workflow_checkpoint_invalid")?,
    )
    .map_err(|_| "workflow_checkpoint_invalid")?;
    if context
        .cancellation_requested()
        .map_err(|_| "workflow_storage_error")?
    {
        crate::provenance::cancel_owned_nodes(c, task)?;
        return Ok(HandlerOutcome::Cancelled);
    }
    if context.shutdown_requested() {
        return Ok(HandlerOutcome::Interrupted);
    }
    let deadline = chrono::DateTime::parse_from_rfc3339(
        cp["deadline"]
            .as_str()
            .ok_or("workflow_checkpoint_invalid")?,
    )
    .map_err(|_| "workflow_checkpoint_invalid")?;
    let (_, approved_deadline) = crate::provenance::definition(c, task)?;
    if Utc::now() >= deadline || Utc::now() >= approved_deadline {
        crate::provenance::cancel_owned_nodes(c, task)?;
        return Err("workflow_timeout_requires_new_confirmed_run".into());
    }
    projects::require_active(c, &task.workspace_id, project)?;
    if cp["children"]
        .as_array()
        .ok_or("workflow_checkpoint_invalid")?
        .len()
        < 2
    {
        let source = FrozenInput {
            project_id: project.into(),
            dataset_id: dataset.into(),
            dataset_sha256: input["dataset_sha256"]
                .as_str()
                .ok_or("workflow_payload_invalid")?
                .into(),
        };
        let preview = dry_run_on_connection(c, &task.workspace_id, &template, &source)?;
        if preview["dryRunSha256"] != input["dry_run_sha256"] {
            return Err("workflow_preview_changed".into());
        }
        atomic_mut(c, |c| {
            ensure_active(c, task)?;
            let index = cp["children"].as_array().unwrap().len();
            let model = &template.models[index];
            let request = TrainSteelDatasetRequest {
                dataset_id: dataset.into(),
                target_column: model.target_column,
                feature_columns: model.feature_columns.clone(),
                split_policy: Some(model.split_policy.clone()),
                algorithm: Some(model.algorithm.clone()),
            };
            let node = delivery::scoped(c, Some(project), || {
                logic::train_steel_dataset_on_connection(c, &task.workspace_id, &request)
            })?;
            crate::provenance::bind_node(c, task, &node, &format!("workflow_train_{index}"))?;
            cp["children"].as_array_mut().unwrap().push(json!(node.id));
            checkpoint(c, task, &cp, 10 + (index as u8) * 10, true)
        })?;
        return Ok(HandlerOutcome::WaitingExternal);
    }
    for (index, model) in template.models.iter().enumerate() {
        let id = parse_id(&cp["children"][index])?;
        let expected = if model.algorithm == "linear_regression" {
            "compute_train_linear_regression"
        } else {
            "compute_train_sklearn_model"
        };
        let record = child(
            c,
            task,
            id,
            project,
            dataset,
            expected,
            &input["dataset_sha256"],
        )?;
        if pending(c, task, &mut cp, &record)? {
            return Ok(HandlerOutcome::WaitingExternal);
        }
        if cp["models"]
            .as_array()
            .ok_or("workflow_checkpoint_invalid")?
            .len()
            <= index
        {
            let lineage = format!("workflow:{}:model_{}", task.id, index + 1);
            let registered = models::list(c, &task.workspace_id, &lineage)?
                .into_iter()
                .find(|m| m.source_task_id.as_deref() == Some(&id.to_string()));
            let model = if let Some(m) = registered {
                models::scientific_cards(c, &task.workspace_id, &m.id)?;
                m
            } else {
                ensure_active(c, task)?;
                scoped_mut(c, project, |c| {
                    logic::register_steel_model_on_connection(
                        c,
                        &task.workspace_id,
                        &RegisterSteelModelRequest {
                            task_id: id.to_string(),
                            lineage_id: Some(lineage),
                        },
                    )
                })?
            };
            cp["models"].as_array_mut().unwrap().push(json!(model.id));
            checkpoint(c, task, &cp, 40 + (index as u8) * 10, true)?;
            return Ok(HandlerOutcome::WaitingExternal);
        }
    }
    if cp["optimization"].is_null() {
        let source = FrozenInput {
            project_id: project.into(),
            dataset_id: dataset.into(),
            dataset_sha256: input["dataset_sha256"]
                .as_str()
                .ok_or("workflow_payload_invalid")?
                .into(),
        };
        let fresh = dry_run_on_connection(c, &task.workspace_id, &template, &source)?;
        if fresh["dryRunSha256"] != input["dry_run_sha256"] {
            return Err("workflow_sources_or_rules_changed".into());
        }
        atomic_mut(c, |c| {
            ensure_active(c, task)?;
            let objectives=template.models.iter().enumerate().map(|(i,m)|json!({"name":m.name,"source":"prediction_model","modelId":cp["children"][i],"direction":m.direction,"unit":m.unit.as_deref().unwrap_or("")})).collect::<Vec<_>>();
            let request=serde_json::from_value(json!({"datasetId":dataset,"objectives":objectives,"variables":template.variables,"constraints":template.constraints,"trials":template.trials,"seed":template.seed,"algorithm":"nsga2"})).map_err(|_|"workflow_optimization_payload")?;
            let node = delivery::scoped(c, Some(project), || {
                logic::submit_optimization_on_connection(c, &request, Uuid::nil())
            })?;
            crate::provenance::bind_node(c, task, &node, "workflow_optimization")?;
            cp["optimization"] = json!(node.id);
            checkpoint(c, task, &cp, 70, true)
        })?;
        return Ok(HandlerOutcome::WaitingExternal);
    }
    let optimization = child(
        c,
        task,
        parse_id(&cp["optimization"])?,
        project,
        dataset,
        "compute_optimize_constrained",
        &input["dataset_sha256"],
    )?;
    if pending(c, task, &mut cp, &optimization)? {
        return Ok(HandlerOutcome::WaitingExternal);
    }
    if cp["plan"].is_null() {
        atomic_mut(c, |c| {
            ensure_active(c, task)?;
            artifacts::reconcile_task(c, &task.workspace_id, &optimization.id.to_string())?;
            let artifact:String=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='optimization_result' AND source_key=?3",params![task.workspace_id,project,optimization.id.to_string()],|r|r.get(0)).map_err(|_|"workflow_optimization_artifact_missing")?;
            let plan = measurements::candidate_plan(
                c,
                &task.workspace_id,
                project,
                &artifact,
                0,
                "工作流优化候选（待人工确认）",
            )?;
            cp["plan"] = json!(plan.id);
            cp["result"] = json!({"projectId":project,"trainingTaskIds":cp["children"],"registeredModelIds":cp["models"],"optimizationTaskId":cp["optimization"],"proposedPlanId":plan.id,"confirmationRequiredToAccept":true,"measurementsCreated":false});
            cp["resultArtifactId"] = json!(crate::provenance::complete_summary(c, task, &cp)?);
            checkpoint(c, task, &cp, 100, false)
        })?;
    }
    // Emits the same Scheduler event used by Inspector after the committed phase.
    context
        .checkpoint(Some(&cp.to_string()), 100, None)
        .map_err(|_| "workflow_checkpoint_stale")?;
    Ok(HandlerOutcome::Completed)
}

pub fn request_cancellation(c: &mut Connection, ws: &str, id: Uuid) -> Result<(), String> {
    let task = repository::get(c, ws, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    if task.kind != KIND {
        return Err("workflow_task_invalid".into());
    }
    crate::provenance::owned_nodes(c, &task)?;
    match task.state {
        TaskState::Running => {
            repository::request_running_cancellation(c, ws, id, task.attempt)
                .map_err(|_| "workflow_cancel_conflict")?;
        }
        TaskState::Queued
        | TaskState::WaitingExternal
        | TaskState::Paused
        | TaskState::Interrupted => {
            repository::transition(
                c,
                ws,
                id,
                task.attempt,
                task.state,
                TaskState::Cancelled,
                None,
            )
            .map_err(|_| "workflow_cancel_conflict")?;
        }
        _ => return Err("workflow_not_cancellable".into()),
    }
    // Freeze the parent first. Existing repository CAS transactions stay intact;
    // interrupted cancellation is finished by maintenance on the next tick.
    let latest = repository::get(c, ws, id)
        .map_err(|_| "workflow_storage_error")?
        .ok_or("workflow_task_missing")?;
    crate::provenance::cancel_owned_nodes(c, &latest)
}
