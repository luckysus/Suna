use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub material_system: Option<String>,
    pub status: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResearchProject {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub material_system: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub expected_updated_at: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchObject {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub relation: String,
    pub availability: String,
    pub created_at: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSummary {
    pub counts: std::collections::BTreeMap<String, i64>,
    pub missing_sources: i64,
}

fn error(e: rusqlite::Error) -> String {
    e.to_string()
}
fn row(row: &rusqlite::Row) -> rusqlite::Result<ResearchProject> {
    let tags: String = row.get(5)?;
    Ok(ResearchProject {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        material_system: row.get(3)?,
        status: row.get(4)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}
const SELECT:&str="SELECT id,name,description,material_system,status,tags_json,created_at,updated_at FROM research_projects";
pub fn get(conn: &Connection, ws: &str, id: &str) -> Result<ResearchProject, String> {
    conn.query_row(
        &format!("{SELECT} WHERE workspace_id=?1 AND id=?2"),
        params![ws, id],
        row,
    )
    .optional()
    .map_err(error)?
    .ok_or_else(|| "research_project_not_found".into())
}
pub fn require_active(conn: &Connection, ws: &str, id: &str) -> Result<ResearchProject, String> {
    let p = get(conn, ws, id)?;
    if p.status != "active" {
        return Err("research_project_archived".into());
    }
    Ok(p)
}
pub fn list(
    conn: &Connection,
    ws: &str,
    search: &str,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ResearchProject>, String> {
    if !matches!(status, None | Some("active") | Some("archived")) {
        return Err("invalid project status".into());
    }
    let mut q=conn.prepare(&format!("{SELECT} WHERE workspace_id=?1 AND (?2 IS NULL OR status=?2) AND (name LIKE '%'||?3||'%' OR description LIKE '%'||?3||'%' OR tags_json LIKE '%'||?3||'%') ORDER BY updated_at DESC,id LIMIT ?4 OFFSET ?5")).map_err(error)?;
    let result = q
        .query_map(
            params![ws, status, search, limit.clamp(1, 100), offset],
            row,
        )
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error);
    result
}
pub fn audit(
    conn: &Connection,
    ws: &str,
    id: &str,
    action: &str,
    metadata: Value,
) -> Result<(), String> {
    conn.execute("INSERT INTO research_audit_events(workspace_id,id,project_id,action,metadata_json,actor,created_at) VALUES(?1,?2,?3,?4,?5,'local_user',?6)",params![ws,Uuid::new_v4().to_string(),id,action,metadata.to_string(),Utc::now().to_rfc3339()]).map_err(error)?;
    Ok(())
}
pub fn save(
    conn: &mut Connection,
    ws: &str,
    input: SaveResearchProject,
) -> Result<ResearchProject, String> {
    let name = input.name.trim();
    if name.is_empty()
        || name.chars().count() > 160
        || input.description.chars().count() > 10000
        || input.tags.len() > 32
        || input
            .tags
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().count() > 64)
        || input
            .material_system
            .as_ref()
            .is_some_and(|s| s.chars().count() > 120)
    {
        return Err("invalid project name, description or tags".into());
    }
    let tx = conn.transaction().map_err(error)?;
    let now = Utc::now().to_rfc3339();
    let id = if let Some(id) = input.id {
        let existing = require_active(&tx, ws, &id)?;
        if input
            .expected_updated_at
            .is_some_and(|expected| expected != existing.updated_at)
        {
            return Err("project_changed: reload before editing".into());
        }
        tx.execute("UPDATE research_projects SET name=?3,description=?4,material_system=?5,tags_json=?6,updated_at=?7 WHERE workspace_id=?1 AND id=?2",params![ws,id,name,input.description,input.material_system,serde_json::to_string(&input.tags).map_err(|e|e.to_string())?,now]).map_err(error)?;
        audit(
            &tx,
            ws,
            &id,
            "project_updated",
            json!({"fields":["name","description","materialSystem","tags"]}),
        )?;
        id
    } else {
        let id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO research_projects(workspace_id,id,name,description,material_system,status,tags_json,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,'active',?6,?7,?7)",params![ws,id,name,input.description,input.material_system,serde_json::to_string(&input.tags).map_err(|e|e.to_string())?,now]).map_err(error)?;
        audit(&tx, ws, &id, "project_created", json!({}))?;
        id
    };
    tx.commit().map_err(error)?;
    get(conn, ws, &id)
}
pub fn archive(
    conn: &mut Connection,
    ws: &str,
    id: &str,
    archived: bool,
) -> Result<ResearchProject, String> {
    let tx = conn.transaction().map_err(error)?;
    get(&tx, ws, id)?;
    tx.execute(
        "UPDATE research_projects SET status=?3,updated_at=?4 WHERE workspace_id=?1 AND id=?2",
        params![
            ws,
            id,
            if archived { "archived" } else { "active" },
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(error)?;
    audit(
        &tx,
        ws,
        id,
        if archived {
            "project_archived"
        } else {
            "project_restored"
        },
        json!({}),
    )?;
    tx.commit().map_err(error)?;
    get(conn, ws, id)
}
// These SQL identifiers are a closed whitelist, never supplied by the caller.
pub fn target(kind: &str) -> Result<(&'static str, &'static str), String> {
    Ok(match kind {
        "dataset" => ("steel_datasets", "source_name"),
        "model" => ("steel_models", "kind || ' v' || version"),
        "background_task" => ("background_tasks", "kind || ' · ' || state"),
        "conversation" => ("conversations", "title"),
        "knowledge_base" => ("knowledge_bases", "name"),
        "document" => ("knowledge_source_documents", "display_name"),
        "agent_run" => ("agent_runs", "id"),
        "optimization" => ("optimization_tasks", "method || ' · ' || state"),
        "experiment" => ("experiments", "title"),
        "report" => ("reports", "title"),
        "artifact" => ("research_artifacts", "title"),
        _ => return Err("unsupported research object kind".into()),
    })
}
pub fn target_title(
    conn: &Connection,
    ws: &str,
    kind: &str,
    id: &str,
) -> Result<Option<String>, String> {
    let (table, title) = target(kind)?;
    conn.query_row(
        &format!("SELECT COALESCE({title},id) FROM {table} WHERE workspace_id=?1 AND id=?2"),
        params![ws, id],
        |r| r.get(0),
    )
    .optional()
    .map_err(error)
}
pub fn attach_in_transaction(
    conn: &Connection,
    ws: &str,
    project: &str,
    kind: &str,
    id: &str,
    relation: &str,
) -> Result<(), String> {
    require_active(conn, ws, project)?;
    if !["attached", "generated_by", "derived_from", "referenced_by"].contains(&relation) {
        return Err("invalid research relation".into());
    }
    target_title(conn, ws, kind, id)?.ok_or("research_source_not_found_in_workspace")?;
    let changed=conn.execute("INSERT OR IGNORE INTO research_project_links(workspace_id,project_id,target_kind,target_id,relation,created_at,created_by) VALUES(?1,?2,?3,?4,?5,?6,'local_user')",params![ws,project,kind,id,relation,Utc::now().to_rfc3339()]).map_err(error)?;
    if kind=="background_task" && super::research_artifacts::installed(conn){
        super::research_artifacts::bind(conn,ws,project,id)?;
        super::research_artifacts::reconcile_task(conn,ws,id)?;
    }
    if changed > 0 {
        audit(
            conn,
            ws,
            project,
            "object_attached",
            json!({"kind":kind,"id":id,"relation":relation}),
        )?;
    }
    Ok(())
}
pub fn attach(
    conn: &mut Connection,
    ws: &str,
    project: &str,
    kind: &str,
    id: &str,
    relation: &str,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(error)?;
    attach_in_transaction(&tx, ws, project, kind, id, relation)?;
    tx.commit().map_err(error)
}
pub fn detach(
    conn: &mut Connection,
    ws: &str,
    project: &str,
    kind: &str,
    id: &str,
    relation: &str,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(error)?;
    require_active(&tx, ws, project)?;
    let count=tx.execute("DELETE FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 AND target_kind=?3 AND target_id=?4 AND relation=?5",params![ws,project,kind,id,relation]).map_err(error)?;
    if count > 0 {
        audit(
            &tx,
            ws,
            project,
            "object_detached",
            json!({"kind":kind,"id":id,"relation":relation}),
        )?;
    }
    tx.commit().map_err(error)
}
pub fn objects(
    conn: &Connection,
    ws: &str,
    project: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<ResearchObject>, String> {
    get(conn, ws, project)?;
    let mut q=conn.prepare("SELECT target_kind,target_id,relation,created_at FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at DESC,target_kind,target_id LIMIT ?3 OFFSET ?4").map_err(error)?;
    let rows = q
        .query_map(params![ws, project, limit.clamp(1, 100), offset], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    rows.into_iter()
        .map(|(kind, id, relation, created_at)| {
            let title = target_title(conn, ws, &kind, &id)?;
            Ok(ResearchObject {
                kind,
                id: id.clone(),
                title: title.clone().unwrap_or(id),
                relation,
                availability: if title.is_some() {
                    "available"
                } else {
                    "missing"
                }
                .into(),
                created_at,
            })
        })
        .collect()
}
pub fn available(
    conn: &Connection,
    ws: &str,
    kind: &str,
    search: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<ResearchObject>, String> {
    let (table, title) = target(kind)?;
    let mut q=conn.prepare(&format!("SELECT id,COALESCE({title},id) FROM {table} WHERE workspace_id=?1 AND COALESCE({title},id) LIKE '%'||?2||'%' ORDER BY id LIMIT ?3 OFFSET ?4")).map_err(error)?;
    let result = q
        .query_map(params![ws, search, limit.clamp(1, 100), offset], |r| {
            Ok(ResearchObject {
                kind: kind.into(),
                id: r.get(0)?,
                title: r.get(1)?,
                relation: "attached".into(),
                availability: "available".into(),
                created_at: String::new(),
            })
        })
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error);
    result
}
pub fn summary(conn: &Connection, ws: &str, id: &str) -> Result<ResearchSummary, String> {
    get(conn, ws, id)?;
    let mut q=conn.prepare("SELECT target_kind,COUNT(DISTINCT target_id) FROM research_project_links WHERE workspace_id=?1 AND project_id=?2 GROUP BY target_kind").map_err(error)?;
    let counts = q
        .query_map(params![ws, id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(error)?
        .collect::<Result<std::collections::BTreeMap<_, _>, _>>()
        .map_err(error)?;
    let mut missing = 0;
    for kind in counts.keys() {
        let (table, _) = target(kind)?;
        missing+=conn.query_row(&format!("SELECT COUNT(DISTINCT l.target_id) FROM research_project_links l LEFT JOIN {table} t ON t.workspace_id=l.workspace_id AND t.id=l.target_id WHERE l.workspace_id=?1 AND l.project_id=?2 AND l.target_kind=?3 AND t.id IS NULL"),params![ws,id,kind],|r|r.get::<_,i64>(0)).map_err(error)?;
    }
    Ok(ResearchSummary {
        counts,
        missing_sources: missing,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::migrations::migrate;
    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        c
    }
    pub fn project(c: &mut Connection, ws: &str) -> ResearchProject {
        save(
            c,
            ws,
            SaveResearchProject {
                id: None,
                name: "耐磨钢课题".into(),
                description: "联合优化".into(),
                material_system: Some("steel".into()),
                tags: vec!["磨损".into()],
                expected_updated_at: None,
            },
        )
        .unwrap()
    }
    fn sources(c: &Connection) {
        c.execute_batch("INSERT INTO steel_datasets VALUES('ws','data','合成数据.csv','/fixture','sha','csv','',2,2,0,'ready','{}','now','now'); INSERT INTO steel_models(workspace_id,id,lineage_id,kind,version,model_sha256,manifest_json,artifact_json,is_active,created_at) VALUES('ws','model','lineage','linear_artifact',1,'sha','{}','{}',1,'now'); INSERT INTO background_tasks(id,workspace_id,kind,state,payload_json,created_at,updated_at) VALUES('task','ws','compute.train','queued','{}','now','now');").unwrap();
    }
    #[test]
    fn links_are_idempotent_scoped_and_non_destructive() {
        let mut c = db();
        sources(&c);
        let p = project(&mut c, "ws");
        for (k, id) in [
            ("dataset", "data"),
            ("model", "model"),
            ("background_task", "task"),
        ] {
            attach(&mut c, "ws", &p.id, k, id, "attached").unwrap();
            attach(&mut c, "ws", &p.id, k, id, "attached").unwrap();
        }
        assert_eq!(objects(&c, "ws", &p.id, 50, 0).unwrap().iter().map(|o|(&o.kind,&o.id)).collect::<std::collections::HashSet<_>>().len(), 3);
        assert!(attach(&mut c, "other", &p.id, "dataset", "data", "attached").is_err());
        let other = project(&mut c, "other");
        assert!(attach(&mut c, "other", &other.id, "dataset", "data", "attached").is_err());
        assert!(get(&c, "ws", "missing").is_err());
        assert!(attach(&mut c, "ws", &p.id, "dataset", "missing", "attached").is_err());
        detach(&mut c, "ws", &p.id, "dataset", "data", "attached").unwrap();
        assert!(target_title(&c, "ws", "dataset", "data").unwrap().is_some());
        archive(&mut c, "ws", &p.id, true).unwrap();
        assert!(attach(&mut c, "ws", &p.id, "dataset", "data", "attached").is_err());
        assert!(target_title(&c, "ws", "model", "model").unwrap().is_some());
        c.execute("DELETE FROM background_tasks WHERE id='task'", [])
            .unwrap();
        assert_eq!(summary(&c, "ws", &p.id).unwrap().missing_sources, 1);
        assert!(c.execute("DELETE FROM research_audit_events", []).is_err());
    }
    #[test]
    fn survives_reopen_and_conflicting_edit() {
        let directory = std::env::temp_dir().join(format!("suna-project-test-{}",Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("研究.sqlite");
        let id = {
            let mut c = Connection::open(&path).unwrap();
            migrate(&mut c).unwrap();
            let p = project(&mut c, "ws");
            assert!(save(
                &mut c,
                "ws",
                SaveResearchProject {
                    id: Some(p.id.clone()),
                    name: "覆盖".into(),
                    description: "".into(),
                    material_system: None,
                    tags: vec![],
                    expected_updated_at: Some("stale".into())
                }
            )
            .is_err());
            p.id
        };
        let mut c = Connection::open(&path).unwrap();
        assert!(migrate(&mut c).unwrap().applied_versions.is_empty());
        assert_eq!(get(&c, "ws", &id).unwrap().name, "耐磨钢课题");
        drop(c); std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn upgrades_v35_preserving_user_configuration() {
        let mut c = Connection::open_in_memory().unwrap();
        // Execute actual pre-project migrations, including their production migration bookkeeping.
        for m in crate::storage::migrations::MIGRATIONS
            .iter()
            .filter(|m| m.version <= 35)
        {
            let tx = c.transaction().unwrap();
            tx.execute_batch(m.sql).unwrap();
            tx.pragma_update(None, "user_version", m.version).unwrap();
            tx.commit().unwrap();
        }
        c.execute("INSERT INTO settings(workspace_id,key,value_json,updated_at) VALUES('ws','user-theme','\"dark\"','now')",[]).unwrap();
        let report = migrate(&mut c).unwrap();
        assert_eq!(report.applied_versions, (36..=crate::storage::migrations::latest_version()).collect::<Vec<_>>());
        assert_eq!(
            c.query_row(
                "SELECT value_json FROM settings WHERE key='user-theme'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "\"dark\""
        );
        project(&mut c, "ws");
    }
}
