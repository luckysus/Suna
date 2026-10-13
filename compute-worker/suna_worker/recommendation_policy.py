"""Budgeted constrained ranking around the existing design GP; no new model backend."""
from collections.abc import Mapping
import math
from .design import DesignError


def candidate_pool(variables, raw_candidates, existing_normalized, policy):
    import numpy as np
    from .optimization import _constraints, _project_equalities, _within_bounds, _constraint_violation
    from .design import _denormalise_point
    if not isinstance(policy, Mapping):
        raise DesignError('invalid_payload', 'recommendation_policy must be an object')
    distance = policy.get('min_distance', 0.01)
    if isinstance(distance, bool) or not isinstance(distance, (int, float)) or not math.isfinite(distance) or not 0 <= distance <= 0.5:
        raise DesignError('invalid_payload', 'min_distance must be in [0,0.5]')
    names = [v['name'] for v in variables]
    bounds = [(v['low'], v['high']) for v in variables]
    try:
        constraints = _constraints(policy.get('constraints', []), names)
    except ValueError as error:
        raise DesignError('invalid_constraint', str(error)) from error
    excluded = policy.get('exclude', [])
    if not isinstance(excluded, list) or len(excluded) > 5000:
        raise DesignError('invalid_payload', 'exclude must contain at most 5000 points')
    excluded_matrix = list(existing_normalized)
    for values in excluded:
        if not isinstance(values, Mapping) or set(values) != set(names) or any(isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v) for v in values.values()):
            raise DesignError('invalid_payload', 'excluded points must use the same finite feature schema')
        excluded_matrix.append([(values[v['name']] - v['low']) / (v['high'] - v['low']) for v in variables])
    excluded_matrix = np.asarray(excluded_matrix, dtype=float)
    pool, seen = [], set()
    for point in raw_candidates:
        values = _denormalise_point(variables, point)
        values, _ = _project_equalities(values, constraints, {})
        if not _within_bounds(values, names, bounds) or any(_constraint_violation(c, values) > 1e-8 for c in constraints):
            continue
        normalized = np.asarray([(values[v['name']] - v['low']) / (v['high'] - v['low']) for v in variables])
        signature = tuple(np.round(normalized, 12))
        if signature in seen or len(excluded_matrix) and np.min(np.linalg.norm(excluded_matrix-normalized, axis=1)) <= max(distance, 1e-10):
            continue
        seen.add(signature)
        pool.append(normalized)
    if not pool:
        raise DesignError('no_feasible_candidates', 'constraints and duplicate exclusion leave no candidate')
    return np.asarray(pool), distance


def rank_candidates(mean, std, candidates, count, direction, policy, default_score):
    import numpy as np
    strategy = policy.get('strategy')
    if strategy not in {'exploitation', 'exploration', 'balanced'}:
        raise DesignError('invalid_payload', 'strategy must be exploitation, exploration or balanced')
    weight = policy.get('exploration_weight', 0.5)
    if isinstance(weight, bool) or not isinstance(weight, (int,float)) or not math.isfinite(weight) or not 0 <= weight <= 1:
        raise DesignError('invalid_payload', 'exploration_weight must be in [0,1]')
    def scale(values):
        return (values - np.min(values)) / max(float(np.ptp(values)), 1e-12)
    performance = mean if direction == 'maximize' else -mean
    if strategy == 'exploitation':
        scores = scale(performance)
    elif strategy == 'exploration':
        scores = scale(std)
    else:
        scores = (1-weight)*scale(performance) + weight*scale(std)
    distance = policy.get('min_distance', 0.01)
    order = np.argsort(-scores, kind='stable')
    selected = []
    for index in order:
        if all(np.linalg.norm(candidates[index]-candidates[prior]) > max(distance,1e-10) for prior in selected):
            selected.append(int(index))
            if len(selected) == count:
                break
    if len(selected) < count:
        raise DesignError('insufficient_candidate_budget', 'available distinct feasible candidates are fewer than requested budget')
    return selected, scores
