//! Versioned scientific metadata from exact verified model artifact bytes.
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

fn hash(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn label(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        let normalized = s.trim();
        !normalized.is_empty()
            && s.chars().count() <= 128
            && !s.chars().any(char::is_control)
            && !normalized.starts_with(['/', '\\'])
            && !s.contains("://")
            && !(normalized.len() >= 2 && normalized.as_bytes()[1] == b':')
    })
}

fn identity(value: &Value) -> Option<&str> {
    label(value).filter(|s| s.len() == 36 && uuid::Uuid::parse_str(s).is_ok())
}

fn finite(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite())
}

fn coverage(value: &Value) -> Option<f64> {
    finite(value).filter(|v| (0.0..=1.0).contains(v))
}

fn known_unit(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|unit| unit.len() <= 64 && crate::steel::materials::conversion(unit, unit).is_ok())
}

fn version_label(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| {
        !s.is_empty()
            && s.len() <= 64
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".+_-".contains(&b))
    })
}

fn row_count(value: &Value) -> Option<usize> {
    value
        .as_array()
        .filter(|rows| rows.len() <= 100_000)
        .map(Vec::len)
}

fn metrics(artifact: &Value, split: &str) -> Value {
    let m = &artifact["metrics"][split];
    json!({"sample_count":m["sample_count"].as_u64().filter(|n| *n <= 100_000),
        "rmse":finite(&m["rmse"]),"mae":finite(&m["mae"]),"r2":finite(&m["r2"])})
}

/// Bind the card's data counts to the frozen, completed training task, not to
/// the currently selected/mutable dataset. Missing summaries remain unknown.
pub fn training_registration_context(
    artifact: &Value,
    payload: &Value,
) -> Result<Value, &'static str> {
    if let (Some(artifact_sha), Some(payload_sha)) = (
        hash(&artifact["data_version"]),
        hash(&payload["data_version"]),
    ) {
        if artifact_sha != payload_sha {
            return Err("model_card_dataset_hash_mismatch");
        }
    }
    let mut result = json!({"dataset_id":identity(&payload["dataset_id"])});
    let rows = row_count(&payload["features"]);
    let valid = rows.filter(|count| {
        Some(*count) == row_count(&payload["targets"])
            && Some(*count) == row_count(&payload["source_rows"])
    });
    result["valid_row_count"] = json!(valid);
    let summary = &payload["data_summary"];
    if summary["schema_version"] == "training-data-summary.v1"
        && summary["valid_row_count"].as_u64() == valid.map(|v| v as u64)
    {
        let source = summary["source_row_count"]
            .as_u64()
            .filter(|v| *v <= 100_000);
        let excluded = summary["filtered_row_count"]
            .as_u64()
            .filter(|v| *v <= 100_000);
        if source.is_some()
            && excluded.is_some()
            && source
                == excluded
                    .zip(valid)
                    .map(|(excluded, valid)| excluded + valid as u64)
        {
            result["source_row_count"] = json!(source);
            result["filtered_row_count"] = json!(excluded);
            if summary["filter_policy"] == "exclude_missing_target_reject_invalid_numbers" {
                result["filter_policy"] = summary["filter_policy"].clone();
            }
        }
    }
    if payload["split_field_metadata"]["schema_version"] == "training-split-field.v1" {
        let kind = artifact["evaluation"]["split_kind"]
            .as_str()
            .or(artifact["split"]["kind"].as_str());
        match kind {
            Some("group") => {
                result["grouping_field"] = json!(label(&payload["split_field_metadata"]["name"]))
            }
            Some("time") => {
                result["time_field"] = json!(label(&payload["split_field_metadata"]["name"]))
            }
            _ => {}
        }
    }
    Ok(result)
}

