"""Deterministic training-range screening for persisted Worker model artifacts.

Reuse the existing trainer's train-only ranges. Unit conversion remains the
existing Rust materials engine's responsibility; callers pass canonical units.
Range membership supplies neither a probability nor joint/physical feasibility.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
from collections.abc import Mapping, Sequence
from typing import Any

from suna_worker.training import (
    ARTIFACT_VERSION,
    SKLEARN_ARTIFACT_VERSION,
    TRANSFORMER_ARTIFACT_VERSION,
    _artifact_id,
)

VERSION = "applicability-support.v1"
MAX_BATCH_ROWS = 10_000
MAX_FEATURES = 128
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024


def _finite(value: Any) -> bool:
    try:
        return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)
    except OverflowError:
        return False


def _unit(value: Any) -> str | None:
    if value is None or value == "":
        return None
    if not isinstance(value, str) or len(value) > 64 or value != value.strip():
        raise ValueError("invalid_canonical_unit")
    return value


def _names(values: Any) -> list[str]:
    if not isinstance(values, list) or not 1 <= len(values) <= MAX_FEATURES:
        raise ValueError("invalid_feature_schema")
    if any(not isinstance(name, str) or not name.strip() or len(name) > 128 for name in values):
        raise ValueError("invalid_feature_name")
    if len(set(values)) != len(values):
        raise ValueError("duplicate_feature_name")
    return list(values)


def _digest(support: Mapping[str, Any]) -> str:
    fields = {key: value for key, value in support.items() if key != "support_sha256"}
    encoded = json.dumps(fields, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def build_training_support(
    artifact: Mapping[str, Any], canonical_units: Mapping[str, str | None]
) -> dict[str, Any]:
    """Build a metadata-only descriptor from a trusted, registered model artifact.

    The digest detects accidental modification; it does not authenticate a model.
    Registry SHA verification must precede this function during future integration.
    No raw training row, pickle, path, credential, coefficient or prediction is copied.
    """
    names = _names(artifact.get("feature_names"))
    if set(canonical_units) - set(names):
        raise ValueError("unexpected_unit_feature")
    unknown: list[str] = []
    known_versions = (ARTIFACT_VERSION, SKLEARN_ARTIFACT_VERSION, TRANSFORMER_ARTIFACT_VERSION)
    if artifact.get("artifact_version") not in known_versions:
        unknown.append("unsupported_artifact_contract")
    model_id = artifact.get("model_id")
    if model_id is None:
        unknown.append("missing_model_identity")
    elif not isinstance(model_id, str) or model_id != _artifact_id(artifact):
        raise ValueError("model_identity_mismatch")
    data_sha = artifact.get("data_version")
    if not isinstance(data_sha, str) or re.fullmatch("[a-f0-9]{64}", data_sha) is None:
        data_sha = None
        unknown.append("missing_dataset_sha256")
    preprocessing = artifact.get("preprocessing")
    train_only = isinstance(preprocessing, Mapping) and preprocessing.get("fit_scope") == "train_only"
    if not train_only:
        unknown.append("unverified_training_scope")
    ranges = artifact.get("applicability_range")
    if ranges is None or ranges == []:
        ranges = [None] * len(names)
        unknown.append("missing_training_support")
    if not isinstance(ranges, list) or len(ranges) != len(names):
        raise ValueError("training_support_dimension_mismatch")
    means = preprocessing.get("means", []) if isinstance(preprocessing, Mapping) else []
    features = []
    for index, (name, limits) in enumerate(zip(names, ranges)):
        lower = limits.get("min") if isinstance(limits, Mapping) else None
        upper = limits.get("max") if isinstance(limits, Mapping) else None
        if lower is not None or upper is not None:
            if not _finite(lower) or not _finite(upper) or lower > upper:
                raise ValueError("invalid_training_limits")
        mean = means[index] if isinstance(means, list) and index < len(means) else None
        imputation = bool(train_only and preprocessing.get("imputation") == "train_mean" and _finite(mean))
        features.append({
            "name": name, "unit": _unit(canonical_units.get(name)),
            "min": lower, "max": upper,
            "imputation_mean": mean if imputation else None,
        })
    support: dict[str, Any] = {
        "schema_version": VERSION, "method": "marginal_training_ranges",
        "fit_scope": "train_only" if train_only else "unknown",
        "dataset_sha256": data_sha,
        "features": features, "unknown_reasons": unknown,
    }
    support["support_sha256"] = _digest(support)
    return support


def assess_batch(
    support: Mapping[str, Any], input_schema: list[Mapping[str, Any]],
    rows: Sequence[Sequence[Any]],
) -> dict[str, Any]:
    """Screen rows by feature name; return evidence without input values.

    A known range violation is reported even if another feature is unknown.
    Units must match exactly after conversion by Suna's existing unit engine.
    No caller policy removes an OOD candidate or turns it into a physical constraint.
    """
    if support.get("schema_version") != VERSION:
        raise ValueError("unsupported_support_version")
    if support.get("support_sha256") != _digest(support):
        raise ValueError("support_digest_mismatch")
    features = support.get("features")
    if not isinstance(features, list):
        raise ValueError("invalid_training_support")
    trained_names = _names([feature["name"] for feature in features])
    if not isinstance(input_schema, list) or not all(isinstance(column, Mapping) for column in input_schema):
        raise ValueError("invalid_input_schema")
    input_names = _names([column.get("name") for column in input_schema])
    if set(input_names) - set(trained_names):
        raise ValueError("unexpected_input_feature")
    units = [_unit(column.get("unit")) for column in input_schema]
    positions = {name: index for index, name in enumerate(input_names)}
    if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence) or not 1 <= len(rows) <= MAX_BATCH_ROWS:
        raise ValueError("invalid_batch_size")
    results = []
    evidence_bytes = 0
    for row_index, row in enumerate(rows):
        if isinstance(row, (str, bytes)) or not isinstance(row, Sequence) or len(row) != len(input_names):
            raise ValueError("input_dimension_mismatch")
        if any(value is not None and not _finite(value) for value in row):
            raise ValueError("non_finite_or_non_numeric_input")
        reasons = [{"state": "unknown", "code": code} for code in support["unknown_reasons"]]
        for feature in features:
            name = feature["name"]
            position = positions.get(name)
            def reason(state: str, code: str, **evidence: Any) -> None:
                reasons.append({"state": state, "code": code, "feature": name, **evidence})
            if position is None:
                reason("unknown", "missing_input_feature")
                continue
            if feature["unit"] is None or units[position] is None:
                reason("unknown", "missing_unit")
                continue
            if feature["unit"] != units[position]:
                reason("unknown", "unit_conversion_required")
                continue
            lower, upper = feature["min"], feature["max"]
            if lower is None or upper is None:
                reason("unknown", "missing_training_limits")
                continue
            value = row[position]
            if value is None:
                value = feature["imputation_mean"]
                if value is None:
                    reason("unknown", "missing_input_value")
                    continue
                reason("warning", "imputed_with_training_mean")
            if value < lower or value > upper:
                reason("outside_domain", "outside_training_range", threshold={
                    "min": lower, "max": upper, "unit": feature["unit"],
                    "source": "training_split", "support_sha256": support["support_sha256"],
                })
        states = {reason["state"] for reason in reasons}
        state = next((item for item in ("outside_domain", "unknown", "warning") if item in states), "within_domain")
        row_result = {"row_index": row_index, "state": state, "reasons": reasons}
        evidence_bytes += len(json.dumps(row_result, ensure_ascii=False, allow_nan=False, separators=(",", ":")).encode("utf-8"))
        if evidence_bytes > MAX_EVIDENCE_BYTES:
            raise ValueError("applicability_evidence_budget: submit smaller batches")
        results.append(row_result)
    return {
        "schema_version": VERSION, "method": "marginal_training_ranges",
        "support_sha256": support["support_sha256"], "model_id": support.get("model_id"),
        "probability": None, "physical_feasibility": "not_assessed",
        "joint_support": "not_assessed", "rows": results,
    }


def assess_model(artifact: Mapping[str, Any], rows: Sequence[Sequence[Any]]) -> dict[str, Any]:
    """Use a persisted descriptor bound by the complete model artifact identity.

    The descriptor omits model_id to avoid a self-referential content hash when
    stored inside the artifact. The outer immutable artifact binds its contents.
    """
    if isinstance(rows, (str, bytes)) or not isinstance(rows, Sequence) or not 1 <= len(rows) <= MAX_BATCH_ROWS:
        raise ValueError("invalid_batch_size")
    support = artifact.get("applicability_support")
    if not isinstance(support, Mapping):
        return {
            "schema_version": VERSION, "method": "marginal_training_ranges",
            "model_id": artifact.get("model_id"), "support_sha256": None,
            "probability": None, "physical_feasibility": "not_assessed",
            "joint_support": "not_assessed", "rows": [
                {"row_index": i, "state": "unknown", "reasons": [
                    {"state": "unknown", "code": "legacy_model_without_support"}
                ]} for i in range(len(rows))
            ],
        }
    if artifact.get("model_id") != _artifact_id(artifact):
        raise ValueError("model_identity_mismatch")
    if [f["name"] for f in support["features"]] != artifact.get("feature_names"):
        raise ValueError("support_feature_binding_mismatch")
    schema = [{"name": f["name"], "unit": f["unit"]} for f in support["features"]]
    assessment = assess_batch(support, schema, rows)
    assessment["model_id"] = artifact["model_id"]
    return assessment
