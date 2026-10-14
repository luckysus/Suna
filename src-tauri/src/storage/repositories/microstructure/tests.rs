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
            gray16_mapping: None,
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
                notes: "".into(),
                gray16_mapping: None,
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
    let result = json!({"images":[{"imageId":i.id,"annotationId":a.id,"result":{"sourceSha256":i.source_sha256,"rasterInput":i.metadata["rasterInput"],"width":i.width,"height":i.height,"metrics":{"thresholdFraction":{"value":50.,"unit":"%","qualifiedName":"阈值区域占比"}},"reviewed":false}}]});
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

fn pixel_fixture(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/microstructure")
            .join(name),
    )
    .unwrap()
}
fn goldens() -> Value {
    serde_json::from_slice(&pixel_fixture("golden.json")).unwrap()
}
#[test]
fn real_gray16_png_and_tiff_byte_orders_match_independent_pixels() {
    let gold = goldens();
    for name in ["gray16-le.tif", "gray16-be.tif", "gray16.png"] {
        let bytes = pixel_fixture(name);
        assert_eq!(hash(&bytes), gold["sourceSha256"][name].as_str().unwrap());
        for (mapping, expected) in [
            (Gray16Mapping::FullRange, "fullRangeGray8"),
            (
                Gray16Mapping::Window {
                    lower: 1000,
                    upper: 5000,
                },
                "window1000to5000Gray8",
            ),
        ] {
            let (image, contract, _) = raster::decode(&bytes, Some(&mapping)).unwrap();
            let pixels: Vec<Vec<u8>> = image
                .to_luma8()
                .rows()
                .map(|row| row.map(|p| p[0]).collect())
                .collect();
            assert_eq!(json!(pixels), gold[expected]);
            assert_eq!(contract["decodedBitDepth"], 16);
            assert_eq!(contract["coordinateSystem"], "raw-raster");
            assert_eq!(contract["grayscaleMapping"]["rounding"], "floor");
            assert_eq!((image.width(), image.height()), (4, 2));
        }
    }
}
#[test]
fn rejects_ambiguous_tiff_and_missing_or_invalid_mapping_before_writing() {
    let (c, root, p) = fixture();
    for (name, mapping) in [
        ("unsupported-pages.tif", Some(Gray16Mapping::FullRange)),
        (
            "unsupported-orientation6.tif",
            Some(Gray16Mapping::FullRange),
        ),
        ("unsupported-float.tif", Some(Gray16Mapping::FullRange)),
        ("gray16-le.tif", None),
        (
            "gray16-le.tif",
            Some(Gray16Mapping::Window { lower: 3, upper: 3 }),
        ),
    ] {
        let path = root.join(name);
        fs::write(&path, pixel_fixture(name)).unwrap();
        assert!(
            import(
                &c,
                "ws",
                &root,
                ImportImage {
                    source_path: path.to_string_lossy().into(),
                    project_id: p.clone(),
                    sample_id: "synthetic".into(),
                    modality: "OM".into(),
                    source_type: "synthetic".into(),
                    magnification: None,
                    acquired_at: None,
                    notes: String::new(),
                    gray16_mapping: mapping,
                }
            )
            .is_err(),
            "{name}"
        );
    }
    assert!(list(&c, "ws", &p, 0).unwrap().is_empty());
    assert!(artifacts::list(&c, "ws", &p, 25, 0).unwrap().is_empty());
    assert!(!root.join("microstructure").exists());
    // Truncated headers, BigTIFF and huge directory offsets cannot panic or allocate.
    for bytes in [
        &b"II\x2a\0\xff\xff\xff\xff"[..],
        &b"II\x2b\0\0\0\0\0"[..],
        &b"MM\0\x2a\0\0\0\x08\xff\xff"[..],
    ] {
        assert!(raster::decode(bytes, Some(&Gray16Mapping::FullRange)).is_err());
    }
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn mapping_preview_and_source_integrity_survive_reopening() {
    let (c, root, p) = fixture();
    let bytes = pixel_fixture("gray16-be.tif");
    let path = root.join("原始16位.tif");
    fs::write(&path, &bytes).unwrap();
    let image = import(
        &c,
        "ws",
        &root,
        ImportImage {
            source_path: path.to_string_lossy().into(),
            project_id: p.clone(),
            sample_id: "synthetic".into(),
            modality: "SEM".into(),
            source_type: "synthetic".into(),
            magnification: Some(1000.),
            acquired_at: None,
            notes: "原像素保留".into(),
            gray16_mapping: Some(Gray16Mapping::Window {
                lower: 1000,
                upper: 5000,
            }),
        },
    )
    .unwrap();
    let png = image::load_from_memory(&preview(&c, "ws", &root, &image.id).unwrap())
        .unwrap()
        .to_luma8();
    assert_eq!(png.dimensions(), (4, 2));
    let actual: Vec<Vec<u8>> = png.rows().map(|r| r.map(|p| p[0]).collect()).collect();
    assert_eq!(json!(actual), goldens()["window1000to5000Gray8"]);
    let reference: String = c
        .query_row(
            "SELECT storage_ref FROM microstructure_images WHERE id=?1",
            [&image.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(fs::read(root.join(&reference)).unwrap(), bytes);
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(image.source_sha256, hash(&bytes));
    let saved = annotate(
        &c,
        "ws",
        &image.id,
        Annotation {
            calibration: Some(Calibration {
                points: [[0., 0.], [2., 0.]],
                distance: 4.,
                unit: "um".into(),
            }),
            rois: vec![Roi {
                kind: "rectangle".into(),
                points: vec![[0., 0.], [2., 2.]],
            }],
            threshold: 128,
            mask_confirmed: true,
        },
    )
    .unwrap();
    let task = submit(&c, "ws", &root, &p, vec![saved.id.clone()]).unwrap();
    let request: Value = serde_json::from_str(&task.payload_json).unwrap();
    assert_eq!(
        request["payload"]["images"][0]["rasterInput"],
        image.metadata["rasterInput"]
    );
    assert_eq!(
        request["payload"]["images"][0]["annotation"]["scaleMicronsPerPixel"],
        2.
    );
    assert!(c
        .execute("UPDATE microstructure_images SET metadata_json='{}'", [])
        .is_err());
    drop(c);
    let c = Connection::open(root.join("研究.sqlite")).unwrap();
    let reopened = get(&c, "ws", &image.id).unwrap();
    assert_eq!(reopened.metadata, image.metadata);
    assert_eq!(
        latest_annotation(&c, "ws", &image.id)
            .unwrap()
            .unwrap()
            .sha256,
        saved.sha256
    );
    drop(c);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn raw_exif_coordinates_and_rgb_tiff_remain_explicit() {
    let (image, meta, _) = raster::decode(&pixel_fixture("raw-orientation6.png"), None).unwrap();
    let pixels: Vec<Vec<u8>> = image
        .to_luma8()
        .rows()
        .map(|r| r.map(|p| p[0]).collect())
        .collect();
    assert_eq!((image.width(), image.height()), (3, 2));
    assert_eq!(json!(pixels), goldens()["rawOrientation6Pixels"]);
    assert_eq!(meta["sourceExifOrientation"], 6);
    assert_eq!(meta["orientationPolicy"], "ignore-exif");
    assert_eq!(meta["iccPolicy"], "unmanaged");
    let (_, meta, _) = raster::decode(&pixel_fixture("rgb8.tif"), None).unwrap();
    assert_eq!(meta["decodedBitDepth"], 8);
    assert_eq!(meta["colorModel"], "rgb");
    assert!(meta["grayscaleMapping"].is_null());
}
