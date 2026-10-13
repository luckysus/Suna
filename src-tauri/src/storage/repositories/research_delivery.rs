//! Project exports contain verified metadata and IDs, never model blobs or raw paths.
use super::{reports, research_artifacts as artifacts, research_projects as projects};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
fn error(e: rusqlite::Error) -> String {
    e.to_string()
}
pub fn context(c: &Connection, ws: &str, run: &str) -> Result<Option<String>, String> {
    c.query_row(
        "SELECT project_id FROM research_agent_contexts WHERE workspace_id=?1 AND run_id=?2",
        params![ws, run],
        |r| r.get::<_, Option<String>>(0),
    )
    .optional()
    .map_err(error)
    .map(Option::flatten)
}
/// A connection-local override permits nested Agent submission transactions and
/// records explicit null scope without modifying the user's current project.
pub fn scoped<T>(
    c: &Connection,
    p: Option<&str>,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    c.execute_batch("CREATE TEMP TABLE IF NOT EXISTS research_submission_scope(id INTEGER PRIMARY KEY,project_id TEXT)").map_err(error)?;
    let previous: Option<Option<String>> = c
        .query_row(
            "SELECT project_id FROM temp.research_submission_scope WHERE id=1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(error)?;
    c.execute(
        "INSERT OR REPLACE INTO temp.research_submission_scope VALUES(1,?1)",
        [p],
    )
    .map_err(error)?;
    let result = f();
    c.execute("DELETE FROM temp.research_submission_scope", [])
        .map_err(error)?;
    if let Some(old) = previous {
        c.execute(
            "INSERT INTO temp.research_submission_scope VALUES(1,?1)",
            [old],
        )
        .map_err(error)?;
    }
    result
}
pub fn scope(c: &Connection) -> Result<Option<Option<String>>, String> {
    let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_temp_master WHERE name='research_submission_scope')",[],|r|r.get(0)).map_err(error)?;
    if !exists {
        return Ok(None);
    }
    c.query_row(
        "SELECT project_id FROM temp.research_submission_scope WHERE id=1",
        [],
        |r| r.get(0),
    )
    .optional()
    .map_err(error)
}
pub fn agent_call(
    c: &mut Connection,
    ws: &str,
    run: Option<&str>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let p = if let Some(run) = run {
        context(c, ws, run)?
    } else {
        artifacts::current(c, ws)?
    };
    match name {
        "research_list_projects" => Ok(json!(projects::list(
            c,
            ws,
            args["search"].as_str().unwrap_or(""),
            None,
            25,
            args["offset"].as_u64().unwrap_or(0) as u32
        )?)),
        "research_get_project_context" => {
            let id = p.ok_or("research_project_unassigned")?;
            Ok(json!({"project":projects::get(c,ws,&id)?,"summary":projects::summary(c,ws,&id)?}))
        }
        "research_list_artifacts" => {
            let id = p.ok_or("research_project_unassigned")?;
            Ok(json!(artifacts::list(
                c,
                ws,
                &id,
                25,
                args["offset"].as_u64().unwrap_or(0) as u32
            )?))
        }
        "research_attach_result" => {
            let id = p.ok_or("research_project_unassigned")?;
            let kind = args["kind"].as_str().ok_or("kind required")?;
            let source = args["id"].as_str().ok_or("id required")?;
            if !matches!(
                kind,
                "background_task" | "model" | "dataset" | "report" | "experiment" | "document"
            ) {
                return Err("unsupported_research_attachment".into());
            }
            projects::attach(c, ws, &id, kind, source, "attached")?;
            Ok(json!({"projectId":id,"attached":true}))
        }
        _ => Err("unknown_research_tool".into()),
    }
}
pub fn manifest(c: &Connection, ws: &str, p: &str) -> Result<Value, String> {
    let project = projects::get(c, ws, p)?;
    let mut objects = vec![];
    let mut all = vec![];
    let mut edges = vec![];
    for offset in (0..10000).step_by(100) {
        let page = projects::objects(c, ws, p, 100, offset)?;
        let done = page.len() < 100;
        objects.extend(page);
        if done {
            break;
        }
        if offset == 9900 {
            return Err("manifest_object_limit".into());
        }
    }
    for offset in (0..10000).step_by(100) {
        let page = artifacts::list(c, ws, p, 100, offset)?;
        let done = page.len() < 100;
        for a in page {
            all.push(artifacts::get(c, ws, &a.id)?);
        }
        if done {
            break;
        }
        if offset == 9900 {
            return Err("manifest_artifact_limit".into());
        }
    }
    for offset in (0..10000).step_by(100) {
        let page = artifacts::edges(c, ws, p, 100, offset)?;
        let done = page.len() < 100;
        edges.extend(page);
        if done {
            break;
        }
        if offset == 9900 {
            return Err("manifest_lineage_limit".into());
        }
    }
    let warnings: Vec<String> = objects
        .iter()
        .filter(|o| o.availability != "available")
        .map(|o| format!("missing:{}:{}", o.kind, o.id))
        .collect();
    Ok(
        json!({"schemaVersion":"research-manifest.v1","appVersion":env!("CARGO_PKG_VERSION"),"exportedAt":Utc::now().to_rfc3339(),"project":project,"objects":objects,"artifacts":all,"edges":edges,"warnings":warnings,"reproductionRequirements":["original source files matching source_sha256","original model versions matching model_sha256","recorded worker environment and split/rule parameters"],"containsRawData":false}),
    )
}
pub fn report(c: &mut Connection, ws: &str, p: &str) -> Result<reports::ReportRecord, String> {
    projects::require_active(c, ws, p)?;
    let m = manifest(c, ws, p)?;
    let project = projects::get(c, ws, p)?;
    let mut text=format!("# {}：科研项目成果报告\n\n{}\n\n以下内容来自已存储成果与审核记录；没有原文证据的内容不作为文献结论。\n\n",project.name,project.description);
    let items = m["artifacts"].as_array().ok_or("manifest_invalid")?;
    for a in items {
        let kind = a["kind"].as_str().unwrap_or("");
        if kind == "report" {
            continue;
        }
        text += &format!(
            "## {} ({})\n\n成果 ID：{}；SHA256：{}\n\n",
            a["title"].as_str().unwrap_or("成果"),
            kind,
            a["id"].as_str().unwrap_or(""),
            a["contentSha256"].as_str().unwrap_or("")
        );
        text += &format!(
            "```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&a["payload"]).map_err(|e| e.to_string())?
        );
    }
    text+="## 未验证事项\n\n候选预测不等同于实测；GP 标准差不构成已验证覆盖率的预测区间。未审核数据不进入训练。Windows 原生/设备与外部 Provider 验收需另行完成。\n\n";
    if let Some(warnings) = m["warnings"].as_array() {
        for w in warnings {
            text += &format!("- {}\n", w.as_str().unwrap_or("来源不可用"));
        }
    }
    let metadata=json!({"projectId":p,"artifactIds":items.iter().filter_map(|a|a["id"].as_str()).collect::<Vec<_>>(),"manifestSha256":format!("{:x}",Sha256::digest(m.to_string().as_bytes()))}).to_string();
    artifacts::atomic(c, || {
        let created = reports::create_in_transaction(
            c,
            ws,
            reports::NewReport {
                kind: "项目成果报告",
                title: &format!("{}成果报告", project.name),
                topic: &project.name,
                format: "markdown",
                content_markdown: &text,
                dataset_id: None,
                knowledge_base_ids_json: "[]",
                metadata_json: &metadata,
            },
        )?;
        projects::attach_in_transaction(c, ws, p, "report", &created.id, "generated_by")?;
        let id = artifacts::create(
            c,
            ws,
            p,
            "report",
            &created.title,
            &created.id,
            None,
            json!({"report_id":created.id,"content_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),"source_artifacts":items.iter().filter_map(|a|a["id"].as_str()).collect::<Vec<_>>() }),
        )?;
        for a in items.iter().filter(|a| a["kind"] != "report") {
            artifacts::edge(
                c,
                ws,
                a["id"].as_str().ok_or("artifact_id_missing")?,
                &id,
                "included_in_report",
            )?;
        }
        Ok(created)
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
                name: "合成报告课题".into(),
                description: "验证成果来源".into(),
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
    fn run(c: &Connection, id: &str) {
        c.execute("INSERT OR IGNORE INTO conversations(id,workspace_id,title,created_at,updated_at) VALUES('chat','ws','测试','now','now')",[]).unwrap();
        c.execute("INSERT OR IGNORE INTO messages(id,workspace_id,conversation_id,role,content,created_at) VALUES('msg','ws','chat','user','测试','now')",[]).unwrap();
        c.execute("INSERT INTO agent_runs(id,workspace_id,conversation_id,user_message_id,state,created_at,updated_at) VALUES(?1,'ws','chat','msg','created','now','now')",[id]).unwrap();
    }
    #[test]
    fn agent_context_is_frozen_scoped_and_survives_project_switch() {
        let (c, p) = fixture();
        run(&c, "run1");
        c.execute(
            "UPDATE settings SET value_json='null' WHERE key='suna.research.current_project'",
            [],
        )
        .unwrap();
        run(&c, "run2");
        assert_eq!(context(&c, "ws", "run1").unwrap(), Some(p.clone()));
        assert_eq!(context(&c, "other", "run1").unwrap(), None);
        assert_eq!(context(&c, "ws", "run2").unwrap(), None);
        assert!(c
            .execute("UPDATE research_agent_contexts SET project_id=NULL", [])
            .is_err());
        assert_eq!(projects::objects(&c, "ws", &p, 25, 0).unwrap().len(), 2);
        scoped(&c, Some(&p), || {
            assert_eq!(artifacts::current(&c, "ws")?, Some(p.clone()));
            let e: Result<(), String> = scoped(&c, None, || {
                assert_eq!(artifacts::current(&c, "ws")?, None);
                Err("failure".into())
            });
            assert!(e.is_err());
            assert_eq!(artifacts::current(&c, "ws")?, Some(p.clone()));
            Ok(())
        })
        .unwrap();
        assert_eq!(artifacts::current(&c, "ws").unwrap(), None);
    }
    #[test]
    fn report_is_persisted_with_verified_sources_and_export_contains_no_raw_payload() {
        let (mut c, p) = fixture();
        let task=crate::tasks::repository::create(&c,crate::tasks::NewTask{workspace_id:"ws".into(),kind:"compute_train_linear_regression".into(),payload_json:json!({"operation":"test","payload":{"features":[[99.]],"api_key":"sentinel-secret","source_path":"/sentinel/private"}}).to_string(),checkpoint_json:None,next_run_at:None,progress:0}).unwrap();
        c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",params![task.id.to_string(),json!({"result":{"artifact":{"model_base64":"sentinel-blob","feature_names":["C"]},"metrics":{"rmse":1.}}}).to_string()]).unwrap();
        artifacts::reconcile_task(&c, "ws", &task.id.to_string()).unwrap();
        let m = manifest(&c, "ws", &p).unwrap();
        let raw = m.to_string();
        for s in [
            "sentinel-secret",
            "sentinel-blob",
            "/sentinel/private",
            "[[99",
        ] {
            assert!(!raw.contains(s))
        }
        assert_eq!(m["containsRawData"], false);
        assert!(manifest(&c, "other", &p).is_err());
        let r = report(&mut c, "ws", &p).unwrap();
        assert!(r.content_markdown.contains("feature_names"));
        assert_eq!(reports::get(&c, "ws", &r.id).unwrap().unwrap(), r);
        assert_eq!(artifacts::edges(&c, "ws", &p, 25, 0).unwrap().len(), 1);
        assert_eq!(
            manifest(&c, "ws", &p).unwrap()["artifacts"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn report_and_links_roll_back_together() {
        let (mut c, p) = fixture();
        c.execute_batch("CREATE TRIGGER reject_test_artifact BEFORE INSERT ON research_artifacts BEGIN SELECT RAISE(ABORT,'simulated_disk_failure'); END").unwrap();
        assert!(report(&mut c, "ws", &p).is_err());
        assert!(reports::list(&c, "ws").unwrap().is_empty());
        assert!(projects::objects(&c, "ws", &p, 25, 0).unwrap().is_empty());
    }
    #[test]
    fn attachments_cannot_guess_cross_workspace_objects_or_change_project_context() {
        let (mut c, p) = fixture();
        run(&c, "run");
        let r = reports::create(
            &mut c,
            "other",
            reports::NewReport {
                kind: "测试",
                title: "他区报告",
                topic: "",
                format: "markdown",
                content_markdown: "",
                dataset_id: None,
                knowledge_base_ids_json: "[]",
                metadata_json: "{}",
            },
        )
        .unwrap();
        assert!(agent_call(
            &mut c,
            "ws",
            Some("run"),
            "research_attach_result",
            json!({"kind":"report","id":r.id})
        )
        .is_err());
        let ctx = agent_call(
            &mut c,
            "ws",
            Some("run"),
            "research_get_project_context",
            json!({"projectId":"other"}),
        )
        .unwrap();
        assert_eq!(ctx["project"]["id"], p);
        projects::archive(&mut c, "ws", &p, true).unwrap();
        assert!(report(&mut c, "ws", &p).is_err());
    }
    #[test]
    fn agent_and_project_views_reference_the_same_task_after_switch() {
        let (mut c, p) = fixture();
        let id = uuid::Uuid::new_v4();
        run(&c, &id.to_string());
        c.execute(
            "UPDATE agent_runs SET state='executing_tools' WHERE id=?1",
            [id.to_string()],
        )
        .unwrap();
        c.execute(
            "UPDATE settings SET value_json='null' WHERE key='suna.research.current_project'",
            [],
        )
        .unwrap();
        let frozen = context(&c, "ws", &id.to_string()).unwrap();
        let source = crate::tasks::sources::AgentTaskSource {
            workspace_id: "ws".into(),
            run_id: id,
            tool_call_id: uuid::Uuid::new_v4(),
        };
        let task = crate::tasks::sources::submit_with(&mut c, source.clone(), |c| {
            scoped(c, frozen.as_deref(), || {
                crate::tasks::repository::create(
                    c,
                    crate::tasks::NewTask {
                        workspace_id: "ws".into(),
                        kind: "compute_optimize_constrained".into(),
                        payload_json:
                            json!({"operation":"optimize_constrained","payload":{"seed":42}})
                                .to_string(),
                        checkpoint_json: None,
                        next_run_at: None,
                        progress: 0,
                    },
                )
                .map_err(|e| e.to_string())
            })
        })
        .unwrap();
        let project_view = artifacts::runs(&c, "ws", &p, 25, 0).unwrap();
        assert_eq!(project_view.len(), 1);
        assert_eq!(
            project_view[0].background_task_id,
            Some(task.id.to_string())
        );
        assert_eq!(project_view[0].agent_run_id, Some(id.to_string()));
        assert_eq!(project_view[0].origin, "agent");
        let replay = crate::tasks::sources::submit_with(&mut c, source, |_| {
            panic!("must reuse original task")
        })
        .unwrap();
        assert_eq!(replay.id, task.id);
        assert_eq!(artifacts::current(&c, "ws").unwrap(), None);
    }
}
