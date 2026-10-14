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
mod raster;
pub use raster::Gray16Mapping;
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
    pub gray16_mapping: Option<Gray16Mapping>,
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
    let (raster, contract, extension) = raster::decode(&bytes, input.gray16_mapping.as_ref())?;
    let (w, h) = (raster.width(), raster.height());
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
    let raw = format!("microstructure/{id}.{extension}");
    let preview = format!("microstructure/{id}.preview.png");
    let sha = hash(&bytes);
    let metadata = json!({"magnification":input.magnification,"acquiredAt":input.acquired_at,"notes":input.notes,"coordinateSystem":"raw raster; EXIF ignored","scaleInferredFromMagnification":false,"rasterInput":contract});
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
            .thumbnail(w.min(1024), h.min(1024))
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
#[cfg(test)]
#[path = "microstructure/tests.rs"]
mod tests;

mod analysis;
pub use analysis::{analyses, record_completed, review, submit};

mod annotations;
pub use annotations::{
    annotate, latest_annotation, Annotation, AnnotationRecord, Calibration, Roi,
};
