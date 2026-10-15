//! Match the existing Worker's evaluation contract before creating a task.
use serde_json::{json, Value};

pub(super) fn options(policy: &Value, algorithm: &str) -> Result<Option<Value>, String> {
    if ![
        "folds",
        "testFraction",
        "calibrationFraction",
        "searchTrials",
        "learningCurve",
    ]
    .iter()
    .any(|key| policy.get(*key).is_some())
    {
        return Ok(None);
    }
    let options = json!({
        "folds":policy.get("folds").cloned().unwrap_or(json!(0)),
        "test_fraction":policy.get("testFraction").cloned().unwrap_or(json!(0.0)),
        "calibration_fraction":policy.get("calibrationFraction").cloned().unwrap_or(json!(0.0)),
        "search_trials":policy.get("searchTrials").cloned().unwrap_or(json!(0)),
        "learning_curve":policy.get("learningCurve").cloned().unwrap_or(json!(false))
    });
    let invalid = || "scientific_evaluation_options_invalid".to_owned();
    let folds = options["folds"]
        .as_u64()
        .filter(|n| *n == 0 || (2..=10).contains(n))
        .ok_or_else(invalid)?;
    let trials = options["search_trials"]
        .as_u64()
        .filter(|n| *n <= 12)
        .ok_or_else(invalid)?;
    let learning = options["learning_curve"].as_bool().ok_or_else(invalid)?;
    let fraction = |key: &str| {
        options[key]
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..0.5).contains(n))
            .ok_or_else(invalid)
    };
    let test = fraction("test_fraction")?;
    let calibration = fraction("calibration_fraction")?;
    if test + calibration >= 0.7 {
        return Err(invalid());
    }
    if (learning || trials > 0) && folds == 0 {
        return Err("scientific_evaluation_requires_cross_validation".into());
    }
    if trials > 0
        && ![
            "elasticnet",
            "svr",
            "random_forest",
            "hist_gradient_boosting",
            "lightgbm",
            "xgboost",
        ]
        .contains(&algorithm)
    {
        return Err("scientific_evaluation_search_algorithm_unsupported".into());
    }
    if trials == 0
        && !learning
        && !["folds", "testFraction", "calibrationFraction"]
            .iter()
            .any(|key| policy.get(*key).is_some())
    {
        // Explicitly disabled standalone flags were ignored by older versions;
        // keep the same payload and split instead of enabling evaluation.
        return Ok(None);
    }
    Ok(Some(options))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_worker_incompatible_evaluation_before_submission_and_keeps_disabled_legacy_flags() {
        for policy in [
            json!({"folds":true}),
            json!({"folds":1}),
            json!({"folds":11}),
            json!({"folds":2,"searchTrials":13}),
            json!({"testFraction":true}),
            json!({"testFraction":0.5}),
            json!({"calibrationFraction":-0.1}),
            json!({"testFraction":0.4,"calibrationFraction":0.4}),
            json!({"learningCurve":1}),
            json!({"learningCurve":null}),
        ] {
            assert_eq!(
                options(&policy, "random_forest").unwrap_err(),
                "scientific_evaluation_options_invalid"
            );
        }
        assert_eq!(
            options(&json!({"folds":2,"searchTrials":1}), "linear_regression").unwrap_err(),
            "scientific_evaluation_search_algorithm_unsupported"
        );
        assert!(options(
            &json!({"searchTrials":0,"learningCurve":false}),
            "linear_regression"
        )
        .unwrap()
        .is_none());
        assert!(options(
            &json!({"folds":0,"testFraction":0,"calibrationFraction":0}),
            "linear_regression"
        )
        .unwrap()
        .is_some());
    }
}
