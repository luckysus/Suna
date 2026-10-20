use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Duration;
use tauri::Emitter;

static DOWNLOAD_TASKS: Mutex<Vec<OllamaDownloadTaskRecord>> = Mutex::new(Vec::new());

#[derive(Debug, Clone, Serialize)]
pub struct OllamaDownloadTaskRecord {
    pub id: String,
    pub model_name: String,
    pub status: String,
    pub progress: f64,
    pub message: String,
}

pub fn list_tasks() -> Vec<OllamaDownloadTaskRecord> {
    DOWNLOAD_TASKS
        .lock()
        .map(|tasks| tasks.clone())
        .unwrap_or_default()
}

fn upsert_task(record: OllamaDownloadTaskRecord) {
    if let Ok(mut tasks) = DOWNLOAD_TASKS.lock() {
        if let Some(existing) = tasks.iter_mut().find(|task| task.id == record.id) {
            *existing = record;
        } else {
            tasks.push(record);
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OllamaStatus {
    pub reachable: bool,
    pub base_url: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaModelTag {
    pub name: String,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct OllamaDownloadProgress {
    pub task_id: String,
    pub model_name: String,
    pub status: String,
    pub progress: f64,
    pub message: String,
}

fn normalize_base_url(base_url: Option<&str>) -> String {
    let trimmed = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("http://127.0.0.1:11434");
    trimmed.trim_end_matches('/').to_string()
}

pub async fn check_status(base_url: Option<String>) -> OllamaStatus {
    let base = normalize_base_url(base_url.as_deref());
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build();
    let client = match client {
        Ok(client) => client,
        Err(error) => {
            return OllamaStatus {
                reachable: false,
                base_url: base,
                message: Some(error.to_string()),
            };
        }
    };
    let url = format!("{base}/api/tags");
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => OllamaStatus {
            reachable: true,
            base_url: base,
            message: None,
        },
        Ok(response) => OllamaStatus {
            reachable: false,
            base_url: base,
            message: Some(format!("Ollama 返回 HTTP {}", response.status())),
        },
        Err(error) => OllamaStatus {
            reachable: false,
            base_url: base,
            message: Some(error.to_string()),
        },
    }
}

pub async fn list_models(base_url: Option<String>) -> Result<Vec<OllamaModelTag>, String> {
    let base = normalize_base_url(base_url.as_deref());
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(format!("{base}/api/tags"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Ollama 列表失败: HTTP {}", response.status()));
    }
    let payload: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    let models = payload
        .get("models")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(models
        .into_iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?.to_string();
            let size = entry.get("size").and_then(|value| value.as_u64());
            Some(OllamaModelTag { name, size })
        })
        .collect())
}

pub async fn download_model_with_events(
    app: tauri::AppHandle,
    base_url: Option<String>,
    model_name: String,
) -> Result<(), String> {
    let base = normalize_base_url(base_url.as_deref());
    let task_id = uuid::Uuid::new_v4().to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(format!("{base}/api/pull"))
        .json(&serde_json::json!({ "name": model_name, "stream": true }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("Ollama 拉取失败: HTTP {}", response.status()));
    }
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        for line in chunk.split(|byte| *byte == b'\n') {
            if line.is_empty() {
                continue;
            }
            let payload: serde_json::Value =
                serde_json::from_slice(line).unwrap_or(serde_json::Value::Null);
            let status = payload
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("downloading");
            let completed = payload.get("completed").and_then(|value| value.as_f64());
            let total = payload.get("total").and_then(|value| value.as_f64());
            let progress = match (completed, total) {
                (Some(completed), Some(total)) if total > 0.0 => completed / total,
                _ => 0.0,
            };
            let event = OllamaDownloadProgress {
                task_id: task_id.clone(),
                model_name: model_name.clone(),
                status: status.to_string(),
                progress,
                message: status.to_string(),
            };
            upsert_task(OllamaDownloadTaskRecord {
                id: task_id.clone(),
                model_name: model_name.clone(),
                status: event.status.clone(),
                progress: event.progress,
                message: event.message.clone(),
            });
            let _ = app.emit("ollama-download-progress", &event);
            if status.eq_ignore_ascii_case("success") {
                let _ = app.emit(
                    "ollama-download-complete",
                    serde_json::json!({
                        "task_id": task_id,
                        "model_name": model_name,
                        "status": "success",
                    }),
                );
                return Ok(());
            }
        }
    }
    Ok(())
}
