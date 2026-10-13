"""Leakage-aware training evaluation and split conformal calibration."""
import copy
import math
from .training import train_linear_regression, train_sklearn_model, predict_model, _split_indices, _metrics, _artifact_id


def train_evaluated(payload, linear=False):
    policy = payload.get("split_policy", {})
    options = payload.get("evaluation", {})
    if not isinstance(options, dict) or not isinstance(options.get("learning_curve", False), bool):
        raise ValueError("evaluation must be an object and learning_curve a boolean")
    raw_train = train_linear_regression if linear else train_sklearn_model
    if not options:
        return raw_train(payload)
    folds = options.get("folds", 0)
    search_trials = options.get("search_trials", 0)
    if isinstance(folds, bool) or not isinstance(folds, int) or folds not in {0, 2, 3, 4, 5, 6, 7, 8, 9, 10}:
        raise ValueError("cross-validation folds must be 0 or 2-10")
    if isinstance(search_trials, bool) or not isinstance(search_trials, int) or not 0 <= search_trials <= 12:
        raise ValueError("search_trials must be between 0 and 12")
    test_fraction = options.get("test_fraction", 0)
    calibration_fraction = options.get("calibration_fraction", 0)
    if any(isinstance(v, bool) or not isinstance(v, (int,float)) or not math.isfinite(v) or not 0 <= v < .5 for v in (test_fraction, calibration_fraction)):
        raise ValueError("test/calibration fractions must be finite in [0, 0.5)")
    if test_fraction + calibration_fraction >= .7:
        raise ValueError("too few development rows after reserving test and calibration")
    size = len(payload["features"])
    if size < 10:
        raise ValueError("scientific evaluation requires at least 10 valid rows")

    def subset(indices):
        p = copy.deepcopy(payload)
        p.pop("evaluation", None)
        for key in ("features", "targets", "groups", "timestamps"):
            if key in p:
                p[key] = [payload[key][i] for i in indices]
        return p

    def reserve(indices, fraction):
        if not fraction:
            return indices, []
        p = subset(indices)
        p["split_policy"] = {**policy, "validation_fraction": fraction}
        split = _split_indices(len(indices), p)
        return [indices[i] for i in split["train_indices"]], [indices[i] for i in split["validation_indices"]]

    development, test = reserve(list(range(size)), test_fraction)
    development, calibration = reserve(development, calibration_fraction / (1-test_fraction))
    if len(development) < 4:
        raise ValueError("at least 4 development rows are required")
    dev = subset(development)
    kind = policy.get("kind", "random")
    seed = policy.get("seed", 0)
    dev["seed"] = seed
    cv_rows = []
    learning_curve = []
    selected_parameters = {}
    algorithm = "linear_regression" if linear else payload.get("algorithm", "random_forest")
    if folds:
        import numpy as np
        from sklearn.model_selection import KFold, GroupKFold, TimeSeriesSplit
        if kind == "group":
            splitter = GroupKFold(n_splits=folds)
            splits = list(splitter.split(dev["features"], groups=dev.get("groups")))
        elif kind == "time":
            ordered = sorted(range(len(development)), key=lambda i: dev.get("timestamps", list(range(len(development))))[i])
            times = dev.get("timestamps", list(range(len(development))))
            splits = []
            for tr, va in TimeSeriesSplit(n_splits=folds).split(ordered):
                validation = [ordered[i] for i in va]
                boundary = min(times[i] for i in validation)
                training = [ordered[i] for i in tr if times[ordered[i]] < boundary]
                if len(training) < 2:
                    raise ValueError("time CV requires distinct training times before validation")
                splits.append((np.asarray(training), np.asarray(validation)))
        else:
            splits = list(KFold(n_splits=folds, shuffle=True, random_state=seed).split(dev["features"]))
        def assess(parameters, train_limit=None):
            rows = []
            for tr, va in splits:
                tr = list(map(int, tr)); va = list(map(int, va))
                if train_limit is not None:
                    if kind == "group":
                        groups = list(dict.fromkeys(dev["groups"][i] for i in tr))
                        keep = set(groups[:max(1, int(len(groups)*train_limit))])
                        tr = [i for i in tr if dev["groups"][i] in keep]
                    else:
                        tr = tr[:max(2, int(len(tr)*train_limit))]
                combined = tr+va
                p = subset([development[i] for i in combined])
                p.update(parameters)
                p["seed"] = seed
                p["split_indices"] = {"train_indices": list(range(len(tr))), "validation_indices": list(range(len(tr),len(combined)))}
                artifact = raw_train(p)
                predictions = predict_model(artifact, [dev["features"][i] for i in va])["predictions"]
                metric = _metrics([dev["targets"][i] for i in va], predictions, list(range(len(va))))
                rows.append({**metric, "train_indices": [development[i] for i in tr], "validation_indices": [development[i] for i in va]})
            return rows
        candidates = [{}]
        if search_trials:
            grid = {"elasticnet": [{"alpha": a, "l1_ratio": r} for a in (.001,.01,.1,1) for r in (.2,.8)],
                "svr": [{"c": c, "epsilon": e} for c in (.1,1,10,100) for e in (.01,.1)],
                "random_forest": [{"n_estimators": n} for n in (32,64,100,200)],
                "lightgbm": [{"n_estimators": n} for n in (32,64,100,200)],
                "xgboost": [{"n_estimators": n} for n in (32,64,100,200)],
                "hist_gradient_boosting": [{"max_iter": n} for n in (50,100,200)]}
            if algorithm not in grid:
                raise ValueError("hyperparameter search supports elasticnet, SVR and tree models")
            candidates += grid[algorithm][:search_trials]
        scored = []
        for parameters in candidates:
            rows = assess(parameters)
            scored.append((sum(row["rmse"] for row in rows)/len(rows), parameters, rows))
        _, selected_parameters, cv_rows = min(scored, key=lambda entry: entry[0])
        if options.get("learning_curve"):
            for fraction in (.4,.7,1.):
                rows = assess(selected_parameters, fraction)
                learning_curve.append({"training_fraction": fraction, "mean_rmse": sum(r["rmse"] for r in rows)/len(rows), "mean_train_count": sum(len(r["train_indices"]) for r in rows)/len(rows)})
    elif search_trials or options.get("learning_curve"):
        raise ValueError("hyperparameter search and learning curves require cross-validation")
    dev.update(selected_parameters)
    artifact = raw_train(dev)
    # Persist global source-row indices for review and reproducibility.
    artifact["split"] = {**artifact["split"], "train_indices": [development[i] for i in artifact["split"]["train_indices"]], "validation_indices": [development[i] for i in artifact["split"]["validation_indices"]], "test_indices": test, "calibration_indices": calibration}
    conformal = None
    if calibration:
        predicted = predict_model(artifact, [payload["features"][i] for i in calibration])["predictions"]
        residuals = sorted(abs(payload["targets"][i]-p) for i,p in zip(calibration,predicted))
        rank = math.ceil((len(residuals)+1)*.95)
        # Never use a capped quantile to falsely claim 95% coverage.
        if rank <= len(residuals):
            conformal = {"method": "split_conformal", "confidence": .95, "radius": residuals[rank-1], "sample_count": len(residuals), "coverage_assumption": "exchangeable calibration and future observations"}
            artifact["conformal"] = conformal
    coverage = None
    extrapolation_count = 0
    if test:
        predicted = predict_model(artifact, [payload["features"][i] for i in test])["predictions"]
        artifact["metrics"]["test"] = _metrics([payload["targets"][i] for i in test], predicted, list(range(len(test))))
        if conformal:
            coverage = sum(abs(payload["targets"][i]-p) <= conformal["radius"] for i,p in zip(test,predicted))/len(test)
        for i in test:
            if any(v is not None and (r["min"] is not None and v < r["min"] or r["max"] is not None and v > r["max"]) for v,r in zip(payload["features"][i],artifact["applicability_range"])):
                extrapolation_count += 1
    artifact["evaluation"] = {"folds": cv_rows, "selected_parameters": selected_parameters, "learning_curve": learning_curve, "test_interval_coverage": coverage, "test_extrapolation_count": extrapolation_count, "calibration_count": len(calibration), "interval_status": "calibrated" if conformal else "unavailable: 95% split conformal needs at least 19 calibration rows", "split_kind": kind}
    artifact["model_id"] = _artifact_id(artifact)
    return artifact
