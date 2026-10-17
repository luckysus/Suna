use super::*;
pub(super) fn run_step(
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
    let saved = crate::research_workflow::templates::get(
        c,
        &task.workspace_id,
        &template.id,
        template.revision,
    )?;
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
        crate::research_workflow::provenance::cancel_owned_nodes(c, task)?;
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
    let (_, approved_deadline) = crate::research_workflow::provenance::definition(c, task)?;
    if Utc::now() >= deadline || Utc::now() >= approved_deadline {
        crate::research_workflow::provenance::cancel_owned_nodes(c, task)?;
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
        if preview["dryRunSha256"] != input["base_dry_run_sha256"] {
            return Err("workflow_preview_changed".into());
        }
        let index = cp["children"].as_array().unwrap().len();
        let model = &template.models[index];
        let request = TrainSteelDatasetRequest {
            dataset_id: dataset.into(),
            target_column: model.target_column,
            feature_columns: model.feature_columns.clone(),
            split_policy: Some(model.split_policy.clone()),
            algorithm: Some(model.algorithm.clone()),
        };
        let prepared = logic::prepare_steel_training_task(
            logic::load_train_dataset_snapshot(c, &task.workspace_id, dataset)?,
            &request,
        )?;
        artifacts::atomic_mut(c, |c| {
            ensure_active(c, task)?;
            let node = delivery::scoped(c, Some(project), || {
                logic::train_steel_dataset_prepared(c, &task.workspace_id, &prepared)
            })?;
            crate::research_workflow::provenance::bind_node(
                c,
                task,
                &node,
                &format!("workflow_train_{index}"),
            )?;
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
            &format!("workflow_train_{index}"),
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
                delivery::scoped_mut(c, Some(project), |c| {
                    logic::register_steel_model_on_connection_with_policy(
                        c,
                        &task.workspace_id,
                        &RegisterSteelModelRequest {
                            task_id: id.to_string(),
                            lineage_id: Some(lineage),
                        },
                        models::RegistrationPolicy {
                            activate_first: false,
                            guard: Some(&|c| {
                                ensure_active(c, task)?;
                                let (_, deadline) =
                                    crate::research_workflow::provenance::definition(c, task)?;
                                if Utc::now() >= deadline {
                                    return Err(
                                        "workflow_timeout_requires_new_confirmed_run".into()
                                    );
                                }
                                Ok(())
                            }),
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
        if fresh["dryRunSha256"] != input["base_dry_run_sha256"] {
            return Err("workflow_sources_or_rules_changed".into());
        }
        artifacts::atomic_mut(c, |c| {
            ensure_active(c, task)?;
            let objectives=template.models.iter().enumerate().map(|(i,m)|json!({"name":m.name,"source":"prediction_model","modelId":cp["children"][i],"direction":m.direction,"unit":m.unit.as_deref().unwrap_or("")})).collect::<Vec<_>>();
            let request=serde_json::from_value(json!({"datasetId":dataset,"objectives":objectives,"variables":template.variables,"constraints":template.constraints,"trials":template.trials,"seed":template.seed,"algorithm":"nsga2"})).map_err(|_|"workflow_optimization_payload")?;
            let node = delivery::scoped(c, Some(project), || {
                logic::submit_optimization_in_workspace(
                    c,
                    &task.workspace_id,
                    &request,
                    Uuid::nil(),
                )
            })?;
            crate::research_workflow::provenance::bind_node(
                c,
                task,
                &node,
                "workflow_optimization",
            )?;
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
        "workflow_optimization",
        &input["dataset_sha256"],
    )?;
    if pending(c, task, &mut cp, &optimization)? {
        return Ok(HandlerOutcome::WaitingExternal);
    }
    if cp["plan"].is_null() {
        artifacts::atomic_mut(c, |c| {
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
            cp["resultArtifactId"] = json!(crate::research_workflow::provenance::complete_summary(
                c, task, &cp
            )?);
            checkpoint(c, task, &cp, 100, false)
        })?;
    }
    // Emits the same Scheduler event used by Inspector after the committed phase.
    context
        .checkpoint(Some(&cp.to_string()), 100, None)
        .map_err(|_| "workflow_checkpoint_stale")?;
    Ok(HandlerOutcome::Completed)
}
