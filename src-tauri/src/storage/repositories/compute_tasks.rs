use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

/// 第 80 章：`prediction_tasks` / `optimization_tasks` 两张表的接线。
///
/// `background_tasks` 记录**执行**（状态机、进度、耗时），这两张表记录**语义**
/// （用哪个数据集 / 模型、预测或优化的请求与结果），提交任务时写入、取回结果时同步状态。

pub const COMPUTE_TASK_STATES: [&str; 5] =
    ["queued", "running", "succeeded", "failed", "cancelled"];

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ComputeTaskRecord {
    pub id: String,
    pub kind: String,
    pub background_task_id: Option<String>,
    pub dataset_id: Option<String>,
    pub model_id: Option<String>,
    pub direction: Option<String>,
    pub method: Option<String>,
    pub trials_requested: Option<i64>,
    pub state: String,
    pub request_json: String,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 把 `background_tasks` 的状态映射到本表的状态集合。
pub fn state_for(task_state: &str) -> &'static str {
    match task_state {
        "queued" => "queued",
        "completed" => "succeeded",
        "failed" | "interrupted" => "failed",
        "cancelled" => "cancelled",
        _ => "running",
    }
}

pub struct NewPredictionTask<'a> {
    pub background_task_id: &'a str,
    pub dataset_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub request_json: &'a str,
}

pub fn create_prediction(
    connection: &Connection,
    workspace_id: &str,
    task: NewPredictionTask<'_>,
) -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(task.request_json)
        .map_err(|error| format!("request must be valid JSON: {error}"))?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO prediction_tasks
               (workspace_id, id, background_task_id, dataset_id, model_id, state,
                request_json, result_json, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, NULL, NULL, ?7, ?7)",
            params![
                workspace_id,
                id,
                task.background_task_id,
                task.dataset_id,
                task.model_id,
                task.request_json,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(id)
}

pub struct NewOptimizationTask<'a> {
    pub background_task_id: &'a str,
    pub dataset_id: Option<&'a str>,
    pub model_id: Option<&'a str>,
    pub direction: &'a str,
    pub method: Option<&'a str>,
    pub trials_requested: i64,
    pub request_json: &'a str,
}

