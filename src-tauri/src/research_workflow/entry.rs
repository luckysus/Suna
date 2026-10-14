//! Shared typed desktop/Agent boundary. Worker evidence never comes from JSON.
use super::{
    schema::{FrozenInput, Template},
    worker_validation::{self, VerifiedWorker},
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub template_id: String,
    pub template_revision: u64,
    pub project_id: String,
    pub dataset_id: String,
    pub dataset_sha256: String,
}
impl Selection {
    pub fn input(&self) -> FrozenInput {
        FrozenInput {
            project_id: self.project_id.clone(),
            dataset_id: self.dataset_id.clone(),
            dataset_sha256: self.dataset_sha256.clone(),
        }
    }
    pub fn template(&self, c: &Connection, ws: &str) -> Result<Template, String> {
        serde_json::from_value(
            super::templates::get(c, ws, &self.template_id, self.template_revision)?["template"]
                .clone(),
        )
        .map_err(|_| "workflow_template_invalid".into())
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRequest {
    pub template_id: String,
    pub template_revision: u64,
    pub project_id: String,
    pub dataset_id: String,
    pub dataset_sha256: String,
    pub dry_run_sha256: String,
    pub nonce: Uuid,
    pub confirmed: bool,
}
impl StartRequest {
    pub fn selection(&self) -> Selection {
        Selection {
            template_id: self.template_id.clone(),
            template_revision: self.template_revision,
            project_id: self.project_id.clone(),
            dataset_id: self.dataset_id.clone(),
            dataset_sha256: self.dataset_sha256.clone(),
        }
    }
}
pub fn preview(
    c: &Connection,
    ws: &str,
    s: &Selection,
    worker: &VerifiedWorker,
) -> Result<Value, String> {
    worker_validation::annotate(
        super::dry_run_on_connection(c, ws, &s.template(c, ws)?, &s.input())?,
        worker,
    )
}
pub fn start(
    c: &mut Connection,
    ws: &str,
    r: &StartRequest,
    worker: &VerifiedWorker,
) -> Result<Value, String> {
    if r.nonce.is_nil() {
        return Err("workflow_nonce_invalid".into());
    }
    let s = r.selection();
    let template = s.template(c, ws)?;
    let root = super::runtime::submit_checked(
        c,
        ws,
        &template,
        &s.input(),
        &r.dry_run_sha256,
        r.nonce,
        r.confirmed,
        worker,
    )?;
    super::status::get(c, ws, &s.project_id, root.id)
}

pub fn start_for_agent(
    c: &mut Connection,
    ws: &str,
    r: &StartRequest,
    worker: &VerifiedWorker,
    source: crate::tasks::sources::AgentTaskSource,
    cancelled: impl Fn() -> bool,
) -> Result<Value, String> {
    if source.workspace_id != ws {
        return Err("workflow_agent_submission_not_authorized".into());
    }
    crate::storage::repositories::research_artifacts::atomic_mut(c, |c| {
        crate::tasks::sources::validate_source_for_submission(c, &source)
            .map_err(|_| "workflow_agent_submission_not_authorized")?;
        let project = crate::storage::repositories::research_delivery::context(
            c,
            ws,
            &source.run_id.to_string(),
        )?
        .ok_or("research_project_unassigned")?;
        if project != r.project_id {
            return Err("workflow_project_mismatch".into());
        }
        if cancelled() {
            return Err("workflow_cancelled".into());
        }
        let result = start(c, ws, r, worker)?;
        let id = Uuid::parse_str(
            result["task"]["id"]
                .as_str()
                .ok_or("workflow_task_invalid")?,
        )
        .map_err(|_| "workflow_task_invalid")?;
        crate::tasks::sources::attach_existing(c, source, id)
            .map_err(|_| "workflow_agent_task_source_conflict")?;
        if cancelled() {
            return Err("workflow_cancelled".into());
        }
        Ok(result)
    })
}
