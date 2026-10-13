//! Managed original rasters, immutable annotation versions and reviewed statistics.
use super::{research_artifacts as artifacts, research_projects as projects};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;
const MAX_BYTES: u64 = 32 * 1024 * 1024;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportImage {
    pub source_path: String,
    pub project_id: String,
    pub sample_id: String,
    pub modality: String,
    pub source_type: String,
    pub magnification: Option<f64>,
    pub acquired_at: Option<String>,
    #[serde(default)]
    pub notes: String,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageRecord {
    pub id: String,
    pub project_id: String,
    pub sample_id: String,
    pub modality: String,
    pub source_type: String,
    pub source_sha256: String,
    pub width: u32,
    pub height: u32,
    pub metadata: Value,
    pub created_at: String,
}
fn image_row(r: &rusqlite::Row) -> rusqlite::Result<ImageRecord> {
    let raw: String = r.get(8)?;
    Ok(ImageRecord {
        id: r.get(0)?,
        project_id: r.get(1)?,
        sample_id: r.get(2)?,
        modality: r.get(3)?,
        source_type: r.get(4)?,
        source_sha256: r.get(5)?,
        width: r.get(6)?,
        height: r.get(7)?,
        metadata: serde_json::from_str(&raw).unwrap_or(Value::Null),
        created_at: r.get(9)?,
    })
}
const SELECT:&str="SELECT id,project_id,sample_id,modality,source_type,source_sha256,width,height,metadata_json,created_at FROM microstructure_images";
pub fn get(c: &Connection, ws: &str, id: &str) -> Result<ImageRecord, String> {
    c.query_row(
        &format!("{SELECT} WHERE workspace_id=?1 AND id=?2"),
        params![ws, id],
        image_row,
    )
    .optional()
    .map_err(err)?
    .ok_or("image_not_found".into())
}
pub fn list(c: &Connection, ws: &str, p: &str, offset: u32) -> Result<Vec<ImageRecord>, String> {
    projects::get(c, ws, p)?;
    let mut q=c.prepare(&format!("{SELECT} WHERE workspace_id=?1 AND project_id=?2 ORDER BY created_at DESC,id LIMIT 25 OFFSET ?3")).map_err(err)?;
    let result = q
        .query_map(params![ws, p, offset], image_row)
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err);
    result
}
fn managed(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    if rel.is_absolute()
        || rel
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err("invalid_image_reference".into());
    }
    let path = root.join(rel);
    let resolved = path
        .canonicalize()
        .map_err(|_| "image_source_unavailable")?;
    if !resolved.starts_with(root.canonicalize().map_err(|_| "invalid_data_root")?) {
        return Err("image_path_outside_data_root".into());
    }
    Ok(resolved)
}
pub fn import(
    c: &Connection,
    ws: &str,
    root: &Path,
    input: ImportImage,
) -> Result<ImageRecord, String> {
    projects::require_active(c, ws, &input.project_id)?;
    if input.sample_id.trim().is_empty()
        || input.sample_id.chars().count() > 160
        || input.notes.chars().count() > 5000
        || !matches!(input.modality.as_str(), "OM" | "SEM" | "EBSD-export")
        || !matches!(
            input.source_type.as_str(),
            "measured" | "synthetic" | "literature"
        )
        || input
            .magnification
            .is_some_and(|m| !m.is_finite() || m <= 0.)
        || input
            .acquired_at
            .as_ref()
            .is_some_and(|d| chrono::DateTime::parse_from_rfc3339(d).is_err())
    {
        return Err("invalid_image_metadata".into());
    }
    let (_, mut file) = crate::permissions::path::authorize_existing_file_with_handle(Path::new(
        &input.source_path,
    ))
    .map_err(|_| "image_path_not_authorized")?;
    let mut bytes = vec![];
    Read::by_ref(&mut file)
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "image_read_failed")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("image_byte_limit".into());
    }
    let format = image::guess_format(&bytes).map_err(|_| "image_decode_failed")?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("supported_images_png_jpeg_only".into());
    }
    let reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    let (w, h) = reader
        .into_dimensions()
        .map_err(|_| "image_decode_failed")?;
    if w == 0 || h == 0 || w as u64 * h as u64 > 16_000_000 || w > 8192 || h > 8192 {
        return Err("image_pixel_limit".into());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let raster = reader.decode().map_err(|_| "image_decode_failed")?;
    if !matches!(
        raster.color(),
        image::ColorType::L8
            | image::ColorType::La8
            | image::ColorType::Rgb8
            | image::ColorType::Rgba8
    ) {
        return Err("unsupported_image_depth".into());
    }
    let id = Uuid::new_v4().to_string();
    let directory = root.join("microstructure");
    fs::create_dir_all(&directory).map_err(|_| "image_storage_failed")?;
    if !directory
        .canonicalize()
        .map_err(|_| "image_storage_failed")?
        .starts_with(root.canonicalize().map_err(|_| "invalid_data_root")?)
    {
        return Err("image_path_outside_data_root".into());
    }
    let raw = format!(
        "microstructure/{id}.{}",
        if format == image::ImageFormat::Png {
            "png"
        } else {
            "jpg"
        }
    );
    let preview = format!("microstructure/{id}.preview.png");
    let sha = hash(&bytes);
    let metadata = json!({"magnification":input.magnification,"acquiredAt":input.acquired_at,"notes":input.notes,"coordinateSystem":"raw raster; EXIF ignored","scaleInferredFromMagnification":false});
    let write = || -> Result<(), String> {
        let mut raw_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(&raw))
            .map_err(|_| "image_storage_failed")?;
        raw_file
            .write_all(&bytes)
            .and_then(|_| raw_file.sync_all())
            .map_err(|_| "image_storage_failed")?;
        let mut thumb = Cursor::new(vec![]);
        let mut rgb = image::DynamicImage::new_rgb8(w, h).to_rgb8();
        for (x, y, p) in raster.to_rgba8().enumerate_pixels() {
            let a = p[3] as u32;
            rgb.put_pixel(
                x,
                y,
                image::Rgb(
                    [
                        (p[0] as u32 * a + 255 * (255 - a)) / 255,
                        (p[1] as u32 * a + 255 * (255 - a)) / 255,
                        (p[2] as u32 * a + 255 * (255 - a)) / 255,
                    ]
                    .map(|v| v as u8),
                ),
            );
        }
        image::DynamicImage::ImageRgb8(rgb)
            .thumbnail(1024, 1024)
            .write_to(&mut thumb, image::ImageFormat::Png)
            .map_err(|_| "image_preview_failed")?;
        let mut dest = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(&preview))
            .map_err(|_| "image_storage_failed")?;
        dest.write_all(thumb.get_ref())
            .and_then(|_| dest.sync_all())
            .map_err(|_| "image_storage_failed")?;
        Ok(())
    };
    let result=write().and_then(|_|artifacts::atomic(c,||{let a=artifacts::create(c,ws,&input.project_id,"analysis_result","原始显微图像",&format!("image:{id}"),None,json!({"imageId":id,"sourceSha256":sha,"width":w,"height":h,"sampleId":input.sample_id,"modality":input.modality,"sourceType":input.source_type,"metadata":metadata}))?;c.execute("INSERT INTO microstructure_images VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",params![ws,id,input.project_id,input.sample_id,input.modality,input.source_type,metadata.to_string(),sha,raw,preview,w,h,a,Utc::now().to_rfc3339()]).map_err(err)?;projects::audit(c,ws,&input.project_id,"image_imported",json!({"imageId":id,"sha256":sha}))?;get(c,ws,&id)}));
    if result.is_err() {
        let _ = fs::remove_file(root.join(raw));
        let _ = fs::remove_file(root.join(preview));
    }
    result
}
pub fn preview(c: &Connection, ws: &str, root: &Path, id: &str) -> Result<Vec<u8>, String> {
    get(c, ws, id)?;
    let reference: String = c
        .query_row(
            "SELECT preview_ref FROM microstructure_images WHERE workspace_id=?1 AND id=?2",
            params![ws, id],
            |r| r.get(0),
        )
        .map_err(err)?;
    let mut data = vec![];
    fs::File::open(managed(root, &reference)?)
        .map_err(|_| "image_preview_unavailable")?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|_| "image_preview_unavailable")?;
    if data.len() > 4 * 1024 * 1024 {
        return Err("image_preview_limit".into());
    }
    Ok(data)
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Calibration {
    pub points: [[f64; 2]; 2],
    pub distance: f64,
    pub unit: String,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Roi {
    pub kind: String,
    pub points: Vec<[f64; 2]>,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Annotation {
    pub calibration: Option<Calibration>,
    #[serde(default)]
    pub rois: Vec<Roi>,
    pub threshold: u8,
    #[serde(default)]
    pub mask_confirmed: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationRecord {
    pub id: String,
    pub image_id: String,
    pub parameters: Value,
    pub sha256: String,
}
fn validate(a: &Annotation, image: &ImageRecord) -> Result<Value, String> {
    let inside = |p: &[f64; 2]| {
        p.iter().all(|v| v.is_finite() && *v >= 0.)
            && p[0] <= image.width as f64
            && p[1] <= image.height as f64
    };
    if a.rois.len() > 20
        || a.rois.iter().any(|r| {
            !matches!(r.kind.as_str(), "rectangle" | "polygon")
                || r.points.len() > 256
                || r.points.iter().any(|p| !inside(p))
                || (r.kind == "rectangle" && r.points.len() != 2)
                || (r.kind == "polygon" && r.points.len() < 3)
        })
    {
        return Err("invalid_image_roi".into());
    }
    let scale = if let Some(cal) = &a.calibration {
        if cal.points.iter().any(|p| !inside(p)) || !cal.distance.is_finite() || cal.distance <= 0.
        {
            return Err("invalid_image_calibration".into());
        }
        let px = (cal.points[0][0] - cal.points[1][0]).hypot(cal.points[0][1] - cal.points[1][1]);
        if px < 1e-6 {
            return Err("zero_scale_span".into());
        }
        let value = crate::steel::materials::convert(cal.distance, &cal.unit, "um")? / px;
        if !value.is_finite() || value <= 0. {
            return Err("invalid_image_calibration".into());
        }
        Some(value)
    } else {
        None
    };
    let mut value = json!(a);
    value["scaleMicronsPerPixel"] = json!(scale);
    Ok(value)
}
pub fn annotate(
    c: &Connection,
    ws: &str,
    id: &str,
    a: Annotation,
) -> Result<AnnotationRecord, String> {
    let image = get(c, ws, id)?;
    projects::require_active(c, ws, &image.project_id)?;
    let parameters = validate(&a, &image)?;
    let sha = hash(parameters.to_string().as_bytes());
    let version = Uuid::new_v4().to_string();
    artifacts::atomic(c, || {
        c.execute(
            "INSERT INTO microstructure_annotations VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                ws,
                version,
                id,
                parameters.to_string(),
                sha,
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(err)?;
        projects::audit(
            c,
            ws,
            &image.project_id,
            "image_annotation_saved",
            json!({"imageId":id,"annotationId":version,"sha256":sha}),
        )?;
        Ok(AnnotationRecord {
            id: version,
            image_id: id.into(),
            parameters,
            sha256: sha,
        })
    })
}
pub fn latest_annotation(
    c: &Connection,
    ws: &str,
    id: &str,
) -> Result<Option<AnnotationRecord>, String> {
    get(c, ws, id)?;
    c.query_row("SELECT id,parameters_json,content_sha256 FROM microstructure_annotations WHERE workspace_id=?1 AND image_id=?2 ORDER BY created_at DESC,id LIMIT 1",params![ws,id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).optional().map_err(err)?.map(|(aid,body,sha)|{if hash(body.as_bytes())!=sha{return Err("image_annotation_hash_mismatch".into())}Ok(AnnotationRecord{id:aid,image_id:id.into(),parameters:serde_json::from_str(&body).map_err(|e|e.to_string())?,sha256:sha})}).transpose()
}
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
        inputs.push(json!({"imageId":image_id,"annotationId":id,"annotation":serde_json::from_str::<Value>(&body).map_err(|e|e.to_string())?,"path":path,"sha256":image.source_sha256}));
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

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Connection, PathBuf, String) {
        let root = std::env::temp_dir().join(format!("suna-图像-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut c = Connection::open(root.join("研究.sqlite")).unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let p = projects::save(
            &mut c,
            "ws",
            projects::SaveResearchProject {
                id: None,
                name: "合成图像测试".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        (c, root, p.id)
    }
    fn image(c: &Connection, root: &Path, p: &str) -> ImageRecord {
        let path = root.join("已知区域.png");
        image::GrayImage::from_fn(20, 10, |x, _| image::Luma([if x < 10 { 255 } else { 0 }]))
            .save(&path)
            .unwrap();
        import(
            c,
            "ws",
            root,
            ImportImage {
                source_path: path.to_string_lossy().into(),
                project_id: p.into(),
                sample_id: "synthetic-1".into(),
                modality: "OM".into(),
                source_type: "synthetic".into(),
                magnification: Some(100.),
                acquired_at: None,
                notes: "合成验证图".into(),
            },
        )
        .unwrap()
    }
    fn annotation() -> Annotation {
        Annotation {
            calibration: Some(Calibration {
                points: [[0., 0.], [10., 0.]],
                distance: 20.,
                unit: "um".into(),
            }),
            rois: vec![],
            threshold: 128,
            mask_confirmed: true,
        }
    }
    #[test]
    fn immutable_image_annotation_units_and_scoping() {
        let (c, root, p) = fixture();
        let i = image(&c, &root, &p);
        let a = annotate(&c, "ws", &i.id, annotation()).unwrap();
        assert_eq!(a.parameters["scaleMicronsPerPixel"], 2.);
        let mut equivalent = annotation();
        equivalent.calibration.as_mut().unwrap().distance = 0.02;
        equivalent.calibration.as_mut().unwrap().unit = "mm".into();
        assert_eq!(
            annotate(&c, "ws", &i.id, equivalent).unwrap().parameters["scaleMicronsPerPixel"],
            2.
        );
        let mut no = annotation();
        no.calibration = None;
        assert!(
            annotate(&c, "ws", &i.id, no).unwrap().parameters["scaleMicronsPerPixel"].is_null()
        );
        let mut bad = annotation();
        bad.calibration.as_mut().unwrap().points = [[0., 0.], [0., 0.]];
        assert!(annotate(&c, "ws", &i.id, bad).is_err());
        assert!(get(&c, "other", &i.id).is_err());
        assert!(preview(&c, "other", &root, &i.id).is_err());
        assert!(c
            .execute("UPDATE microstructure_images SET width=1", [])
            .is_err());
        assert!(c
            .execute("DELETE FROM microstructure_annotations", [])
            .is_err());
        let bytes = fs::read(root.join("已知区域.png")).unwrap();
        let reference: String = c
            .query_row("SELECT storage_ref FROM microstructure_images", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(fs::read(root.join(reference)).unwrap(), bytes);
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_bad_file_and_relative_path_without_partial_rows() {
        let (c, root, p) = fixture();
        let path = root.join("坏图.png");
        fs::write(&path, b"broken").unwrap();
        for source in ["relative.png".to_string(), path.to_string_lossy().into()] {
            assert!(import(
                &c,
                "ws",
                &root,
                ImportImage {
                    source_path: source,
                    project_id: p.clone(),
                    sample_id: "s".into(),
                    modality: "OM".into(),
                    source_type: "synthetic".into(),
                    magnification: None,
                    acquired_at: None,
                    notes: "".into()
                }
            )
            .is_err());
        }
        assert!(list(&c, "ws", &p, 0).unwrap().is_empty());
        assert!(artifacts::list(&c, "ws", &p, 25, 0).unwrap().is_empty());
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn submit_locks_parameters_and_review_is_append_only() {
        let (c, root, p) = fixture();
        let i = image(&c, &root, &p);
        let a = annotate(&c, "ws", &i.id, annotation()).unwrap();
        let task = submit(&c, "ws", &root, &p, vec![a.id.clone()]).unwrap();
        let payload: Value = serde_json::from_str(&task.payload_json).unwrap();
        assert_eq!(
            payload["payload"]["images"][0]["annotation"]["scaleMicronsPerPixel"],
            2.
        );
        annotate(
            &c,
            "ws",
            &i.id,
            Annotation {
                threshold: 1,
                ..annotation()
            },
        )
        .unwrap();
        assert_eq!(
            payload["payload"]["images"][0]["annotation"]["threshold"],
            128
        );
        let result = json!({"images":[{"imageId":i.id,"annotationId":a.id,"result":{"sourceSha256":i.source_sha256,"metrics":{"thresholdFraction":{"value":50.,"unit":"%","qualifiedName":"阈值区域占比"}},"reviewed":false}}]});
        c.execute("UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",params![task.id.to_string(),json!({"result":result}).to_string()]).unwrap();
        artifacts::reconcile_task(&c, "ws", &task.id.to_string()).unwrap();
        let rows = analyses(&c, "ws", &i.id, false).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert!(analyses(&c, "ws", &i.id, true)
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        let id = rows[0]["id"].as_str().unwrap();
        review(&c, "ws", id, "accepted", "人工审核合成区域").unwrap();
        assert!(review(&c, "ws", id, "rejected", "改写").is_err());
        assert_eq!(
            analyses(&c, "ws", &i.id, true)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(artifacts::list(&c, "ws", &p, 25, 0).unwrap().len(), 3);
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn managed_reference_cannot_escape_data_root() {
        let (c, root, p) = fixture();
        let i = image(&c, &root, &p);
        assert!(managed(&root, "../outside").is_err());
        assert!(managed(&root, "/absolute").is_err());
        assert!(preview(&c, "ws", &root, &i.id)
            .unwrap()
            .starts_with(b"\x89PNG"));
        drop(c);
        fs::remove_dir_all(root).unwrap();
    }
}
