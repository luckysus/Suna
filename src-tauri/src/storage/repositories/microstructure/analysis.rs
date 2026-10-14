use super::*;

pub fn submit(
    c: &Connection,
    ws: &str,
    root: &Path,
    p: &str,
    ids: Vec<String>,
) -> Result<crate::tasks::TaskRecord, String> {
    projects::require_active(c, ws, p)?;
    if ids.is_empty()
        || ids.len() > 20
        || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
    {
        return Err("invalid_image_batch".into());
    }
    let mut inputs = vec![];
    let mut sources = vec![];
    let mut seen_images = std::collections::HashSet::new();
    for id in ids {
        let (image_id,body,sha):(String,String,String)=c.query_row("SELECT image_id,parameters_json,content_sha256 FROM microstructure_annotations WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?.ok_or("image_annotation_not_found")?;
        if hash(body.as_bytes()) != sha {
            return Err("image_annotation_hash_mismatch".into());
        }
        if !seen_images.insert(image_id.clone()) {
            return Err("duplicate_image_in_batch".into());
        }
        let image = get(c, ws, &image_id)?;
        if image.project_id != p {
            return Err("image_outside_project".into());
        }
        let(raw,source):(String,String)=c.query_row("SELECT storage_ref,source_artifact_id FROM microstructure_images WHERE workspace_id=?1 AND id=?2",params![ws,image_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
        let path = managed(root, &raw)?;
        inputs.push(json!({"imageId":image_id,"annotationId":id,"annotation":serde_json::from_str::<Value>(&body).map_err(|e|e.to_string())?,"path":path,"sha256":image.source_sha256,"rasterInput":image.metadata.get("rasterInput")}));
        sources.push(source);
    }
    artifacts::atomic(c, || {
        let task=crate::tasks::repository::create(c,crate::tasks::NewTask{workspace_id:ws.into(),kind:"compute_analyze_microstructure".into(),payload_json:json!({"_research_project_id":p,"operation":"analyze_microstructure","payload":{"images":inputs}}).to_string(),checkpoint_json:None,next_run_at:None,progress:0}).map_err(|e|e.to_string())?;
        let run:String=c.query_row("SELECT id FROM research_runs WHERE workspace_id=?1 AND project_id=?2 AND background_task_id=?3",params![ws,p,task.id.to_string()],|r|r.get(0)).map_err(err)?;
        for source in sources {
            c.execute(
                "INSERT OR IGNORE INTO research_run_sources VALUES(?1,?2,?3,'based_on_evidence')",
                params![ws, run, source],
            )
            .map_err(err)?;
        }
        Ok(task)
    })
}
pub fn record_completed(
    c: &Connection,
    ws: &str,
    task: &str,
    p: &str,
    run: &str,
    result: &Value,
) -> Result<(), String> {
    let rows = result["images"]
        .as_array()
        .ok_or("invalid_image_analysis_result")?;
    if rows.len() > 20 {
        return Err("invalid_image_analysis_result".into());
    }
    let raw: String = c
        .query_row(
            "SELECT payload_json FROM background_tasks WHERE workspace_id=?1 AND id=?2",
            params![ws, task],
            |r| r.get(0),
        )
        .map_err(err)?;
    let request: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let mut expected: std::collections::HashSet<(String, String)> = request["payload"]["images"]
        .as_array()
        .ok_or("invalid_image_task")?
        .iter()
        .map(|r| {
            Ok((
                r["imageId"].as_str().ok_or("invalid_image_task")?.into(),
                r["annotationId"]
                    .as_str()
                    .ok_or("invalid_image_task")?
                    .into(),
            ))
        })
        .collect::<Result<_, String>>()?;
    if rows.len() != expected.len() {
        return Err("image_analysis_count_mismatch".into());
    }
    for row in rows {
        let id = row["imageId"].as_str().ok_or("missing_image_id")?;
        let image = get(c, ws, id)?;
        if image.project_id != p {
            return Err("image_outside_project".into());
        }
        let aid = row["annotationId"]
            .as_str()
            .ok_or("missing_annotation_id")?;
        if !expected.remove(&(id.to_string(), aid.to_string())) {
            return Err("image_result_outside_submission".into());
        }
        let (annotation_image,annotation_body):(String,String)=c.query_row("SELECT image_id,parameters_json FROM microstructure_annotations WHERE workspace_id=?1 AND id=?2",params![ws,aid],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
        if annotation_image != id {
            return Err("image_annotation_mismatch".into());
        }
        let output = &row["result"];
        let error = row["error"].as_str();
        if error.is_none() && output["sourceSha256"] != image.source_sha256 {
            return Err("image_source_hash_mismatch".into());
        }
        if error.is_none()
            && image.metadata.get("rasterInput").is_some_and(|input| {
                !input.is_null()
                    && (output.get("rasterInput") != Some(input)
                        || output["width"] != image.width
                        || output["height"] != image.height)
            })
        {
            return Err("image_input_contract_mismatch".into());
        }
        let key = format!("image-analysis:{task}:{id}");
        let a = if error.is_none() {
            Some(artifacts::create(
                c,
                ws,
                p,
                "analysis_result",
                "显微组织阈值统计",
                &key,
                Some(run),
                json!({"imageId":id,"annotationId":aid,"sampleId":image.sample_id,"sourceType":image.source_type,"parameters":serde_json::from_str::<Value>(&annotation_body).map_err(|e|e.to_string())?,"statistics":output}),
            )?)
        } else {
            None
        };
        if let Some(a) = &a {
            let source:String=c.query_row("SELECT source_artifact_id FROM microstructure_images WHERE workspace_id=?1 AND id=?2",params![ws,id],|r|r.get(0)).map_err(err)?;
            artifacts::edge(c, ws, &source, a, "based_on_evidence")?;
        }
        c.execute(
            "INSERT OR IGNORE INTO microstructure_analyses VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                ws,
                Uuid::new_v4().to_string(),
                id,
                aid,
                task,
                output.to_string(),
                error,
                a,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(err)?;
    }
    Ok(())
}
pub fn analyses(c: &Connection, ws: &str, id: &str, reviewed_only: bool) -> Result<Value, String> {
    get(c, ws, id)?;
    let mut q=c.prepare("SELECT a.id,a.task_id,a.annotation_id,a.result_json,a.error_code,r.state,r.note,a.created_at FROM microstructure_analyses a LEFT JOIN microstructure_reviews r ON r.workspace_id=a.workspace_id AND r.analysis_id=a.id WHERE a.workspace_id=?1 AND a.image_id=?2 AND (?3=0 OR r.state='accepted') ORDER BY a.created_at DESC,a.id LIMIT 25").map_err(err)?;
    let rows=q.query_map(params![ws,id,reviewed_only],|r|{let raw:String=r.get(3)?;Ok(json!({"id":r.get::<_,String>(0)?,"taskId":r.get::<_,String>(1)?,"annotationId":r.get::<_,String>(2)?,"result":serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null),"error":r.get::<_,Option<String>>(4)?,"reviewState":r.get::<_,Option<String>>(5)?,"reviewNote":r.get::<_,Option<String>>(6)?,"createdAt":r.get::<_,String>(7)?}))}).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    Ok(json!(rows))
}
pub fn review(c: &Connection, ws: &str, id: &str, state: &str, note: &str) -> Result<(), String> {
    if !matches!(state, "accepted" | "rejected") || note.chars().count() > 2000 {
        return Err("invalid_image_review".into());
    }
    let(image_id,artifact,raw):(String,Option<String>,String)=c.query_row("SELECT image_id,artifact_id,result_json FROM microstructure_analyses WHERE workspace_id=?1 AND id=?2 AND error_code IS NULL",params![ws,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(err)?.ok_or("image_analysis_unavailable")?;
    let image = get(c, ws, &image_id)?;
    projects::require_active(c, ws, &image.project_id)?;
    artifacts::atomic(c, || {
        c.execute(
            "INSERT INTO microstructure_reviews VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                ws,
                Uuid::new_v4().to_string(),
                id,
                state,
                note,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(err)?;
        let a = artifacts::create(
            c,
            ws,
            &image.project_id,
            "analysis_result",
            "显微统计人工审核",
            &format!("image-review:{id}"),
            None,
            json!({"analysisId":id,"imageId":image_id,"state":state,"note":note,"reviewed":true,"statistics":serde_json::from_str::<Value>(&raw).map_err(|e|e.to_string())?}),
        )?;
        if let Some(source) = artifact {
            artifacts::edge(c, ws, &source, &a, "based_on_evidence")?;
        }
        projects::audit(
            c,
            ws,
            &image.project_id,
            "image_analysis_reviewed",
            json!({"analysisId":id,"state":state}),
        )?;
        Ok(())
    })
}
