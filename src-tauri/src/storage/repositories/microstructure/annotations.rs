use super::*;

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
