use super::*;
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
    fn maintain(
        &self,
        c: &mut Connection,
        ws: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<Vec<TaskRecord>, HandlerError> {
        crate::research_workflow::lifecycle::sweep(c, ws, now).map_err(HandlerError::permanent)
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
