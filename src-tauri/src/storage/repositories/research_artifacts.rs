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
    if let Some(p) = super::research_delivery::scope(c)? {
        return Ok(p);
    }
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
#[path = "research_artifacts/tests.rs"]
mod tests;
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

mod metadata;
pub use metadata::{safe_metadata, safe_request};

mod task_snapshots;
pub use task_snapshots::{bind, bind_current, dataset_snapshot, reconcile_task};
