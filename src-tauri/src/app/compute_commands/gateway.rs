use super::logic::{self, OptimizeSteelProcessRequest};
use crate::steel::OptimizationGateway;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Agent-facing gateway bound to the workspace database file. Each call opens
/// a short-lived connection so tool executions never hold locks across await
/// points.
pub struct DesktopOptimizationGateway {
    database: PathBuf,
    run_id: Option<uuid::Uuid>,
}

impl DesktopOptimizationGateway {
    pub fn new(database: PathBuf) -> Self {
        Self {
            database,
            run_id: None,
        }
    }

    pub fn with_run_id(mut self, run_id: uuid::Uuid) -> Self {
        self.run_id = Some(run_id);
        self
    }

    fn open(&self) -> Result<rusqlite::Connection, String> {
        let (connection, _) = crate::storage::database::open(&self.database)
            .map_err(|error| format!("open optimization database failed: {error}"))?;
        Ok(connection)
    }

    fn submit_task(
        &self,
        arguments: Value,
        tool_call_id: Option<uuid::Uuid>,
    ) -> Result<Value, String> {
        let request: OptimizeSteelProcessRequest = serde_json::from_value(arguments)
            .map_err(|error| format!("invalid optimization request: {error}"))?;
        let training_task_id = if request.objectives.is_empty() {
            uuid::Uuid::parse_str(&request.training_task_id)
                .map_err(|error| format!("invalid training task ID: {error}"))?
        } else {
            uuid::Uuid::nil()
        };
        let mut connection = self.open()?;
        let task = match (self.run_id, tool_call_id) {
            (Some(run_id), Some(tool_call_id)) => crate::tasks::sources::submit_with(
                &mut connection,
                crate::tasks::sources::AgentTaskSource {
                    workspace_id: crate::db::current_workspace_id().to_string(),
                    run_id,
                    tool_call_id,
                },
                |connection| {
                    logic::submit_optimization_on_connection(connection, &request, training_task_id)
                },
            )?,
            _ => logic::submit_optimization_on_connection(&connection, &request, training_task_id)?,
        };
        Ok(json!(
            crate::app::task_commands::tasks::background_task_response(task)
        ))
    }
}

impl OptimizationGateway for DesktopOptimizationGateway {
    fn submit(&self, arguments: Value) -> Result<Value, String> {
        self.submit_task(arguments, None)
    }

    fn submit_for_tool_call(
        &self,
        arguments: Value,
        tool_call_id: uuid::Uuid,
    ) -> Result<Value, String> {
        self.submit_task(arguments, Some(tool_call_id))
    }

    fn status(&self, task_id: &str) -> Result<Value, String> {
        let id =
            uuid::Uuid::parse_str(task_id).map_err(|error| format!("invalid task ID: {error}"))?;
        let connection = self.open()?;
        logic::optimization_task_status_on_connection(&connection, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_gateway_accepts_named_joint_models_without_legacy_training_id() {
        let path = std::env::temp_dir().join(format!(
            "suna-joint-gateway-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let (c, _) = crate::storage::database::open(&path).unwrap();
        let mut ids = vec![];
        for kind in [
            "compute_train_linear_regression",
            "compute_train_sklearn_model",
        ] {
            let t = crate::tasks::repository::create(
                &c,
                crate::tasks::NewTask {
                    workspace_id: crate::db::current_workspace_id().into(),
                    kind: kind.into(),
                    payload_json: json!({"payload":{"dataset_id":"synthetic"}}).to_string(),
                    checkpoint_json: None,
                    next_run_at: None,
                    progress: 0,
                },
            )
            .unwrap();
            c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?1 WHERE id=?2",rusqlite::params![json!({"result":{"artifact":{"feature_names":["C"]}}}).to_string(),t.id.to_string()]).unwrap();
            ids.push(t.id.to_string());
        }
        drop(c);
        let gateway = DesktopOptimizationGateway::new(path.clone());
        let result=gateway.submit(json!({"datasetId":"synthetic","trials":20,"variables":[{"name":"C","min":0,"max":1}],"objectives":[{"name":"HV","source":"prediction_model","modelId":ids[0],"direction":"maximize","unit":"HV"},{"name":"J","source":"prediction_model","modelId":ids[1],"direction":"maximize","unit":"J"}]})).unwrap();
        assert_eq!(result["kind"], "compute_optimize_constrained");
        std::fs::remove_file(path).unwrap();
    }
}