pub fn create_optimization(
    connection: &Connection,
    workspace_id: &str,
    task: NewOptimizationTask<'_>,
) -> Result<String, String> {
    if task.direction != "minimize" && task.direction != "maximize" {
        return Err("optimization direction must be minimize or maximize".to_string());
    }
    if task.trials_requested < 0 {
        return Err("trials_requested must not be negative".to_string());
    }
    serde_json::from_str::<serde_json::Value>(task.request_json)
        .map_err(|error| format!("request must be valid JSON: {error}"))?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    connection
        .execute(
            "INSERT INTO optimization_tasks
               (workspace_id, id, background_task_id, dataset_id, model_id, direction, method,
                trials_requested, state, request_json, result_json, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, NULL, NULL, ?10, ?10)",
            params![
                workspace_id,
                id,
                task.background_task_id,
                task.dataset_id,
                task.model_id,
                task.direction,
                task.method,
                task.trials_requested,
                task.request_json,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(id)
}

/// 依据后台任务的最新状态同步语义记录；返回是否命中了记录。
pub fn sync_state(
    connection: &Connection,
    workspace_id: &str,
    background_task_id: &str,
    state: &str,
    result_json: Option<&str>,
    error_message: Option<&str>,
) -> Result<(), String> {
    if !COMPUTE_TASK_STATES.contains(&state) {
        return Err(format!("unknown compute task state: {state}"));
    }
    let now = Utc::now().to_rfc3339();
    for table in ["prediction_tasks", "optimization_tasks"] {
        let sql = format!(
            "UPDATE {table}
             SET state = ?3, result_json = COALESCE(?4, result_json),
                 error_message = COALESCE(?5, error_message), updated_at = ?6
             WHERE workspace_id = ?1 AND background_task_id = ?2"
        );
        connection
            .execute(
                &sql,
                params![
                    workspace_id,
                    background_task_id,
                    state,
                    result_json,
                    error_message,
                    now
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn list(
    connection: &Connection,
    workspace_id: &str,
    kind: &str,
) -> Result<Vec<ComputeTaskRecord>, String> {
    let (table, direction, method, trials) = match kind {
        "prediction" => ("prediction_tasks", "NULL", "NULL", "NULL"),
        "optimization" => (
            "optimization_tasks",
            "direction",
            "method",
            "trials_requested",
        ),
        _ => return Err("compute task kind must be prediction or optimization".to_string()),
    };
    let sql = format!(
        "SELECT id, background_task_id, dataset_id, model_id, {direction}, {method}, {trials},
                state, request_json, result_json, error_message, created_at, updated_at
         FROM {table} WHERE workspace_id = ?1 ORDER BY created_at DESC LIMIT 200"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![workspace_id], |row| {
            Ok(ComputeTaskRecord {
                id: row.get(0)?,
                kind: kind.to_string(),
                background_task_id: row.get(1)?,
                dataset_id: row.get(2)?,
                model_id: row.get(3)?,
                direction: row.get(4)?,
                method: row.get(5)?,
                trials_requested: row.get(6)?,
                state: row.get(7)?,
                request_json: row.get(8)?,
                result_json: row.get(9)?,
                error_message: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

pub fn get(
    connection: &Connection,
    workspace_id: &str,
    id: &str,
) -> Result<Option<ComputeTaskRecord>, String> {
    connection
        .query_row(
            "SELECT id, background_task_id, dataset_id, model_id, state, request_json,
                    result_json, error_message, created_at, updated_at
             FROM prediction_tasks WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
            |row| {
                Ok(ComputeTaskRecord {
                    id: row.get(0)?,
                    kind: "prediction".to_string(),
                    background_task_id: row.get(1)?,
                    dataset_id: row.get(2)?,
                    model_id: row.get(3)?,
                    direction: None,
                    method: None,
                    trials_requested: None,
                    state: row.get(4)?,
                    request_json: row.get(5)?,
                    result_json: row.get(6)?,
                    error_message: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        create_optimization, create_prediction, get, list, state_for, sync_state,
        NewOptimizationTask, NewPredictionTask,
    };
    use crate::storage::migrations::migrate;
    use rusqlite::Connection;

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().expect("open database");
        migrate(&mut connection).expect("migrate database");
        connection
    }

    #[test]
    fn maps_background_states_to_semantic_states() {
        assert_eq!(state_for("queued"), "queued");
        assert_eq!(state_for("running"), "running");
        assert_eq!(state_for("completed"), "succeeded");
        assert_eq!(state_for("failed"), "failed");
        assert_eq!(state_for("interrupted"), "failed");
        assert_eq!(state_for("cancelled"), "cancelled");
    }

    #[test]
    fn records_and_syncs_prediction_and_optimization_tasks() {
        let connection = database();
        create_prediction(
            &connection,
            "ws",
            NewPredictionTask {
                background_task_id: "bg-1",
                dataset_id: Some("dataset-1"),
                model_id: Some("model-1"),
                request_json: r#"{"feature_values":[1.0]}"#,
            },
        )
        .expect("create prediction task");
        create_optimization(
            &connection,
            "ws",
            NewOptimizationTask {
                background_task_id: "bg-2",
                dataset_id: Some("dataset-1"),
                model_id: Some("model-1"),
                direction: "maximize",
                method: Some("nsga2"),
                trials_requested: 80,
                request_json: r#"{"objective_columns":[0]}"#,
            },
        )
        .expect("create optimization task");

        let predictions = list(&connection, "ws", "prediction").expect("list predictions");
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].state, "queued");
        assert_eq!(predictions[0].dataset_id.as_deref(), Some("dataset-1"));

        let optimizations = list(&connection, "ws", "optimization").expect("list optimizations");
        assert_eq!(optimizations[0].direction.as_deref(), Some("maximize"));
        assert_eq!(optimizations[0].trials_requested, Some(80));

        sync_state(
            &connection,
            "ws",
            "bg-1",
            "succeeded",
            Some(r#"{"predictions":[1.0]}"#),
            None,
        )
        .expect("sync prediction");
        sync_state(
            &connection,
            "ws",
            "bg-2",
            "failed",
            None,
            Some("budget exceeded"),
        )
        .expect("sync optimization");

        let predictions = list(&connection, "ws", "prediction").expect("relist");
        assert_eq!(predictions[0].state, "succeeded");
        assert!(predictions[0]
            .result_json
            .as_deref()
            .unwrap_or_default()
            .contains("predictions"));
        let optimizations = list(&connection, "ws", "optimization").expect("relist");
        assert_eq!(optimizations[0].state, "failed");
        assert_eq!(
            optimizations[0].error_message.as_deref(),
            Some("budget exceeded")
        );

        let fetched = get(&connection, "ws", &predictions[0].id)
            .expect("get")
            .expect("present");
        assert_eq!(fetched.state, "succeeded");
    }

    #[test]
    fn rejects_invalid_inputs() {
        let connection = database();
        assert!(create_optimization(
            &connection,
            "ws",
            NewOptimizationTask {
                background_task_id: "bg",
                dataset_id: None,
                model_id: None,
                direction: "sideways",
                method: None,
                trials_requested: 1,
                request_json: "{}",
            },
        )
        .is_err());
        assert!(create_prediction(
            &connection,
            "ws",
            NewPredictionTask {
                background_task_id: "bg",
                dataset_id: None,
                model_id: None,
                request_json: "not json",
            },
        )
        .is_err());
        assert!(sync_state(&connection, "ws", "bg", "unknown", None, None).is_err());
        assert!(list(&connection, "ws", "nope").is_err());
    }
}
