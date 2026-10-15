use super::*;

fn kind(task: &str) -> Option<(&str, &str)> {
    if task.starts_with("compute_train_") {
        Some(("training", "model_snapshot"))
    } else if task.starts_with("compute_predict_") {
        Some(("prediction", "prediction_result"))
    } else if task == "compute_optimize_constrained" {
        Some(("optimization", "optimization_result"))
    } else if task == "database_query" {
        Some(("analysis", "analysis_result"))
    } else if task == "literature_metadata_search" {
        Some(("literature", "analysis_result"))
    } else if task == "compute_analyze_microstructure" {
        Some(("analysis", "analysis_result"))
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
        let raw: String = c
            .query_row(
                "SELECT payload_json FROM background_tasks WHERE workspace_id=?1 AND id=?2",
                params![ws, task],
                |r| r.get(0),
            )
            .map_err(err)?;
        let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

        let explicit = payload["_research_project_id"].as_str().map(str::to_string);
        if let Some(p) = match explicit {
            Some(p) => Some(p),
            None => match crate::storage::repositories::research_delivery::scope(c)? {
                Some(p) => p,
                None => current(c, ws)?,
            },
        } {
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
    let Some(d) = crate::storage::repositories::steel::get(c, ws, id)? else {
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
        if cp.get("result").is_none() && task_kind != "database_query" {
            return Err("missing_compute_result".into());
        }
        let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if task_kind == "compute_analyze_microstructure" {
            crate::storage::repositories::microstructure::record_completed(
                c,
                ws,
                task,
                &p,
                &run,
                &cp["result"],
            )?;
            continue;
        }
        if task_kind == "database_query" {
            let result = crate::storage::repositories::database_query_results::get(
                c,
                ws,
                Uuid::parse_str(task).map_err(|e| e.to_string())?,
            )?
            .ok_or("missing_query_result")?;
            create(
                c,
                ws,
                &p,
                "analysis_result",
                "生产数据只读查询",
                task,
                Some(&run),
                json!({"connectionRef":result.connection_id,"queryTaskId":task,"queryTemplateSha256":format!("{:x}",Sha256::digest(result.query_text.as_bytes())),"parameterSha256":format!("{:x}",Sha256::digest(payload["parameters"].to_string().as_bytes())),"queryTime":result.created_at,"columns":result.columns,"rowCount":result.row_count,"truncated":result.truncated}),
            )?;
            continue;
        }
        if task_kind == "literature_metadata_search" {
            // Old frozen results retain their exact bytes and historical digest,
            // including snapshots created before multiple providers were supported.
            let existing: Option<String> = c.query_row(
                "SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind='analysis_result' AND source_key=?3",
                params![ws, p, task], |r| r.get(0),
            ).optional().map_err(err)?;
            if let Some(id) = existing {
                get(c, ws, &id)?;
                continue;
            }
            let frozen = super::literature_snapshot::payload(c, ws, &payload, &cp["result"])?;
            create(
                c,
                ws,
                &p,
                "analysis_result",
                "公开元数据检索清单",
                task,
                Some(&run),
                frozen,
            )?;
            continue;
        }
        let mut frozen = json!({"request":safe_request(&payload["payload"]),"result":safe_metadata(&cp["result"]),"producer":{"app_version":env!("CARGO_PKG_VERSION"),"worker_protocol":"1.0"}});
        if artifact_kind == "model_snapshot" {
            if let Some(model) = cp["result"].get("artifact") {
                frozen["result"]["model_sha256"] = json!(format!(
                    "{:x}",
                    Sha256::digest(model.to_string().as_bytes())
                ));
            }
        }
        if let Some(raw) = payload["payload"].get("material_rule_snapshot") {
            let snapshot: crate::storage::repositories::material_rules::RuleSnapshot =
                serde_json::from_value(raw.clone()).map_err(|e| e.to_string())?;
            frozen["request"]["material_rule_snapshot"] = json!(snapshot);
            frozen["result"]["material_rule_snapshot"] = raw.clone();
        }
        let existing:Option<String>=c.query_row("SELECT id FROM research_artifacts WHERE workspace_id=?1 AND project_id=?2 AND kind=?3 AND source_key=?4",params![ws,p,artifact_kind,task],|r|r.get(0)).optional().map_err(err)?;
        let output = if let Some(id) = existing {
            get(c, ws, &id)?;
            id
        } else {
            create(
                c,
                ws,
                &p,
                artifact_kind,
                &task_kind,
                task,
                Some(&run),
                frozen,
            )?
        };
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
