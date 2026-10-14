use super::*;
pub fn submit(
    c: &mut Connection,
    ws: &str,
    template: &Template,
    input: &FrozenInput,
    preview_sha: &str,
    nonce: Uuid,
    confirmed: bool,
) -> Result<TaskRecord, String> {
    submit_inner(c, ws, template, input, preview_sha, nonce, confirmed, None)
}
/// Desktop and Agent entry points must obtain this permit from the actual Worker
/// diagnostic. The private evidence cannot be supplied through a JSON request.
pub fn submit_checked(
    c: &mut Connection,
    ws: &str,
    template: &Template,
    input: &FrozenInput,
    preview_sha: &str,
    nonce: Uuid,
    confirmed: bool,
    worker: &crate::research_workflow::worker_validation::VerifiedWorker,
) -> Result<TaskRecord, String> {
    submit_inner(
        c,
        ws,
        template,
        input,
        preview_sha,
        nonce,
        confirmed,
        Some(worker),
    )
}
#[allow(clippy::too_many_arguments)]
fn submit_inner(
    c: &mut Connection,
    ws: &str,
    template: &Template,
    input: &FrozenInput,
    preview_sha: &str,
    nonce: Uuid,
    confirmed: bool,
    worker: Option<&crate::research_workflow::worker_validation::VerifiedWorker>,
) -> Result<TaskRecord, String> {
    if !confirmed {
        return Err("workflow_start_confirmation_required".into());
    }
    let saved = crate::research_workflow::templates::get(c, ws, &template.id, template.revision)?;
    if saved["template"]
        != serde_json::to_value(template).map_err(|_| "workflow_template_invalid")?
    {
        return Err("workflow_template_revision_changed".into());
    }
    let base = dry_run_on_connection(c, ws, template, input)?;
    let preview = match worker {
        Some(worker) => {
            crate::research_workflow::worker_validation::annotate(base.clone(), worker)?
        }
        None => base.clone(),
    };
    if preview["dryRunSha256"] != preview_sha {
        return Err("workflow_preview_changed".into());
    }
    artifacts::atomic_mut(c, |c| {
        let previous:Option<(String,String)>=c.query_row("SELECT id,payload_json FROM background_tasks WHERE workspace_id=?1 AND kind=?2 AND json_extract(payload_json,'$.nonce')=?3",params![ws,KIND,nonce.to_string()],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(|_|"workflow_storage_error")?;
        let payload = json!({"template":template,"template_content_sha256":saved["contentSha256"],"project_id":input.project_id,"dataset_id":input.dataset_id,"dataset_sha256":input.dataset_sha256,"dry_run_sha256":preview_sha,"base_dry_run_sha256":base["dryRunSha256"],"worker_validation":worker.map(|w|w.evidence()),"material_rules_hash":preview["materialRules"]["ruleSetHash"],"nonce":nonce,"confirmed":true});
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
        let active:u32=c.query_row("SELECT COUNT(*) FROM background_tasks WHERE workspace_id=?1 AND kind=?2 AND state IN ('queued','running','waiting_external','paused','interrupted')",params![ws,KIND],|r|r.get(0)).map_err(|_|"workflow_storage_error")?;
        if active >= 64 {
            return Err("workflow_active_budget".into());
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
        crate::research_workflow::provenance::create_definition(c, &root)?;
        Ok(root)
    })
}