/// Verify the exact registry artifact bytes before extracting allowlisted fields.
/// Missing metadata remains null/unknown; no scientific review is inferred.
pub fn cards(
    artifact_json: &str,
    registry_sha: &str,
    registration: &Value,
) -> Result<Value, &'static str> {
    if artifact_json.len() > 8 * 1024 * 1024 {
        return Err("model_card_artifact_budget");
    }
    if registry_sha.len() != 64
        || !registry_sha.bytes().all(|b| b.is_ascii_hexdigit())
        || format!("{:x}", Sha256::digest(artifact_json.as_bytes()))
            != registry_sha.to_ascii_lowercase()
    {
        return Err("model_card_hash_mismatch");
    }
    let a: Value =
        serde_json::from_str(artifact_json).map_err(|_| "model_card_invalid_artifact")?;
    if !a.is_object() {
        return Err("model_card_invalid_artifact");
    }
    let names = a["feature_names"].as_array().filter(|f| f.len() <= 128);
    let features: Vec<Value> = names
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, name)| {
            let schema = &a["feature_schema"]["fields"][i];
            let unit = if schema["name"] == *name {
                known_unit(&schema["unit"])
            } else {
                None
            };
            json!({"name":label(name), "unit":unit})
        })
        .collect();
    let algorithm = a["model_type"].as_str().filter(|v| {
        [
            "linear_regression",
            "elasticnet",
            "random_forest",
            "hist_gradient_boosting",
            "lightgbm",
            "xgboost",
            "svr",
            "mlp",
            "transformer",
        ]
        .contains(v)
    });
    let allowed_parameters = [
        "ridge",
        "alpha",
        "l1_ratio",
        "n_estimators",
        "max_depth",
        "min_samples_leaf",
        "learning_rate",
        "max_iter",
        "max_leaf_nodes",
        "c",
        "epsilon",
        "hidden_layer_size",
        "epochs",
        "d_model",
        "heads",
        "layers",
        "d_ff",
        "train_rows",
        "seed",
    ];
    let mut parameters = Map::new();
    for key in allowed_parameters {
        if let Some(value) = finite(&a["parameters"][key]) {
            parameters.insert(key.into(), json!(value));
        }
    }
    if let Some(value) = a["parameters"]["train_subsampled"].as_bool() {
        parameters.insert("train_subsampled".into(), json!(value));
    }
    let mut dependencies = Map::new();
    for key in [
        "python",
        "numpy",
        "scipy",
        "scikit-learn",
        "lightgbm",
        "xgboost",
    ] {
        if let Some(version) = version_label(&a["runtime_dependencies"][key]) {
            dependencies.insert(key.into(), json!(version));
        }
    }
    let seed = a["split"]["seed"]
        .as_i64()
        .or(a["parameters"]["seed"].as_i64());
    let kind = a["evaluation"]["split_kind"]
        .as_str()
        .or(a["split"]["kind"].as_str())
        .filter(|v| ["random", "group", "time"].contains(v));
    let calibrated = a["conformal"]["method"] == "split_conformal";
    let tested_coverage = if calibrated
        && a["metrics"]["test"]["sample_count"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 100_000)
    {
        coverage(&a["evaluation"]["test_interval_coverage"])
    } else {
        None
    };
    let target = json!({"name":label(&a["target_schema"]["name"]),"unit":known_unit(&a["target_schema"]["unit"])});
    let dataset_sha = hash(&a["data_version"]);
    let version = registration["version"].as_u64().filter(|v| *v > 0);
    let mut result = json!({
        "model_card":{
            "schema_version":"model-card.v1", "registry_id":identity(&registration["id"]),
            "version":version, "source_task_id":identity(&registration["source_task_id"]),
            "model_sha256":registry_sha.to_ascii_lowercase(), "algorithm":algorithm,
            "parameters":parameters, "random_seed":seed, "features":features, "target":target,
            "artifact_version":version_label(&a["artifact_version"]),
            "runtime_dependencies":dependencies, "environment_status":if dependencies.is_empty() {"unknown"} else {"partially_recorded"},
            "dataset_sha256":dataset_sha, "split_kind":kind,
            "applicability":{"method":if a["applicability_support"]["method"]=="marginal_training_ranges" {"marginal_training_ranges"} else {"unknown"},
                "support_sha256":hash(&a["applicability_support"]["support_sha256"]),"joint_support":"not_assessed","probability":null},
            "split_counts":{"train":row_count(&a["split"]["train_indices"]),"validation":row_count(&a["split"]["validation_indices"]),"test":row_count(&a["split"]["test_indices"]),"calibration":row_count(&a["split"]["calibration_indices"])},
            "metrics":{"train":metrics(&a,"train"),"validation":metrics(&a,"validation"),"test":metrics(&a,"test")},
            "interval":{"status":if calibrated {"recorded"} else {"unknown"},
                "nominal_coverage":if calibrated {coverage(&a["conformal"]["confidence"])} else {None},
                "observed_test_coverage":tested_coverage,
                "calibration_sample_count":a["conformal"]["sample_count"].as_u64().filter(|n| *n <= 100_000),
                "exchangeability_verified":false,
                "applicability":"requires_exchangeable_calibration_and_future_observations"},
            "review_status":"unreviewed", "physical_feasibility":"not_assessed"
        },
        "data_card":{
            "schema_version":"data-card.v1", "dataset_id":identity(&registration["dataset_id"]),
            "dataset_sha256":dataset_sha, "features":features, "target":target,
            "filtered_row_count":registration["filtered_row_count"].as_u64().filter(|n| *n<=100_000),
            "valid_row_count":registration["valid_row_count"].as_u64().filter(|n| *n<=100_000),
            "source_row_count":registration["source_row_count"].as_u64().filter(|n| *n<=100_000),
            "filter_policy":if registration["filter_policy"]=="exclude_missing_target_reject_invalid_numbers" {"exclude_missing_target_reject_invalid_numbers"} else {"unknown"},
            "grouping_field":label(&registration["grouping_field"]),
            "time_field":label(&registration["time_field"]),
            "imputation":if a["preprocessing"]["fit_scope"]=="train_only" && a["preprocessing"]["imputation"]=="train_mean" {"train_mean"} else {"unknown"},
            "quality_status":"unknown", "review_status":"unreviewed"
        }
    });
    let body = result.to_string();
    result["cards_sha256"] = json!(format!("{:x}", Sha256::digest(body.as_bytes())));
    Ok(result)
}

