//! Scientific workflows use frozen Agent project context and existing tool-call lineage.
use super::*;
use crate::research_workflow::{
    entry::{Selection, StartRequest},
    schema::Template,
    status, templates, worker_validation,
};
use crate::storage::repositories::research_delivery as delivery;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Save {
    template: Template,
    expected_revision: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetTemplate {
    id: String,
    revision: u64,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    limit: Option<u32>,
    offset: Option<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetRun {
    id: Uuid,
}
fn parse<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|_| "workflow_request_invalid".into())
}
impl DesktopSteelAgentGateway {
    pub(super) async fn workflow_call(
        &self,
        name: &str,
        args: Value,
        cancel: CancellationToken,
    ) -> Result<Value, String> {
        if cancel.is_cancelled() {
            return Err("workflow_cancelled".into());
        }
        let mut c = self.open().map_err(|_| "workflow_storage_error")?;
        if name == "research_list_workflow_templates" {
            let p: Page = parse(args)?;
            return Ok(json!(templates::list(
                &c,
                &self.workspace_id,
                p.limit.unwrap_or(25),
                p.offset.unwrap_or(0)
            )?));
        }
        if name == "research_get_workflow_template" {
            let r: GetTemplate = parse(args)?;
            return templates::get(&c, &self.workspace_id, &r.id, r.revision);
        }
        // No fallback to the user's mutable current-project setting.
        let run = self.run_id.ok_or("workflow_agent_run_required")?;
        let project = delivery::context(&c, &self.workspace_id, &run.to_string())?
            .ok_or("research_project_unassigned")?;
        if name == "research_list_workflows" {
            let p: Page = parse(args)?;
            return Ok(json!(status::list(
                &c,
                &self.workspace_id,
                &project,
                p.limit.unwrap_or(25),
                p.offset.unwrap_or(0)
            )?));
        }
        if name == "research_get_workflow" {
            let r: GetRun = parse(args)?;
            return status::get(&c, &self.workspace_id, &project, r.id);
        }
        let source = crate::tasks::sources::AgentTaskSource {
            workspace_id: self.workspace_id.clone(),
            run_id: run,
            tool_call_id: self
                .tool_call_id
                .ok_or("workflow_agent_tool_call_required")?,
        };
        if name == "research_save_workflow_template" {
            crate::tasks::sources::validate_source_for_submission(&c, &source)
                .map_err(|_| "workflow_agent_submission_not_authorized")?;
            let r: Save = parse(args)?;
            return templates::save(&mut c, &self.workspace_id, &r.template, r.expected_revision);
        }
        let start = if name == "research_start_workflow" {
            Some(parse::<StartRequest>(args.clone())?)
        } else {
            None
        };
        let selection = match &start {
            Some(r) => r.selection(),
            None => parse::<Selection>(args)?,
        };
        if selection.project_id != project {
            return Err("workflow_project_mismatch".into());
        }
        if let Some(r) = &start {
            if !r.confirmed {
                return Err("workflow_start_confirmation_required".into());
            }
            crate::tasks::sources::validate_source_for_submission(&c, &source)
                .map_err(|_| "workflow_agent_submission_not_authorized")?;
        }
        let template = selection.template(&c, &self.workspace_id)?;
        let config = self.app.as_ref().and_then(crate::db::compute_worker_config);
        drop(c);
        let worker = tauri::async_runtime::spawn_blocking(move || {
            worker_validation::probe(&template, config)
        })
        .await
        .map_err(|_| "workflow_worker_probe_failed")??;
        if cancel.is_cancelled() {
            return Err("workflow_cancelled".into());
        }
        let mut c = self.open().map_err(|_| "workflow_storage_error")?;
        match start {
            None => crate::research_workflow::entry::preview(
                &c,
                &self.workspace_id,
                &selection,
                &worker,
            ),
            Some(r) => crate::research_workflow::entry::start_for_agent(
                &mut c,
                &self.workspace_id,
                &r,
                &worker,
                source,
                || cancel.is_cancelled(),
            ),
        }
    }
}
