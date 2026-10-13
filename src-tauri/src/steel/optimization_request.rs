use serde_json::Value;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeSteelProcessRequest {
    pub dataset_id: String,
    #[serde(default)]
    pub objectives: Vec<Value>,
    #[serde(default)]
    pub variables: Vec<Value>,
    #[serde(default)]
    pub training_task_id: String,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub objective_columns: Vec<usize>,
    #[serde(default)]
    pub bounds: Vec<Value>,
    #[serde(default)]
    pub fixed_values: Vec<Option<f64>>,
    #[serde(default)]
    pub constraints: Vec<Value>,
    pub trials: u32,
    #[serde(default)]
    pub seed: i64,
    #[serde(default)]
    pub algorithm: Option<String>,
}
