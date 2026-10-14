//! Prediction schema normalization and audit evidence.
pub(crate) mod risk;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PredictionInputFeature {
    pub name: String,
    #[serde(default)]
    pub unit: Option<String>,
}

/// Reorder by model names and convert using the actual, unchanged materials
/// engine. No field ordinal, guessed alias or unverified model unit is used.
pub fn normalize_prediction_rows(
    artifact: &Value,
    rows: &[Vec<f64>],
    schema: Option<&[PredictionInputFeature]>,
) -> Result<Vec<Vec<f64>>, String> {
    let names = artifact["feature_names"]
        .as_array()
        .ok_or("model_feature_schema_missing")?;
    if names.is_empty() || names.len() > 128 || rows.is_empty() || rows.len() > 10_000 {
        return Err("prediction_schema_budget".into());
    }
    if rows
        .iter()
        .any(|r| r.len() != names.len() || r.iter().any(|v| !v.is_finite()))
    {
        return Err("prediction_rows_invalid".into());
    }
    let Some(schema) = schema else {
        return Ok(rows.to_vec());
    };
    if schema.len() != names.len() {
        return Err("prediction_schema_dimension_mismatch".into());
    }
    let mut unique = HashSet::new();
    if schema.iter().any(|f| !unique.insert(f.name.as_str())) {
        return Err("prediction_schema_duplicate_name".into());
    }
    let mut plan = Vec::new();
    for (model_index, name) in names.iter().enumerate() {
        let name = name.as_str().ok_or("model_feature_name_invalid")?;
        let input_index = schema
            .iter()
            .position(|f| f.name == name)
            .ok_or("prediction_schema_missing_feature")?;
        let metadata = &artifact["feature_schema"]["fields"][model_index];
        let model_unit = if metadata["name"] == name {
            metadata["unit"].as_str()
        } else {
            None
        };
        let input_unit = schema[input_index].unit.as_deref();
        let transform = match (input_unit, model_unit) {
            (Some(from), Some(to)) => crate::steel::materials::conversion(from, to)?,
            (Some(_), None) => return Err("model_units_unknown".into()),
            (None, _) => (1.0, 0.0),
        };
        plan.push((input_index, transform));
    }
    rows.iter()
        .map(|row| {
            plan.iter()
                .map(|(index, (scale, offset))| {
                    let value = row[*index] * scale + offset;
                    if !value.is_finite() {
                        return Err("prediction_unit_conversion_overflow".into());
                    }
                    Ok(value)
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod schema_tests {
    use super::*;
    use serde_json::json;
    fn artifact() -> Value {
        json!({"feature_names":["C","temperature"],"feature_schema":{"fields":[{"name":"C","unit":"wt%"},{"name":"temperature","unit":"degC"}]}})
    }
    fn schema(c_unit: &str) -> Vec<PredictionInputFeature> {
        vec![
            PredictionInputFeature {
                name: "temperature".into(),
                unit: Some("K".into()),
            },
            PredictionInputFeature {
                name: "C".into(),
                unit: Some(c_unit.into()),
            },
        ]
    }
    #[test]
    fn reorders_fields_and_uses_real_fraction_and_temperature_conversions() {
        let out = normalize_prediction_rows(
            &artifact(),
            &[vec![1123.15, 0.002]],
            Some(&schema("fraction")),
        )
        .unwrap();
        assert!((out[0][0] - 0.2).abs() < 1e-12);
        assert!((out[0][1] - 850.0).abs() < 1e-9);
    }
    #[test]
    fn duplicate_and_wrong_feature_names_cannot_be_positional_guesses() {
        let mut s = schema("fraction");
        s[1].name = "temperature".into();
        assert!(
            normalize_prediction_rows(&artifact(), &[vec![1123.15, 0.002]], Some(&s))
                .unwrap_err()
                .contains("duplicate")
        );
        s[1].name = "carbon_alias_not_declared".into();
        assert!(
            normalize_prediction_rows(&artifact(), &[vec![1123.15, 0.002]], Some(&s))
                .unwrap_err()
                .contains("missing_feature")
        );
    }
    #[test]
    fn unknown_legacy_units_do_not_allow_guessed_conversion() {
        let legacy = json!({"feature_names":["C","temperature"]});
        assert_eq!(
            normalize_prediction_rows(&legacy, &[vec![1123.15, 0.002]], Some(&schema("fraction")))
                .unwrap_err(),
            "model_units_unknown"
        );
        assert_eq!(
            normalize_prediction_rows(&legacy, &[vec![0.2, 850.]], None).unwrap(),
            vec![vec![0.2, 850.]]
        );
    }
    #[test]
    fn incompatible_hardness_scales_remain_rejected() {
        let model = json!({"feature_names":["hardness"],"feature_schema":{"fields":[{"name":"hardness","unit":"HV"}]}});
        let fields = [PredictionInputFeature {
            name: "hardness".into(),
            unit: Some("HRC".into()),
        }];
        assert!(normalize_prediction_rows(&model, &[vec![60.]], Some(&fields)).is_err());
    }
    #[test]
    fn conversion_overflow_is_rejected_before_worker_submission() {
        let model = json!({"feature_names":["stress"],"feature_schema":{"fields":[{"name":"stress","unit":"Pa"}]}});
        let fields = [PredictionInputFeature {
            name: "stress".into(),
            unit: Some("MPa".into()),
        }];
        assert_eq!(
            normalize_prediction_rows(&model, &[vec![f64::MAX]], Some(&fields)).unwrap_err(),
            "prediction_unit_conversion_overflow"
        );
    }
    #[test]
    fn empty_schema_and_non_finite_rows_are_rejected() {
        assert!(normalize_prediction_rows(&artifact(), &[vec![0.2, 850.]], Some(&[])).is_err());
        assert!(normalize_prediction_rows(&artifact(), &[vec![f64::NAN, 850.]], None).is_err());
    }
}
