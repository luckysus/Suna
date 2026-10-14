//! Scientific workflow orchestration through the existing Scheduler and repositories.
use crate::app::compute_commands::logic::{
    self, RegisterSteelModelRequest, TrainSteelDatasetRequest,
};
use crate::research_workflow::{
    dry_run_on_connection,
    schema::{FrozenInput, Template},
};
use crate::storage::repositories::{
    experiment_measurements as measurements, research_artifacts as artifacts,
    research_delivery as delivery, research_projects as projects, steel_models as models,
};
use crate::tasks::scheduler::{
    HandlerContext, HandlerError, HandlerFuture, HandlerOutcome, TaskHandler,
};
use crate::tasks::{repository, NewTask, TaskRecord, TaskState};
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;

pub const KIND: &str = "research_modeling_workflow";
mod handler;
mod nodes;
mod step;
mod submission;
pub use handler::ModelingWorkflowHandler;
pub use nodes::request_cancellation;
use nodes::*;
use step::run_step;
pub use submission::submit;
pub use submission::submit_checked;
