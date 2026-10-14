"""Joint performance optimization; model inputs are mapped by name, never ordinal.

Formula objectives are explicit affine cost functions, not executable expressions.
Pareto dominance uses each objective's direction. Weights are retained metadata;
NSGA-II never collapses competing performances into a weighted scalar.
"""
from collections.abc import Mapping
import itertools
import math
import json
from .applicability import assess_model

from .optimization import (
    OptimizationError, _model, _predict, _bounds, _fixed_values, _constraints,
    _direction, _finite, _trials, _seed, _check_cancel, _project_equalities,
    _within_bounds, _constraint_violation, _non_dominated_indices,
    MAX_FRONT_POINTS, RECOMMENDATION_LIMIT,
)


def optimize_joint(payload, report=None, is_cancelled=None):
    names = payload.get("feature_names")
    if not isinstance(names, list) or not names or any(not isinstance(n, str) or not n.strip() for n in names) or len(set(names)) != len(names):
        raise OptimizationError("invalid_payload", "unique feature_names are required")
    bounds = _bounds(payload.get("bounds"), len(names))
    fixed = _fixed_values(payload.get("fixed_values"), names, bounds)
    # Zero-width bounds must also remain fixed during equality projection.
    fixed.update({n: low for n, (low, high) in zip(names, bounds) if low == high})
    constraints = _constraints(payload.get("constraints"), names)
    specs = payload.get("objective_specs")
    if not isinstance(specs, list) or not 1 <= len(specs) <= 4:
        raise OptimizationError("invalid_payload", "between 1 and 4 objectives are required")
    evaluators, directions, metadata, model_inputs = [], [], [], []
    domain_policy = payload.get("domain_policy", "annotate_all")
    if domain_policy not in {"annotate_all", "within_domain_only"}:
        raise OptimizationError("invalid_payload", "unknown domain policy")
    for spec in specs:
        if not isinstance(spec, Mapping) or not isinstance(spec.get("name"), str) or not spec["name"].strip():
            raise OptimizationError("invalid_payload", "objective name is required")
        if spec["name"] in [s["name"] for s in metadata]:
            raise OptimizationError("invalid_payload", "objective names must be unique")
        direction = _direction(spec.get("direction"))
        source = spec.get("source")
        if source == "prediction_model":
            model = _model(spec.get("artifact"))
            if len(set(model["feature_names"])) != len(model["feature_names"]) or not set(model["feature_names"]) <= set(names):
                raise OptimizationError("invalid_payload", "model features are missing from optimization variables")
            transforms = spec.get("feature_transforms", {})
            if not isinstance(transforms, Mapping) or set(transforms) - set(model["feature_names"]):
                raise OptimizationError("invalid_payload", "invalid model input transformation schema")
            plan = {}
            for feature in model["feature_names"]:
                transform = transforms.get(feature, {"scale": 1.0, "shift": 0.0})
                if not isinstance(transform, Mapping) or set(transform) != {"scale", "shift"}:
                    raise OptimizationError("invalid_payload", "invalid model input transformation")
                scale, shift = _finite(transform["scale"], "input scale"), _finite(transform["shift"], "input shift")
                if scale <= 0:
                    raise OptimizationError("invalid_payload", "input scale must be positive")
                plan[feature] = (scale, shift)
            def model_values(values, plan=plan):
                result = {key: scale * values[key] + shift for key, (scale, shift) in plan.items()}
                if any(not math.isfinite(v) for v in result.values()):
                    raise OptimizationError("invalid_payload", "model unit transformation overflow")
                return result
            output = spec.get("prediction_transform", {"scale": 1.0, "shift": 0.0})
            if not isinstance(output, Mapping) or set(output) != {"scale", "shift"}:
                raise OptimizationError("invalid_payload", "invalid prediction unit transformation")
            output_scale, output_shift = _finite(output["scale"], "output scale"), _finite(output["shift"], "output shift")
            if output_scale <= 0:
                raise OptimizationError("invalid_payload", "output scale must be positive")
            evaluators.append(lambda values, m=model, convert=model_values, s=output_scale, b=output_shift: s * _predict(m, m["feature_names"], convert(values)) + b)
            # _model is a numeric prediction representation. Domain evidence
            # must use the original frozen artifact with its verified digest.
            model_inputs.append((len(metadata), spec["artifact"], model_values, spec.get("model_sha256")))
        elif source == "formula":
            coefficients = spec.get("coefficients")
            if not isinstance(coefficients, Mapping) or not coefficients or not set(coefficients) <= set(names):
                raise OptimizationError("invalid_payload", "formula coefficients must reference optimization variables")
            coefficients = {n: _finite(v, "formula coefficient") for n, v in coefficients.items()}
            intercept = _finite(spec.get("intercept", 0), "formula intercept")
            evaluators.append(lambda values, c=coefficients, b=intercept: b + sum(v * values[n] for n, v in c.items()))
        else:
            raise OptimizationError("invalid_payload", "objective source must be prediction_model or formula")
        weight = _finite(spec.get("weight", 1), "objective weight")
        if weight <= 0:
            raise OptimizationError("invalid_payload", "objective weight must be positive")
        directions.append(direction)
        metadata.append({k: spec.get(k, "") for k in ("name", "source", "unit", "model_id")})
        metadata[-1].update(direction=direction, weight=weight)
        metadata[-1].pop("model_id", None)
        if source == "prediction_model":
            metadata[-1]["modelId"] = spec.get("model_id", spec.get("modelId", model["model_id"]))
            metadata[-1]["unit_status"] = spec.get("unit_status", "unknown_training_unit")
        else:
            metadata[-1].update(coefficients=coefficients, intercept=intercept)
    trials, seed = _trials(payload.get("trials")), _seed(payload.get("seed"))
    algorithm = payload.get("algorithm") or ("nsga2" if len(specs) > 1 else "tpe")
    if algorithm not in {"nsga2", "tpe", "grid"} or (len(specs) > 1 and algorithm == "tpe"):
        raise OptimizationError("invalid_payload", "joint objectives support NSGA-II/grid; TPE requires one objective")
    candidates = []
    def evaluate(raw):
        _check_cancel(is_cancelled)
        values, failed = _project_equalities(raw, constraints, fixed)
        violations = [_constraint_violation(c, values) for c in constraints]
        feasible = not failed and _within_bounds(values, names, bounds) and not any(v > 0 for v in violations)
        results = [float(e(values)) for e in evaluators]
        if any(not math.isfinite(v) for v in results):
            raise OptimizationError("invalid_prediction", "objective values must be finite")
        signed = [v if d == "minimize" else -v for v, d in zip(results, directions)]
        if feasible:
            candidates.append({"values": values, "objectives": results, "prediction": results[0], "feasible": True,
                "constraint_residuals": {str(i): sum(v * values[n] for n, v in c["coefficients"].items()) - c["value"] for i, c in enumerate(constraints)}})
        if report:
            report("searching", min(90, 10 + int(80 * (evaluate.count + 1) / trials)))
        evaluate.count += 1
        return signed, violations + ([1.0] if not feasible and not any(violations) else [0.0])
    evaluate.count = 0
    free = [n for n in names if n not in fixed]
    if not free:
        evaluate(dict(fixed))
    elif algorithm == "grid":
        points = max(2, int(trials ** (1 / len(free))))
        if points ** len(free) > trials:
            raise OptimizationError("invalid_payload", "grid needs at least 2^free_variables evaluations")
        axes = []
        for n in free:
            low, high = bounds[names.index(n)]
            axes.append([low + (high-low)*i/(points-1) for i in range(points)])
        for row in itertools.product(*axes):
            evaluate({**fixed, **dict(zip(free, row))})
    else:
        import optuna
        optuna.logging.set_verbosity(optuna.logging.WARNING)
        def objective(trial):
            raw = {**fixed, **{n: trial.suggest_float(n, *bounds[names.index(n)]) for n in free}}
            signed, violations = evaluate(raw)
            trial.set_user_attr("violations", violations)
            return signed if len(specs) > 1 else signed[0]
        options = {"seed": seed, "constraints_func": lambda t: t.user_attrs["violations"]}
        sampler = optuna.samplers.NSGAIISampler(**options) if algorithm == "nsga2" else optuna.samplers.TPESampler(**options)
        study = optuna.create_study(directions=["minimize"] * len(specs), sampler=sampler)
        study.optimize(objective, n_trials=trials)
    if not candidates:
        raise OptimizationError("optimization_infeasible", "no candidate satisfies bounds, fixed variables and constraints")
    signed_rows = [[v if d == "minimize" else -v for v, d in zip(c["objectives"], directions)] for c in candidates]
    front = [candidates[i] for i in _non_dominated_indices(signed_rows)]
    front = front[:MAX_FRONT_POINTS]
    for candidate in front:
        candidate["model_applicability"] = []
    for objective_index, model, convert, model_sha in model_inputs:
        _check_cancel(is_cancelled)
        converted = [convert(candidate["values"]) for candidate in front]
        rows = [[values[n] for n in model["feature_names"]] for values in converted]
        evidence = assess_model(model, rows)
        for candidate_index, (candidate, risk) in enumerate(zip(front, evidence["rows"])):
            candidate["model_applicability"].append({"objective_index": objective_index,
                "model_id": model["model_id"], "model_sha256": model_sha, "schema_version": evidence["schema_version"],
                "method": evidence["method"], "support_sha256": evidence["support_sha256"],
                "dataset_sha256": model.get("applicability_support", {}).get("dataset_sha256"),
                "candidate_index": candidate_index, "risk": {**risk, "row_index": 0},
                "probability": None, "physical_feasibility": "not_assessed", "joint_support": "not_assessed",
                "interval_status": "requires_exchangeability_assumption" if model.get("conformal", {}).get("method") == "split_conformal" else "unknown"})
        if len(json.dumps(front, allow_nan=False, separators=(",", ":")).encode()) > 2 * 1024 * 1024:
            raise OptimizationError("invalid_payload", "joint domain evidence exceeds result budget")
    returned_front_size = len(front)
    if domain_policy == "within_domain_only":
        front = [candidate for candidate in front if candidate["model_applicability"] and
                 all(model["risk"]["state"] == "within_domain" for model in candidate["model_applicability"])]
    domain_filter = {"policy": domain_policy, "scope": "returned_pareto_front", "before": returned_front_size,
        "after": len(front), "excluded": returned_front_size-len(front),
        "status": "insufficient_domain_candidates" if not front else "completed",
        "joint_probability": None, "physical_constraints_changed": False}
    if report:
        report("validated", 99)
    result = {"method": algorithm, "direction": directions[0], "directions": directions,
        "objectives": [s["name"] for s in specs], "objective_specs": metadata, "feature_names": names,
        "model_id": "joint", "model_type": "joint", "trials_completed": evaluate.count,
        "deterministic_seed": seed, "recommendations": front[:RECOMMENDATION_LIMIT], "pareto_front": front,
        "domain_filter": domain_filter}
    if len(json.dumps(result, allow_nan=False, separators=(",", ":")).encode()) > 2 * 1024 * 1024:
        raise OptimizationError("invalid_payload", "joint domain result exceeds result budget")
    return result
