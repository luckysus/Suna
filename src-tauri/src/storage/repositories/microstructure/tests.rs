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
    assert!(annotate(&c, "ws", &i.id, no).unwrap().parameters["scaleMicronsPerPixel"].is_null());
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
    c.execute(
        "UPDATE background_tasks SET state='completed',progress=100,checkpoint_json=?2 WHERE id=?1",
        params![task.id.to_string(), json!({"result":result}).to_string()],
    )
    .unwrap();
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
