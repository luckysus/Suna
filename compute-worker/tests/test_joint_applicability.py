import copy

import pytest

from suna_worker.joint_optimization import optimize_joint
from suna_worker.optimization import OptimizationError
from suna_worker.training import _artifact_id, train_linear_regression


def model(order, units):
    source = [{"C": c, "temperature": temperature} for c in (.1, .2, .3) for temperature in (700, 710, 720, 730)]
    features = []
    for record in source:
        row = []
        for field, unit in zip(order, units):
            value = record[field]
            if unit == "K":
                value += 273.15
            elif unit == "fraction":
                value /= 100
            row.append(value)
        features.append(row)
    return train_linear_regression({"features": features,
        "targets": [2 * record["C"] + .01 * record["temperature"] for record in source],
        "feature_names": order, "feature_metadata": [{"name": n, "unit": u} for n, u in zip(order, units)],
        "data_version": "a" * 64, "split_indices": {"train_indices": list(range(8)), "validation_indices": list(range(8, 12))}})


def request(c=.2, temperature=710):
    return {"feature_names": ["C", "temperature"], "bounds": [{"min": .1, "max": .4}, {"min": 690, "max": 740}],
        "fixed_values": {"C": c, "temperature": temperature}, "constraints": [], "trials": 10, "seed": 42,
        "objective_specs": [
            {"name": "HV", "source": "prediction_model", "direction": "maximize", "unit": "HV", "artifact": model(["C", "temperature"], ["wt%", "degC"])},
            {"name": "J", "source": "prediction_model", "direction": "maximize", "unit": "J", "artifact": model(["temperature", "C"], ["K", "fraction"]),
             "feature_transforms": {"temperature": {"scale": 1., "shift": 273.15}, "C": {"scale": .01, "shift": 0.}}}]}


def test_independent_orders_and_units_use_the_same_inputs_for_predictions_and_risk():
    result = optimize_joint(request())
    candidate = result["pareto_front"][0]
    assert candidate["objectives"] == pytest.approx([7.5, 7.5], abs=1e-6)
    risks = candidate["model_applicability"]
    assert [entry["risk"]["state"] for entry in risks] == ["within_domain", "within_domain"]
    assert risks[0]["support_sha256"] != risks[1]["support_sha256"]
    assert all(entry["probability"] is None and entry["joint_support"] == "not_assessed" for entry in risks)


def test_ood_annotation_keeps_physically_feasible_candidates_until_explicit_filter():
    payload = request(c=.4)
    all_candidates = optimize_joint(payload)
    assert all_candidates["pareto_front"][0]["feasible"] is True
    assert [entry["risk"]["state"] for entry in all_candidates["pareto_front"][0]["model_applicability"]] == ["outside_domain", "outside_domain"]
    payload["domain_policy"] = "within_domain_only"
    filtered = optimize_joint(payload)
    assert filtered["pareto_front"] == filtered["recommendations"] == []
    assert filtered["domain_filter"]["status"] == "insufficient_domain_candidates"
    assert filtered["domain_filter"]["excluded"] == 1
    assert filtered["domain_filter"]["physical_constraints_changed"] is False


def test_legacy_model_stays_unknown_and_is_never_auto_accepted_as_domain_supported():
    payload = request()
    legacy = payload["objective_specs"][0]["artifact"]
    legacy.pop("applicability_support")
    legacy["model_id"] = _artifact_id(legacy)
    risk = optimize_joint(payload)["pareto_front"][0]["model_applicability"][0]["risk"]
    assert risk["state"] == "unknown"
    payload["domain_policy"] = "within_domain_only"
    assert optimize_joint(payload)["domain_filter"]["after"] == 0


def test_domain_policy_does_not_soften_constraints_or_modify_the_original_request():
    payload = request()
    original = copy.deepcopy(payload)
    optimize_joint(payload)
    assert payload == original
    payload["constraints"] = [{"kind": "equality", "coefficients": {"C": 1}, "value": 100, "tolerance": 1e-8}]
    with pytest.raises(OptimizationError) as failure:
        optimize_joint(payload)
    assert failure.value.code == "optimization_infeasible"


def test_unrecognized_policy_and_nonfinite_transforms_fail_explicitly():
    for policy in ("physical_constraint",):
        payload = request(); payload["domain_policy"] = policy
        with pytest.raises(OptimizationError):
            optimize_joint(payload)
    payload = request()
    payload["objective_specs"][1]["feature_transforms"]["C"]["scale"] = float("inf")
    with pytest.raises(OptimizationError):
        optimize_joint(payload)


def test_target_conversion_does_not_change_model_artifacts_or_range_screening():
    payload = request()
    original = copy.deepcopy(payload["objective_specs"][0]["artifact"])
    payload["objective_specs"][0]["prediction_transform"] = {"scale": .001, "shift": 0.}
    payload["objective_specs"][0]["unit"] = "GPa"
    candidate = optimize_joint(payload)["pareto_front"][0]
    assert candidate["objectives"] == pytest.approx([.0075, 7.5], abs=1e-6)
    assert all(r["risk"]["state"] == "within_domain" for r in candidate["model_applicability"])
    assert payload["objective_specs"][0]["artifact"] == original


def test_direction_reversal_and_nsga_front_are_reproducible_with_domain_evidence():
    payload = request()
    payload["fixed_values"] = {"temperature": 710}
    payload["algorithm"] = "nsga2"
    payload["trials"] = 20
    payload["objective_specs"][1]["direction"] = "minimize"
    first, second = optimize_joint(payload), optimize_joint(payload)
    assert first["pareto_front"] == second["pareto_front"]
    assert first["directions"] == ["maximize", "minimize"]
    assert len(first["pareto_front"]) > 1
    for candidate in first["pareto_front"]:
        assert len(candidate["model_applicability"]) == 2
        assert candidate["objectives"][0] == pytest.approx(candidate["objectives"][1], abs=1e-6)


def test_cancellation_remains_effective_and_formula_has_no_model_support_claim():
    with pytest.raises(OptimizationError) as failure:
        optimize_joint(request(), is_cancelled=lambda: True)
    assert failure.value.code == "optimization_cancelled"
    payload = request()
    payload["objective_specs"] = [{"name": "cost", "source": "formula", "direction": "minimize", "coefficients": {"C": 1}}]
    candidate = optimize_joint(payload)["pareto_front"][0]
    assert candidate["model_applicability"] == []
    payload["domain_policy"] = "within_domain_only"
    assert optimize_joint(payload)["domain_filter"]["status"] == "insufficient_domain_candidates"


def test_output_transform_refuses_nonfinite_zero_scale_and_overflow():
    for transform in ({"scale": 0, "shift": 0}, {"scale": float("inf"), "shift": 0}, {"scale": 1e308, "shift": 0}):
        payload = request(); payload["objective_specs"][0]["prediction_transform"] = transform
        with pytest.raises(OptimizationError):
            optimize_joint(payload)
