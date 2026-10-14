import copy
import json

import pytest

from suna_worker.evaluation import train_evaluated
from suna_worker.training import _artifact_id, train_linear_regression
from suna_worker.applicability import assess_model, build_training_support


def persist_support(artifact, units):
    artifact = copy.deepcopy(artifact)
    artifact["applicability_support"] = build_training_support(artifact, units)
    artifact["model_id"] = _artifact_id(artifact)
    return artifact


def test_embedded_support_has_no_circular_model_identity_and_survives_reload():
    artifact = train_linear_regression({
        "features": [[1], [2], [3], [100]], "targets": [2, 4, 6, 200],
        "feature_names": ["time"], "data_version": "a" * 64,
        "split_policy": {"kind": "time", "validation_fraction": .25},
    })
    original = copy.deepcopy(artifact)
    supported = persist_support(artifact, {"time": "min"})
    reloaded = json.loads(json.dumps(supported))
    assert "model_id" not in supported["applicability_support"]
    result = assess_model(reloaded, [[2], [100]])
    assert result["model_id"] == reloaded["model_id"]
    assert [r["state"] for r in result["rows"]] == ["within_domain", "outside_domain"]
    assert artifact == original


def test_calibration_and_test_rows_do_not_widen_support():
    payload = {
        "features": [[i] for i in range(100)], "targets": [i * 2 for i in range(100)],
        "feature_names": ["temperature"], "data_version": "a" * 64,
        "split_policy": {"kind": "time", "validation_fraction": .2},
        "evaluation": {"test_fraction": .2, "calibration_fraction": .2},
    }
    artifact = train_evaluated(payload, linear=True)
    supported = persist_support(artifact, {"temperature": "degC"})
    range_max = supported["applicability_support"]["features"][0]["max"]
    assert range_max == max(payload["features"][i][0] for i in artifact["split"]["train_indices"])
    assert range_max < min(payload["features"][i][0] for i in artifact["split"]["calibration_indices"])
    assert range_max < min(payload["features"][i][0] for i in artifact["split"]["test_indices"])
    assert assess_model(supported, [[99]])["rows"][0]["state"] == "outside_domain"


def test_legacy_ranges_alone_do_not_claim_known_domain():
    artifact = {"model_id": "legacy", "feature_names": ["C"], "applicability_range": [{"min": 0, "max": 1}]}
    assert all(row["state"] == "unknown" for row in assess_model(artifact, [[.5], [2]])["rows"])


def test_mutated_embedded_statistics_are_detected():
    artifact = train_linear_regression({
        "features": [[1], [2], [3]], "targets": [2, 4, 6],
        "feature_names": ["time"], "data_version": "a" * 64,
    })
    supported = persist_support(artifact, {"time": "min"})
    supported["applicability_support"]["features"][0]["max"] = 999
    with pytest.raises(ValueError, match="model_identity_mismatch"):
        assess_model(supported, [[1]])


def test_outside_evidence_cannot_exceed_worker_frame_budget(monkeypatch):
    import suna_worker.applicability as applicability
    artifact = train_linear_regression({
        "features": [[1], [2], [3]], "targets": [2, 4, 6],
        "feature_names": ["time"], "data_version": "a" * 64,
    })
    supported = persist_support(artifact, {"time": "min"})
    monkeypatch.setattr(applicability, "MAX_EVIDENCE_BYTES", 1000)
    with pytest.raises(ValueError, match="applicability_evidence_budget"):
        assess_model(supported, [[999]] * 100)


def test_extreme_integer_input_is_rejected_as_non_finite():
    artifact = train_linear_regression({
        "features": [[1], [2], [3]], "targets": [2, 4, 6],
        "feature_names": ["time"], "data_version": "a" * 64,
    })
    supported = persist_support(artifact, {"time": "min"})
    with pytest.raises(ValueError, match="non_finite_or_non_numeric_input"):
        assess_model(supported, [[10 ** 1000]])
