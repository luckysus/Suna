import copy
import json

import pytest

from suna_worker.training import _artifact_id, train_linear_regression
from suna_worker.applicability import MAX_BATCH_ROWS, assess_batch, build_training_support


@pytest.fixture
def artifact():
    return train_linear_regression({
        "features": [[0.1, 800], [0.2, 850], [0.3, 900], [0.4, 950], [10, 1300], [20, 1800]],
        "targets": [101, 102, 103, 104, 200, 300],
        "feature_names": ["C", "temperature"],
        "split_policy": {"kind": "time", "validation_fraction": 0.33},
        "data_version": "b" * 64,
    })


@pytest.fixture
def support(artifact):
    return build_training_support(artifact, {"C": "wt%", "temperature": "degC"})


SCHEMA = [{"name": "C", "unit": "wt%"}, {"name": "temperature", "unit": "degC"}]


def test_training_only_ranges_exclude_future_holdout_outliers(artifact, support):
    assert artifact["split"]["train_indices"] == [0, 1, 2, 3]
    result = assess_batch(support, SCHEMA, [[0.2, 850], [10, 1300]])
    assert [row["state"] for row in result["rows"]] == ["within_domain", "outside_domain"]
    evidence = result["rows"][1]["reasons"]
    assert {item["feature"] for item in evidence} == {"C", "temperature"}
    assert evidence[0]["threshold"]["max"] == 0.4
    assert evidence[1]["threshold"]["max"] == 950
    assert all(item["threshold"]["source"] == "training_split" for item in evidence)
    assert result["probability"] is None
    assert result["physical_feasibility"] == result["joint_support"] == "not_assessed"


def test_feature_order_is_matched_by_name_not_ordinal(support):
    expected = assess_batch(support, SCHEMA, [[0.2, 850], [0.8, 850]])
    actual = assess_batch(support, list(reversed(SCHEMA)), [[850, 0.2], [850, 0.8]])
    assert actual == expected


def test_different_units_need_explicit_existing_engine_conversion(support):
    fraction_schema = [{"name": "C", "unit": "fraction"}, SCHEMA[1]]
    result = assess_batch(support, fraction_schema, [[0.002, 850]])
    assert result["rows"][0]["state"] == "unknown"
    assert result["rows"][0]["reasons"][0]["code"] == "unit_conversion_required"
    # Canonical values here are synthetic fixtures. This is not a new converter.
    assert assess_batch(support, SCHEMA, [[0.2, 850]])["rows"][0]["state"] == "within_domain"


def test_imputed_missing_values_are_warnings(support):
    row = assess_batch(support, SCHEMA, [[None, 850]])["rows"][0]
    assert row["state"] == "warning"
    assert row["reasons"][0]["code"] == "imputed_with_training_mean"


def test_missing_feature_is_unknown_without_positional_guess(support):
    row = assess_batch(support, [SCHEMA[0]], [[0.2]])["rows"][0]
    assert row["state"] == "unknown"
    assert row["reasons"][0]["feature"] == "temperature"


@pytest.mark.parametrize("field", ["applicability_range", "data_version", "preprocessing", "model_id"])
def test_legacy_missing_evidence_is_unknown(artifact, field):
    artifact.pop(field)
    if field != "model_id":
        artifact["model_id"] = _artifact_id(artifact)
    support = build_training_support(artifact, {"C": "wt%", "temperature": "degC"})
    row = assess_batch(support, SCHEMA, [[0.2, 850]])["rows"][0]
    assert row["state"] == "unknown"
    assert row["reasons"]


def test_empty_training_support_is_unknown(artifact):
    artifact["applicability_range"] = [{"min": None, "max": None}] * 2
    artifact["model_id"] = _artifact_id(artifact)
    support = build_training_support(artifact, {"C": "wt%", "temperature": "degC"})
    row = assess_batch(support, SCHEMA, [[0.2, 850]])["rows"][0]
    assert row["state"] == "unknown"
    assert all(item["code"] == "missing_training_limits" for item in row["reasons"])


