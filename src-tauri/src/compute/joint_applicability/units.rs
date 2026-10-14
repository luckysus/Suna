//! Compile per-model units from the frozen schema using Suna's materials engine.
use serde_json::{json, Map, Value};
use std::collections::HashSet;

pub fn model_transforms(
    objective: &mut Value,
    artifact: &Value,
    variables: &[Value],
) -> Result<(), String> {
    let names = artifact["feature_names"]
        .as_array()
        .filter(|n| !n.is_empty() && n.len() <= 128)
        .ok_or("joint_model_schema_invalid")?;
    let mut unique = HashSet::new();
    let mut transforms = Map::new();
    for (index, raw_name) in names.iter().enumerate() {
        let name = raw_name.as_str().ok_or("joint_model_schema_invalid")?;
        if !unique.insert(name) {
            return Err("joint_model_schema_duplicate".into());
        }
        let variable = variables
            .iter()
            .find(|v| v["name"] == name)
            .ok_or("joint_model_variable_missing")?;
        let declared = &artifact["feature_schema"]["fields"][index];
        let model_unit = if declared["name"] == name {
            unit(&declared["unit"])?
        } else {
            None
        };
        let input_unit = unit(&variable["unit"])?;
        let (scale, shift) = match (input_unit, model_unit) {
            (Some(from), Some(to)) => crate::steel::materials::conversion(from, to)?,
            (Some(_), None) => return Err("joint_model_units_unknown".into()),
            (None, _) => (1., 0.), // Legacy API values are already in model units.
        };
        for key in ["min", "max", "fixedValue"] {
            if let Some(value) = variable[key].as_f64() {
                if !value.is_finite() || !(value * scale + shift).is_finite() {
                    return Err("joint_model_conversion_overflow".into());
                }
            }
        }
        transforms.insert(name.into(), json!({"scale":scale,"shift":shift}));
    }
    // Always overwrite untrusted UI-supplied coefficients.
    objective["feature_transforms"] = Value::Object(transforms);
    let model_unit = unit(&artifact["target_schema"]["unit"])?;
    let requested_unit = unit(&objective["unit"])?;
    let (scale, shift, status) = match (model_unit, requested_unit) {
        (Some(from), Some(to)) => {
            let (scale, shift) = crate::steel::materials::conversion(from, to)?;
            (scale, shift, "verified_conversion")
        }
        (Some(from), None) => {
            objective["unit"] = json!(from);
            (1., 0., "verified_training_unit")
        }
        (None, _) => (1., 0., "unknown_training_unit"),
    };
    objective["prediction_transform"] = json!({"scale":scale,"shift":shift});
    objective["unit_status"] = json!(status);
    Ok(())
}

fn unit(value: &Value) -> Result<Option<&str>, String> {
    if value.is_null() {
        return Ok(None);
    }
    let s = value.as_str().ok_or("joint_unit_invalid")?.trim();
    if s.is_empty() {
        return Ok(None);
    }
    if s.len() > 64 || s.chars().any(char::is_control) {
        return Err("joint_unit_invalid".into());
    }
    Ok(Some(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> Value {
        json!({"model_id":"unchanged","feature_names":["temperature","C"],"feature_schema":{"fields":[{"name":"temperature","unit":"K"},{"name":"C","unit":"fraction"}]},"target_schema":{"unit":"MPa"}})
    }
    fn variables() -> Vec<Value> {
        vec![
            json!({"name":"C","unit":"wt%","min":0.1,"max":0.3}),
            json!({"name":"temperature","unit":"degC","min":700,"max":900}),
        ]
    }
    #[test]
    fn reordered_models_convert_inputs_and_outputs_without_touching_artifacts() {
        let artifact = model();
        let original = artifact.clone();
        let mut spec = json!({"unit":"GPa"});
        model_transforms(&mut spec, &artifact, &variables()).unwrap();
        assert_eq!(
            spec["feature_transforms"]["C"],
            json!({"scale":0.01,"shift":0.0})
        );
        assert_eq!(spec["feature_transforms"]["temperature"]["shift"], 273.15);
        assert_eq!(spec["prediction_transform"]["scale"], 0.001);
        assert_eq!(artifact, original);
    }
    #[test]
    fn untrusted_transform_is_overwritten_and_absent_target_unit_uses_frozen_unit() {
        let mut spec = json!({"feature_transforms":{"C":{"scale":10000}},"prediction_transform":{"scale":10000}});
        model_transforms(&mut spec, &model(), &variables()).unwrap();
        assert_eq!(spec["feature_transforms"]["C"]["scale"], 0.01);
        assert_eq!(spec["prediction_transform"]["scale"], 1.0);
        assert_eq!(spec["unit"], "MPa");
    }
    #[test]
    fn unknown_legacy_units_keep_old_unitless_api_but_refuse_guessed_conversion() {
        let artifact = json!({"feature_names":["C"]});
        let mut spec = json!({"unit":"HV"});
        model_transforms(
            &mut spec,
            &artifact,
            &[json!({"name":"C","min":0.1,"max":0.3})],
        )
        .unwrap();
        assert_eq!(spec["unit_status"], "unknown_training_unit");
        assert!(model_transforms(
            &mut spec,
            &artifact,
            &[json!({"name":"C","unit":"fraction"})]
        )
        .is_err());
    }
    #[test]
    fn mismatched_schema_name_duplicate_dimension_and_hardness_are_refused() {
        let mut spec = json!({"unit":"HRC"});
        assert!(model_transforms(&mut spec, &model(), &variables()).is_err());
        let mut artifact = model();
        artifact["feature_schema"]["fields"][1]["name"] = json!("wrong");
        assert!(model_transforms(&mut json!({}), &artifact, &variables()).is_err());
        let duplicate = json!({"feature_names":["C","C"]});
        assert!(model_transforms(&mut json!({}), &duplicate, &[json!({"name":"C"})]).is_err());
    }
    #[test]
    fn conversion_overflow_and_unknown_explicit_unit_are_refused() {
        let artifact =
            json!({"feature_names":["C"],"feature_schema":{"fields":[{"name":"C","unit":"wt%"}]}});
        assert!(model_transforms(
            &mut json!({}),
            &artifact,
            &[json!({"name":"C","unit":"fraction","max":1e308})]
        )
        .is_err());
        assert!(model_transforms(
            &mut json!({}),
            &artifact,
            &[json!({"name":"C","unit":"made-up"})]
        )
        .is_err());
    }
}
