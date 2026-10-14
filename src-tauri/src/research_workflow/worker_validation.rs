//! Reuse the existing offline diagnostic; only required modules enter the permit.
use super::schema::Template;
use crate::compute::worker::WorkerConfig;
use serde_json::{json, Value};
use std::collections::BTreeSet;
pub struct VerifiedWorker {
    evidence: Value,
}
impl VerifiedWorker {
    pub(crate) fn evidence(&self) -> &Value {
        &self.evidence
    }
}
pub fn probe(template: &Template, config: Option<WorkerConfig>) -> Result<VerifiedWorker, String> {
    from_report(template, crate::diagnostics::compute::probe(config))
}
fn from_report(template: &Template, report: Value) -> Result<VerifiedWorker, String> {
    if report["networkUsed"] != false || report["features"]["jointOptimization"] != true {
        return Err("workflow_worker_dependencies_unavailable".into());
    }
    let mut required = BTreeSet::from(["numpy", "scikit-learn", "scipy", "optuna"]);
    for model in &template.models {
        match model.algorithm.as_str() {
            "xgboost" => {
                required.insert("xgboost");
            }
            "lightgbm" => {
                required.insert("lightgbm");
            }
            _ => {}
        }
    }
    let dependencies = report["dependencies"]
        .as_array()
        .ok_or("workflow_worker_dependencies_unavailable")?;
    let mut verified = Vec::new();
    for name in required {
        let matching = dependencies
            .iter()
            .filter(|d| d["name"] == name)
            .collect::<Vec<_>>();
        if matching.len() != 1 || matching[0]["status"] != "ready" {
            return Err("workflow_worker_dependencies_unavailable".into());
        }
        let version = matching[0]["version"].as_str().filter(|v| {
            v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
        });
        verified.push(json!({"name":name,"status":"ready","version":version}));
    }
    let text = |key: &str| {
        report[key].as_str().filter(|v| {
            v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        })
    };
    Ok(VerifiedWorker {
        evidence: json!({"validationStatus":"actual_offline_diagnostic","dependencies":verified,
        "workerVersion":text("workerVersion"),"protocolVersion":text("protocolVersion"),"networkUsed":false}),
    })
}
pub fn annotate(mut preview: Value, worker: &VerifiedWorker) -> Result<Value, String> {
    preview["workerValidation"] = worker.evidence.clone();
    preview
        .as_object_mut()
        .ok_or("workflow_preview_invalid")?
        .remove("dryRunSha256");
    preview["dryRunSha256"] = json!(format!(
        "{:x}",
        sha2::Sha256::digest(preview.to_string().as_bytes())
    ));
    Ok(preview)
}
use sha2::Digest;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_workflow::testing::fixture;
    fn report() -> Value {
        json!({"networkUsed":false,"workerVersion":"1.0.0","protocolVersion":"1.0","features":{"jointOptimization":true},"dependencies":[{"name":"numpy","status":"ready","version":"1"},{"name":"scipy","status":"ready","version":"1"},{"name":"scikit-learn","status":"ready","version":"1"},{"name":"optuna","status":"ready","version":"1"}],"private_path":"C:/synthetic-private","secret":"synthetic-token"})
    }
    #[test]
    fn failed_required_dependencies_block_before_jobs_without_leaking_diagnostic_fields() {
        let f = fixture();
        let mut bad = report();
        bad["dependencies"][0]["status"] = json!("unavailable");
        assert!(from_report(&f.template, bad).is_err());
        assert!(probe(&f.template, None).is_err());
        let verified = from_report(&f.template, report()).unwrap();
        let bytes = verified.evidence.to_string();
        assert!(!bytes.contains("synthetic-private"));
        assert!(!bytes.contains("synthetic-token"));
        assert_eq!(
            f.connection
                .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn diagnostic_version_change_invalidates_the_approved_preview() {
        let f = fixture();
        let base =
            super::super::dry_run_on_connection(&f.connection, "local", &f.template, &f.input)
                .unwrap();
        let a = annotate(base.clone(), &from_report(&f.template, report()).unwrap()).unwrap();
        let mut changed = report();
        changed["dependencies"][0]["version"] = json!("2");
        let b = annotate(base, &from_report(&f.template, changed).unwrap()).unwrap();
        assert_ne!(a["dryRunSha256"], b["dryRunSha256"]);
    }

    #[test]
    fn changed_diagnostic_blocks_submission_and_checked_preview_is_bound_to_the_definition() {
        let (mut f, _) = crate::research_workflow::runtime_support::setup();
        let worker = from_report(&f.template, report()).unwrap();
        let base =
            super::super::dry_run_on_connection(&f.connection, "local", &f.template, &f.input)
                .unwrap();
        let approved = annotate(base, &worker).unwrap();
        let mut r = report();
        r["dependencies"][0]["version"] = json!("changed");
        let changed = from_report(&f.template, r).unwrap();
        let nonce = uuid::Uuid::new_v4();
        assert_eq!(
            crate::research_workflow::runtime::submit_checked(
                &mut f.connection,
                "local",
                &f.template,
                &f.input,
                approved["dryRunSha256"].as_str().unwrap(),
                nonce,
                true,
                &changed
            )
            .unwrap_err(),
            "workflow_preview_changed"
        );
        assert_eq!(
            f.connection
                .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        let root = crate::research_workflow::runtime::submit_checked(
            &mut f.connection,
            "local",
            &f.template,
            &f.input,
            approved["dryRunSha256"].as_str().unwrap(),
            nonce,
            true,
            &worker,
        )
        .unwrap();
        let (id, _) = super::super::provenance::definition(&f.connection, &root).unwrap();
        let def =
            crate::storage::repositories::research_artifacts::get(&f.connection, "local", &id)
                .unwrap()
                .payload
                .unwrap();
        assert_eq!(def["request"]["worker_validation"], worker.evidence);
        assert_eq!(def["request"]["dry_run_sha256"], approved["dryRunSha256"]);
    }
}