def test_constant_training_feature_uses_actual_constant_range():
    artifact = train_linear_regression({
        "features": [[5], [5], [5], [100]], "targets": [1, 2, 3, 4],
        "feature_names": ["time"], "data_version": "b" * 64,
        "split_policy": {"kind": "time", "validation_fraction": 0.25},
    })
    support = build_training_support(artifact, {"time": "min"})
    result = assess_batch(support, [{"name": "time", "unit": "min"}], [[5], [5.01]])
    assert [row["state"] for row in result["rows"]] == ["within_domain", "outside_domain"]
    assert result["rows"][1]["reasons"][0]["threshold"]["min"] == 5


@pytest.mark.parametrize("unit", [None, ""])
def test_unspecified_unit_is_unknown(support, unit):
    schema = [{"name": "C", "unit": unit}, SCHEMA[1]]
    row = assess_batch(support, schema, [[0.2, 850]])["rows"][0]
    assert row["state"] == "unknown"
    assert row["reasons"][0]["code"] == "missing_unit"


def test_verified_violation_is_not_hidden_by_another_unknown_unit(support):
    schema = [{"name": "C", "unit": None}, SCHEMA[1]]
    row = assess_batch(support, schema, [[0.2, 1400]])["rows"][0]
    assert row["state"] == "outside_domain"
    assert {item["state"] for item in row["reasons"]} == {"unknown", "outside_domain"}


@pytest.mark.parametrize("value", [float("nan"), float("inf"), float("-inf"), True, "0.2"])
def test_invalid_input_cannot_enter_risk_results(support, value):
    with pytest.raises(ValueError, match="non_finite_or_non_numeric_input"):
        assess_batch(support, SCHEMA, [[value, 850]])


def test_duplicate_and_unexpected_names_are_rejected(support):
    with pytest.raises(ValueError, match="duplicate_feature_name"):
        assess_batch(support, [SCHEMA[0], SCHEMA[0]], [[0.2, 0.3]])
    with pytest.raises(ValueError, match="unexpected_input_feature"):
        assess_batch(support, [{"name": "unknown", "unit": "wt%"}], [[0.2]])


def test_dimensions_and_batch_size_are_bounded(support):
    with pytest.raises(ValueError, match="input_dimension_mismatch"):
        assess_batch(support, SCHEMA, [[0.2]])
    with pytest.raises(ValueError, match="invalid_batch_size"):
        assess_batch(support, SCHEMA, [[0.2, 850]] * (MAX_BATCH_ROWS + 1))


def test_support_digest_survives_json_and_detects_mutation(support):
    assert assess_batch(json.loads(json.dumps(support)), SCHEMA, [[0.2, 850]]) == assess_batch(support, SCHEMA, [[0.2, 850]])
    modified = copy.deepcopy(support)
    modified["features"][0]["max"] = 100
    with pytest.raises(ValueError, match="support_digest_mismatch"):
        assess_batch(modified, SCHEMA, [[0.2, 850]])


def test_model_artifact_mutation_is_detected(artifact):
    artifact["applicability_range"][0]["max"] = 100
    with pytest.raises(ValueError, match="model_identity_mismatch"):
        build_training_support(artifact, {"C": "wt%", "temperature": "degC"})


def test_descriptor_and_results_do_not_copy_sensitive_artifact_fields(artifact):
    artifact["credentials"] = "synthetic-secret"
    artifact["source_path"] = "C:/private/原始数据.csv"
    artifact["raw_rows"] = [[0.1234567, 891.234567]]
    artifact["model_id"] = _artifact_id(artifact)
    support = build_training_support(artifact, {"C": "wt%", "temperature": "degC"})
    result = assess_batch(support, SCHEMA, [[0.1234567, 891.234567]])
    serialized = json.dumps({"support": support, "result": result}, ensure_ascii=False)
    assert "synthetic-secret" not in serialized
    assert "private/" not in serialized and "原始数据.csv" not in serialized
    assert "0.1234567" not in serialized and "891.234567" not in serialized
    assert "coefficients" not in serialized and "intercept" not in serialized
