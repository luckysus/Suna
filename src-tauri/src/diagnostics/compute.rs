//! Reuse the scheduler's verified Worker configuration; explicit offline probe.
use crate::compute::{
    protocol::WorkerRequest,
    worker::{WorkerClient, WorkerConfig},
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub fn probe(config: Option<WorkerConfig>) -> Value {
    let Some(config) = config else {
        return json!({"state":"unavailable","errorCode":"worker_not_configured","networkUsed":false});
    };
    let start = Instant::now();
    let Ok(mut client) = WorkerClient::spawn(config) else {
        return json!({"state":"unavailable","errorCode":"worker_start_or_integrity_failed","networkUsed":false});
    };
    let result = client.request_with_progress_and_cancel(
        &WorkerRequest::new("diagnose", "diagnose", json!({})),
        |_| Ok(()),
        move || start.elapsed() > Duration::from_secs(20),
    );
    let report = match result {
        Ok(response) => response.get("result").cloned().unwrap_or_else(||json!({"state":"unavailable","errorCode":"unsupported_worker_diagnostics","networkUsed":false})),
        Err(_) => json!({"state":"unavailable","errorCode":"worker_probe_failed_or_timed_out","networkUsed":false}),
    };
    let _ = client.shutdown_with_timeout(
        &WorkerRequest::new("stop-diagnostics", "shutdown", json!({})),
        Duration::from_secs(2),
    );
    report
}

#[tauri::command]
pub async fn diagnose_compute_worker(app: tauri::AppHandle) -> Result<Value, String> {
    let config = crate::db::compute_worker_config(&app);
    tauri::async_runtime::spawn_blocking(move || probe(config))
        .await
        .map_err(|_| "worker_probe_unavailable".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_worker_reports_fixed_code_without_paths() {
        assert_eq!(probe(None)["errorCode"], "worker_not_configured");
        let result = probe(Some(WorkerConfig::new(std::path::PathBuf::from(
            "/private/invalid.exe",
        ))));
        assert_eq!(result["state"], "unavailable");
        assert!(!result.to_string().contains("/private"));
    }
}
