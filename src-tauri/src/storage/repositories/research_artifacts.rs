//! Immutable local research provenance. Task state remains in background_tasks.
use super::research_projects as projects;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
pub fn installed(c: &Connection) -> bool {
    c.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='research_runs')",
        [],
        |r| r.get(0),
    )
    .unwrap_or(false)
}
/// Savepoints also work inside the Agent's existing submission transaction.
pub fn atomic<T>(c: &Connection, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let name = format!("research_{}", Uuid::new_v4().simple());
    c.execute_batch(&format!("SAVEPOINT {name}")).map_err(err)?;
    match f() {
        Ok(v) => {
            c.execute_batch(&format!("RELEASE {name}")).map_err(err)?;
            Ok(v)
        }
        Err(e) => {
            c.execute_batch(&format!("ROLLBACK TO {name}; RELEASE {name}"))
                .map_err(err)?;
            Err(e)
        }
    }
}
pub fn current(c: &Connection, ws: &str) -> Result<Option<String>, String> {
    let saved:Option<String>=c.query_row("SELECT value_json FROM settings WHERE workspace_id=?1 AND key='suna.research.current_project'",[ws],|r|r.get(0)).optional().map_err(err)?;
    saved
        .map(|s| {
            serde_json::from_str::<Option<String>>(&s)
                .map_err(|_| "invalid_current_research_project".into())
        })
        .transpose()
        .map(Option::flatten)
}
// Only known metadata keys are copied from worker/model results. Binary artifacts,
// prompts, credentials, filesystem paths and raw training rows never enter snapshots.
pub fn safe_metadata(value: &Value) -> Value {
    const KEYS: &[&str] = &[
        "material_rule_snapshot", "material_assessment", "compiled", "projectId", "fieldId", "canonicalUnit", "schemaVersion", "ruleSetHash", "ruleId", "sourceRef", "sourceVersion", "provenance", "type", "reviewed", "demonstration", "enabled", "hardFeasible", "softPenalty", "supportRisk", "satisfied", "margin", "version", "values", "prediction", "deterministic_seed", "notes", "featureNames", "canonicalField", "originalName", "inferredType", "targetName", "dataVersion", "artifact",
        "dataset_id",
        "data_version",
        "source_sha256",
        "model_sha256",
        "training_task_id",
        "model_id",
        "algorithm",
        "method",
        "seed",
        "random_seed",
        "kind",
        "version",
        "feature_names",
        "feature_columns",
        "target_column",
        "target_name",
        "target_names",
        "unit",
        "units",
        "direction",
        "directions",
        "name",
        "source",
        "weight",
        "coefficients",
        "intercept",
        "min",
        "max",
        "low",
        "high",
        "fixedValue",
        "value",
        "offset",
        "bounds",
        "fixed_values",
        "variables",
        "objectives",
        "objective_specs",
        "constraints",
        "split_policy",
        "policy",
        "validation_fraction",
        "test_fraction",
        "calibration_fraction",
        "group_column",
        "time_column",
        "evaluation",
        "metrics",
        "train",
        "validation",
        "test",
        "mae",
        "mse",
        "rmse",
        "r2",
        "coverage",
        "nominal_coverage",
        "calibration_count",
        "learning_curve",
        "train_count",
        "validation_count",
        "test_count",
        "sample_count",
        "row_count",
        "column_count",
        "columns",
        "ordinal",
        "original_name",
        "canonical_field",
        "inferred_type",
        "min_value",
        "max_value",
        "predictions",
        "points",
        "recommendations",
        "features",
        "predicted_value",
        "objective_values",
        "objective_names",
        "pareto",
        "pareto_front",
        "feasible",
        "trials",
        "trials_requested",
        "trials_completed",
        "state",
        "applicability_warnings",
        "warnings",
        "confidence",
        "uncertainty",
        "uncertainty_method",
        "value",
        "score",
        "ranking",
        "summary",
        "correlation",
        "statistics",
        "mean",
        "std",
        "count",
        "missing_count",
        "missing",
        "q1",
        "q3",
        "median",
        "iqr",
        "histogram",
        "counts",
        "edges",
        "percentile",
        "actual_variables",
        "measurements",
        "property",
        "measured_at",
        "sample_id",
        "batch_id",
        "heat_id",
        "source_type",
        "review_state",
        "execution_id",
        "measurement_id",
        "note",
        "reviewed_by",
        "reviewed_at",
        "review_note",
        "mapping_hash",
        "schema_version",
        "rule_snapshot",
        "rules",
        "rule_id",
        "severity",
        "expression",
        "terms",
        "field",
        "operator",
        "rhs",
        "review_status",
        "source_version",
        "source_ref",
        "tolerance",
        "penalty",
        "support",
        "training_ranges",
        "rule_set_hash",
    ];
    match value {
        Value::Object(map)
            if !map.is_empty()
                && map
                    .iter()
                    .all(|(k, v)| k.len() <= 128 && v.as_f64().is_some_and(f64::is_finite)) =>
        {
            value.clone()
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(k, _)| KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), safe_metadata(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(safe_metadata).collect()),
        Value::String(s) => Value::String(s.chars().take(4096).collect()),
        other => other.clone(),
    }
}
pub fn safe_request(value: &Value) -> Value {
    let mut request = value.clone();
    if let Some(map) = request.as_object_mut() {
        for key in ["features", "targets", "groups", "timestamps"] {
            map.remove(key);
        }
    }
    safe_metadata(&request)
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub title: String,
    pub schema_version: String,
    pub content_sha256: String,
    pub state: String,
    pub source_run_id: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
}
fn row(r: &rusqlite::Row) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: r.get(0)?,
        project_id: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        schema_version: r.get(4)?,
        content_sha256: r.get(5)?,
        state: r.get(6)?,
        source_run_id: r.get(7)?,
        created_at: r.get(8)?,
        payload: None,
    })
}
const SELECT:&str="SELECT id,project_id,kind,title,schema_version,content_sha256,state,source_run_id,created_at FROM research_artifacts";
pub fn create(
    c: &Connection,
    ws: &str,
    p: &str,
    kind: &str,
    title: &str,
    key: &str,
    run: Option<&str>,
    payload: Value,
) -> Result<String, String> {
    projects::get(c, ws, p)?;
    if ![
        "dataset_snapshot",
        "model_snapshot",
        "prediction_result",
        "optimization_result",
        "experiment_plan",
        "experiment_measurement",
        "literature_evidence",
        "chart",
        "report",
        "analysis_result",
    ]
    .contains(&kind)
        || title.chars().count() > 300
    {
        return Err("invalid_artifact".into());
    }
    if let Some(run) = run {
        let ok:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND id=?3)",params![ws,p,run],|r|r.get(0)).map_err(err)?;
        if !ok {
            return Err("research_run_not_found".into());
        }
    }
    let body = payload.to_string();
    if body.len() > 2_000_000 {
        return Err("research_artifact_too_large".into());
    }
    let sha = format!("{:x}", Sha256::digest(body.as_bytes()));
    if let Some((id,existing))=c.query_row("SELECT id,content_sha256 FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind=?3 AND source_key=?4",params![ws,p,kind,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(err)?{if existing!=sha{return Err("immutable_artifact_conflict".into())}return Ok(id)}
    let id = Uuid::new_v4().to_string();
    c.execute("INSERT INTO research_artifacts(workspace_id,id,project_id,kind,title,schema_version,immutable_payload_json,content_sha256,source_key,source_run_id,created_at,created_by) VALUES(?1,?2,?3,?4,?5,'research-artifact.v1',?6,?7,?8,?9,?10,'local_runtime')",params![ws,id,p,kind,title,body,sha,key,run,Utc::now().to_rfc3339()]).map_err(err)?;
    c.execute("INSERT OR IGNORE INTO research_project_links VALUES(?1,?2,'artifact',?3,'generated_by',?4,'local_runtime')",params![ws,p,id,Utc::now().to_rfc3339()]).map_err(err)?;
    Ok(id)
}
pub fn get(c: &Connection, ws: &str, id: &str) -> Result<Artifact, String> {
    let mut a = c
        .query_row(
            &format!("{SELECT} WHERE workspace_id=?1 AND id=?2"),
            params![ws, id],
            row,
        )
        .optional()
        .map_err(err)?
        .ok_or("research_artifact_not_found")?;
    let body: String = c
        .query_row(
            "SELECT immutable_payload_json FROM research_artifacts WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| r.get(0),
        )
        .map_err(err)?;
    if format!("{:x}", Sha256::digest(body.as_bytes())) != a.content_sha256 {
        return Err("artifact_hash_mismatch".into());
    }
    a.payload = Some(serde_json::from_str(&body).map_err(|e| e.to_string())?);
    Ok(a)
}
pub fn list(
    c: &Connection,
    ws: &str,
    p: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Artifact>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare(&format!("{SELECT} WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at DESC,id LIMIT ?3 OFFSET ?4")).map_err(err)?;
    let result = q
        .query_map(params![ws, p, limit.clamp(1, 100), offset], row)
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err);
    result
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
}
pub fn edge(c: &Connection, ws: &str, from: &str, to: &str, relation: &str) -> Result<(), String> {
    let a = get(c, ws, from)?;
    let b = get(c, ws, to)?;
    if a.project_id != b.project_id || from == to {
        return Err("invalid_artifact_edge".into());
    }
    let cycle:bool=c.query_row("WITH RECURSIVE reachable(id) AS (SELECT to_artifact_id FROM research_artifact_edges WHERE workspace_id=?1 AND from_artifact_id=?2 UNION SELECT e.to_artifact_id FROM research_artifact_edges e JOIN reachable r ON r.id=e.from_artifact_id WHERE e.workspace_id=?1) SELECT EXISTS(SELECT 1 FROM reachable WHERE id=?3)",params![ws,to,from],|r|r.get(0)).map_err(err)?;
    if cycle {
        return Err("artifact_lineage_cycle".into());
    }
    c.execute(
        "INSERT OR IGNORE INTO research_artifact_edges VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            ws,
            Uuid::new_v4().to_string(),
            from,
            to,
            relation,
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(err)?;
    Ok(())
}
pub fn edges(
    c: &Connection,
    ws: &str,
    p: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Edge>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT e.from_artifact_id,e.to_artifact_id,e.relation FROM research_artifact_edges e JOIN research_artifacts a ON a.workspace_id=e.workspace_id AND a.id=e.from_artifact_id WHERE e.workspace_id=?1 AND a.project_id=?2 ORDER BY e.created_at,e.id LIMIT ?3 OFFSET ?4").map_err(err)?;
    let result = q
        .query_map(params![ws, p, limit.clamp(1, 100), offset], |r| {
            Ok(Edge {
                from_id: r.get(0)?,
                to_id: r.get(1)?,
                relation: r.get(2)?,
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err);
    result
}
fn kind(task: &str) -> Option<(&str, &str)> {
    if task.starts_with("compute_train_") {
        Some(("training", "model_snapshot"))
    } else if task.starts_with("compute_predict_") {
        Some(("prediction", "prediction_result"))
    } else if task == "compute_optimize_constrained" {
        Some(("optimization", "optimization_result"))
    } else if task == "compute_design_experiments" {
        Some(("experiment_design", "experiment_plan"))
    } else {
        None
    }
}
pub fn bind_current(c: &Connection, ws: &str, task: &str) -> Result<(), String> {
    if !installed(c) {
        return Ok(());
    }
    let task_kind: String = c
        .query_row(
            "SELECT kind FROM background_tasks WHERE workspace_id=?1 AND id=?2",
            params![ws, task],
            |r| r.get(0),
        )
        .map_err(err)?;
    if kind(&task_kind).is_some() {
        if let Some(p) = current(c, ws)? {
            projects::require_active(c, ws, &p)?;
            bind(c, ws, &p, task)?;
        }
    }
    Ok(())
}
pub fn bind(c: &Connection, ws: &str, p: &str, task: &str) -> Result<String, String> {
    projects::get(c, ws, p)?;
    let task_kind: String = c
        .query_row(
            "SELECT kind FROM background_tasks WHERE workspace_id=?1 AND id=?2",
            params![ws, task],
            |r| r.get(0),
        )
        .optional()
        .map_err(err)?
        .ok_or("research_task_not_found")?;
    let (kind, _) = kind(&task_kind).unwrap_or(("custom", "analysis_result"));
    let id = Uuid::new_v4().to_string();
    let inserted = c
        .execute(
            "INSERT OR IGNORE INTO research_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![ws, id, p, task_kind, kind, task, Utc::now().to_rfc3339()],
        )
        .map_err(err)?;
    let id:String=c.query_row("SELECT id FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND background_task_id=?3",params![ws,p,task],|r|r.get(0)).map_err(err)?;
    c.execute("INSERT OR IGNORE INTO research_project_links VALUES(?1,?2,'background_task',?3,'generated_by',?4,'local_runtime')",params![ws,p,task,Utc::now().to_rfc3339()]).map_err(err)?;
    if inserted > 0 {
        if let Some(data) = snapshot_dataset(c, ws, p, task)? {
            c.execute(
                "INSERT OR IGNORE INTO research_run_sources VALUES(?1,?2,?3,'uses_dataset')",
                params![ws, id, data],
            )
            .map_err(err)?;
        }
        let raw: String = c
            .query_row(
                "SELECT payload_json FROM background_tasks WHERE workspace_id=?1 AND id=?2",
                params![ws, task],
                |r| r.get(0),
            )
            .map_err(err)?;
        let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let mut models = Vec::new();
        if let Some(model) = payload["payload"]["training_task_id"].as_str() {
            models.push(model.to_string());
        }
        if let Some(specs) = payload["payload"]["objective_specs"].as_array() {
            for spec in specs {
                if let Some(model) = spec["model_id"].as_str() {
                    models.push(model.to_string());
                }
            }
        }
        models.sort();
        models.dedup();
        for model in models {
            if model == task {
                return Err("self_model_reference".into());
            }
            let mt: Option<String> = c
                .query_row(
                    "SELECT kind FROM background_tasks WHERE workspace_id=?1 AND id=?2",
                    params![ws, model],
                    |r| r.get(0),
                )
                .optional()
                .map_err(err)?;
            if !mt.is_some_and(|k| k.starts_with("compute_train_")) {
                return Err("invalid_research_model_source".into());
            }
            bind(c, ws, p, &model)?;
            reconcile_task(c, ws, &model)?;
            if let Some(artifact)=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='model_snapshot' AND source_key=?3",params![ws,p,model],|r|r.get::<_,String>(0)).optional().map_err(err)?{c.execute("INSERT OR IGNORE INTO research_run_sources VALUES(?1,?2,?3,'uses_model')",params![ws,id,artifact]).map_err(err)?;}
        }
    }
    Ok(id)
}
fn snapshot_dataset(
    c: &Connection,
    ws: &str,
    p: &str,
    task: &str,
) -> Result<Option<String>, String> {
    let raw: String = c
        .query_row(
            "SELECT payload_json FROM background_tasks WHERE workspace_id=?1 AND id=?2",
            params![ws, task],
            |r| r.get(0),
        )
        .map_err(err)?;
    let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let Some(id) = payload["payload"]["dataset_id"].as_str() else {
        return Ok(None);
    };
    dataset_snapshot(c, ws, p, id)
}
pub fn dataset_snapshot(
    c: &Connection,
    ws: &str,
    p: &str,
    id: &str,
) -> Result<Option<String>, String> {
    let Some(d) = super::steel::get(c, ws, id)? else {
        return Ok(None);
    };
    let columns = json!(d.columns.iter().map(|c|json!({"ordinal":c.ordinal,"original_name":c.original_name,"canonical_field":c.canonical_field,"unit":c.unit,"inferred_type":c.inferred_type,"min":c.min,"max":c.max})).collect::<Vec<_>>());
    let columns = safe_metadata(&columns);
    let mapping_hash = format!("{:x}", Sha256::digest(columns.to_string().as_bytes()));
    let key = format!("{}:{}:{}", d.id, d.source_sha256, mapping_hash);
    let artifact = create(
        c,
        ws,
        p,
        "dataset_snapshot",
        "数据集版本",
        &key,
        None,
        json!({"dataset_id":d.id,"source_sha256":d.source_sha256,"row_count":d.row_count,"column_count":d.column_count,"columns":columns,"mapping_hash":mapping_hash}),
    )?;
    Ok(Some(artifact))
}
pub fn reconcile_task(c: &Connection, ws: &str, task: &str) -> Result<(), String> {
    if !installed(c) {
        return Ok(());
    }
    let mut q=c.prepare("SELECT id,project_id FROM research_runs WHERE workspace_id=?1 AND background_task_id=?2").map_err(err)?;
    let runs = q
        .query_map(params![ws, task], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    for (run, p) in runs {
        let record:Option<(String,String,String,Option<String>)>=c.query_row("SELECT kind,state,payload_json,checkpoint_json FROM background_tasks WHERE workspace_id=?1 AND id=?2",params![ws,task],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(err)?;
        let Some((task_kind, state, raw, checkpoint)) = record else {
            continue;
        };
        if state != "completed" {
            continue;
        }
        let Some((_, artifact_kind)) = kind(&task_kind) else {
            continue;
        };
        let cp: Value =
            serde_json::from_str(checkpoint.as_deref().ok_or("missing_compute_checkpoint")?)
                .map_err(|e| e.to_string())?;
        if cp.get("result").is_none() {
            return Err("missing_compute_result".into());
        }
        let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let mut frozen=json!({"request":safe_request(&payload["payload"]),"result":safe_metadata(&cp["result"]),"producer":{"app_version":env!("CARGO_PKG_VERSION"),"worker_protocol":"1.0"}});
        if let Some(raw)=payload["payload"].get("material_rule_snapshot") {let snapshot:super::material_rules::RuleSnapshot=serde_json::from_value(raw.clone()).map_err(|e|e.to_string())?;frozen["request"]["material_rule_snapshot"]=json!(snapshot);frozen["result"]["material_rule_snapshot"]=raw.clone();}
        let existing:Option<String>=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind=?3 AND source_key=?4",params![ws,p,artifact_kind,task],|r|r.get(0)).optional().map_err(err)?;
        let output = if let Some(id)=existing{get(c,ws,&id)?;id}else{
        create(
            c,
            ws,
            &p,
            artifact_kind,
            &task_kind,
            task,
            Some(&run),
            frozen,
        )?};
        let mut sources=c.prepare("SELECT artifact_id,relation FROM research_run_sources WHERE workspace_id=?1 AND run_id=?2").map_err(err)?;
        let inputs = sources
            .query_map(params![ws, run], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(err)?;
        for (input, relation) in inputs {
            edge(c, ws, &input, &output, &relation)?;
        }
    }
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub background_task_id: Option<String>,
    pub state: String,
    pub progress: i64,
    pub error_code: Option<String>,
    pub origin: String,
    pub agent_run_id: Option<String>,
    pub created_at: String,
}
pub fn runs(
    c: &Connection,
    ws: &str,
    p: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Run>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare("SELECT r.id,r.title,r.kind,r.background_task_id,COALESCE(t.state,'unavailable'),COALESCE(t.progress,0),t.error_code,CASE WHEN s.run_id IS NULL THEN 'ui' ELSE 'agent' END,s.run_id,r.created_at FROM research_runs r LEFT JOIN background_tasks t ON t.workspace_id=r.workspace_id AND t.id=r.background_task_id LEFT JOIN agent_task_sources s ON s.workspace_id=r.workspace_id AND s.task_id=r.background_task_id WHERE r.workspace_id=?1 AND r.project_id=?2 ORDER BY r.created_at DESC,r.id LIMIT ?3 OFFSET ?4").map_err(err)?;
    let result = q
        .query_map(params![ws, p, limit.clamp(1, 100), offset], |r| {
            Ok(Run {
                id: r.get(0)?,
                title: r.get(1)?,
                kind: r.get(2)?,
                background_task_id: r.get(3)?,
                state: r.get(4)?,
                progress: r.get(5)?,
                error_code: r.get(6)?,
                origin: r.get(7)?,
                agent_run_id: r.get(8)?,
                created_at: r.get(9)?,
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err);
    result
}
pub fn reconcile_project(c: &Connection, ws: &str, p: &str) -> Result<(), String> {
    // Bounded repair after a crash; later pages reconcile their own task IDs.
    for run in runs(c, ws, p, 100, 0)? {
        if let Some(task) = run.background_task_id {
            atomic(c, || reconcile_task(c, ws, &task))?;
        }
    }
    Ok(())
}
pub fn record_analysis(
    c: &Connection,
    ws: &str,
    dataset: &str,
    result: Value,
) -> Result<(), String> {
    if !installed(c) {
        return Ok(());
    }
    let Some(p) = current(c, ws)? else {
        return Ok(());
    };
    projects::require_active(c, ws, &p)?;
    atomic(c, || {
        let data = dataset_snapshot(c, ws, &p, dataset)?;
        let safe = safe_metadata(&result);
        let key = format!("{:x}", Sha256::digest(safe.to_string().as_bytes()));
        let id = create(c, ws, &p, "analysis_result", "数据分析", &key, None, safe)?;
        if let Some(data) = data {
            edge(c, ws, &data, &id, "uses_dataset")?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::super::research_projects::SaveResearchProject;
    use super::*;
    fn fixture() -> (Connection, String) {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            SaveResearchProject {
                id: None,
                name: "合成测试项目".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        c.execute(
            "INSERT INTO settings VALUES('ws','suna.research.current_project',?1,'now')",
            [json!(p.id).to_string()],
        )
        .unwrap();
        (c, p.id)
    }
    fn task(c: &Connection, kind: &str, payload: Value) -> String {
        crate::tasks::repository::create(
            c,
            crate::tasks::NewTask {
                workspace_id: "ws".into(),
                kind: kind.into(),
                payload_json: json!({"operation":"test","payload":payload}).to_string(),
                checkpoint_json: None,
                next_run_at: None,
                progress: 0,
            },
        )
        .unwrap()
        .id
        .to_string()
    }
    fn complete(c: &Connection, id: &str, result: Value) {
        c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",params![id,json!({"result":result}).to_string()]).unwrap();
        reconcile_task(c, "ws", id).unwrap();
    }
    #[test]
    fn maps_single_state_and_reconciles_immutable_outputs() {
        let (c, p) = fixture();
        let t = task(
            &c,
            "compute_train_linear_regression",
            json!({"seed":42,"api_key":"secret","features":[[1,2]]}),
        );
        assert_eq!(runs(&c, "ws", &p, 50, 0).unwrap()[0].state, "queued");
        complete(
            &c,
            &t,
            json!({"artifact":{"feature_names":["C"],"model_base64":"secret-model"},"metrics":{"rmse":1},"access_token":"secret"}),
        );
        reconcile_task(&c, "ws", &t).unwrap();
        let a = list(&c, "ws", &p, 50, 0).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(runs(&c, "ws", &p, 50, 0).unwrap()[0].state, "completed");
        let body = get(&c, "ws", &a[0].id)
            .unwrap()
            .payload
            .unwrap()
            .to_string();
        assert!(!body.contains("secret"));
        assert!(body.contains("feature_names"));
        assert!(c
            .execute("UPDATE research_artifacts SET title='changed'", [])
            .is_err());
        assert!(get(&c, "other", &a[0].id).is_err());
    }
    #[test]
    fn lineage_rejects_cycles_self_cross_project_and_duplicates() {
        let (c, p) = fixture();
        let a = create(&c, "ws", &p, "analysis_result", "A", "a", None, json!({})).unwrap();
        let b = create(&c, "ws", &p, "analysis_result", "B", "b", None, json!({})).unwrap();
        edge(&c, "ws", &a, &b, "uses_dataset").unwrap();
        edge(&c, "ws", &a, &b, "uses_dataset").unwrap();
        assert_eq!(edges(&c, "ws", &p, 100, 0).unwrap().len(), 1);
        assert!(edge(&c, "ws", &b, &a, "uses_model").is_err());
        assert!(edge(&c, "ws", &a, &a, "uses_model").is_err());
        assert!(edge(&c, "other", &a, &b, "uses_model").is_err());
        assert!(create(
            &c,
            "ws",
            &p,
            "analysis_result",
            "A",
            "a",
            None,
            json!({"revision":1})
        )
        .is_err());
    }
    #[test]
    fn failed_cancelled_and_missing_tasks_never_fabricate_outputs() {
        let (c, p) = fixture();
        let t = task(&c, "compute_train_linear_regression", json!({}));
        c.execute(
            "UPDATE background_tasks SET state='failed',error_code='compute_failed' WHERE id=?1",
            [&t],
        )
        .unwrap();
        reconcile_project(&c, "ws", &p).unwrap();
        assert_eq!(
            runs(&c, "ws", &p, 10, 0).unwrap()[0].error_code.as_deref(),
            Some("compute_failed")
        );
        assert!(list(&c, "ws", &p, 10, 0).unwrap().is_empty());
        c.execute(
            "UPDATE background_tasks SET state='cancelled',error_code=NULL WHERE id=?1",
            [&t],
        )
        .unwrap();
        assert_eq!(runs(&c, "ws", &p, 10, 0).unwrap()[0].state, "cancelled");
        c.execute("DELETE FROM background_tasks WHERE id=?1", [t])
            .unwrap();
        assert_eq!(runs(&c, "ws", &p, 10, 0).unwrap()[0].state, "unavailable");
    }
    #[test]
    fn submission_rollback_works_inside_an_agent_transaction() {
        let (mut c, p) = fixture();
        projects::archive(&mut c, "ws", &p, true).unwrap();
        let tx = c.transaction().unwrap();
        let result = crate::tasks::repository::create(
            &tx,
            crate::tasks::NewTask {
                workspace_id: "ws".into(),
                kind: "compute_train_linear_regression".into(),
                payload_json: "{}".into(),
                checkpoint_json: None,
                next_run_at: None,
                progress: 0,
            },
        );
        assert!(result.is_err());
        assert_eq!(
            tx.query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        tx.commit().unwrap();
    }
    #[test]
    fn joint_output_links_two_independent_models() {
        let (c, p) = fixture();
        let m1 = task(&c, "compute_train_linear_regression", json!({"seed":1}));
        let m2 = task(&c, "compute_train_sklearn_model", json!({"seed":2}));
        complete(&c, &m1, json!({"artifact":{"feature_names":["C"]}}));
        complete(&c, &m2, json!({"artifact":{"feature_names":["C"]}}));
        let t = task(
            &c,
            "compute_optimize_constrained",
            json!({"objective_specs":[{"model_id":m1},{"model_id":m2}],"bounds":[{"min":0,"max":1}]}),
        );
        complete(
            &c,
            &t,
            json!({"recommendations":[{"features":{"C":0.4},"objective_values":[100,50]}]}),
        );
        let edges = edges(&c, "ws", &p, 100, 0).unwrap();
        assert_eq!(
            edges.iter().filter(|e| e.relation == "uses_model").count(),
            2
        );
    }
}

pub fn imported_dataset(c: &Connection, ws: &str, dataset: &str) -> Result<(), String> {
    if !installed(c) {
        return Ok(());
    }
    if let Some(p) = current(c, ws)? {
        projects::require_active(c, ws, &p)?;
        projects::attach_in_transaction(c, ws, &p, "dataset", dataset, "attached")?;
        dataset_snapshot(c, ws, &p, dataset)?;
    }
    Ok(())
}
pub fn registered_model(c: &Connection, ws: &str, id: &str) -> Result<(), String> {
    if !installed(c) {
        return Ok(());
    }
    if let Some(p) = current(c, ws)? {
        projects::require_active(c, ws, &p)?;
        let model = super::steel_models::get(c, ws, id)?.ok_or("model_not_found")?;
        projects::attach_in_transaction(c, ws, &p, "model", id, "attached")?;
        let a = create(
            c,
            ws,
            &p,
            "model_snapshot",
            "注册模型版本",
            &format!("registered:{id}"),
            None,
            json!({"model_id":id,"model_sha256":model.model_sha256,"kind":model.kind,"version":model.version,"training_task_id":model.source_task_id,"manifest":safe_metadata(&serde_json::from_str::<Value>(&model.manifest_json).map_err(|e|e.to_string())?)}),
        )?;
        if let Some(task) = model.source_task_id {
            bind(c, ws, &p, &task)?;
            reconcile_task(c, ws, &task)?;
            if let Some(source)=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='model_snapshot' AND source_key=?3",params![ws,p,task],|r|r.get::<_,String>(0)).optional().map_err(err)?{edge(c,ws,&source,&a,"uses_model")?;}
        }
    }
    Ok(())
}