/// Persist a scientific-card snapshot in the existing registry manifest. The
/// artifact bytes and model SHA stay unchanged; no second registry is created.
pub fn registration_manifest(
    artifact_json: &str,
    registry_sha: &str,
    manifest_json: &str,
    registration: &Value,
) -> Result<Value, &'static str> {
    if manifest_json.len() > 8 * 1024 * 1024 + 256 * 1024 {
        return Err("model_card_manifest_budget");
    }
    let mut manifest: Value =
        serde_json::from_str(manifest_json).map_err(|_| "model_card_invalid_manifest")?;
    if !manifest.is_object() || manifest.get("scientific_cards").is_some() {
        return Err("model_card_invalid_manifest");
    }
    manifest["scientific_cards"] = cards(artifact_json, registry_sha, registration)?;
    Ok(manifest)
}

/// Verify an exact registered model and its persisted card snapshot without
/// exposing the source manifest (which may contain the actual model blob).
pub fn read_registered_cards(
    artifact_json: &str,
    registry_sha: &str,
    manifest_json: &str,
    registration: &Value,
) -> Result<Value, &'static str> {
    if manifest_json.len() > 8 * 1024 * 1024 + 256 * 1024 {
        return Err("model_card_manifest_budget");
    }
    // Always verify the actual artifact, even for a legacy registration.
    let expected = cards(artifact_json, registry_sha, registration)?;
    let manifest: Value =
        serde_json::from_str(manifest_json).map_err(|_| "model_card_invalid_manifest")?;
    let Some(saved) = manifest.get("scientific_cards") else {
        return Ok(json!({"status":"legacy_derived","persisted":false,"cards":expected}));
    };
    let mut body = saved.clone();
    let recorded_hash = hash(&body["cards_sha256"])
        .ok_or("model_card_snapshot_hash_mismatch")?
        .to_owned();
    body.as_object_mut()
        .ok_or("model_card_invalid_snapshot")?
        .remove("cards_sha256");
    if format!("{:x}", Sha256::digest(body.to_string().as_bytes())) != recorded_hash {
        return Err("model_card_snapshot_hash_mismatch");
    }
    // Re-derive the exact allowlisted fields from the verified model and frozen
    // registration context. A digest alone is not validation of card contents.
    if *saved != expected {
        return Err("model_card_snapshot_source_mismatch");
    }
    Ok(json!({"status":"recorded","persisted":true,"cards":expected}))
}

#[cfg(test)]
#[path = "model_cards/tests.rs"]
mod tests;
