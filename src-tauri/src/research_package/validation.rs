//! Typed reference metadata and bounded DAG validation; no executable assets.
use crate::research_package::{identity, valid_sha};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub const RELATIONS: [&str; 8] = [
    "uses_dataset",
    "uses_model",
    "generated_by_run",
    "based_on_evidence",
    "recommends_experiment",
    "measured_against",
    "supersedes",
    "included_in_report",
];
fn allowed(v: &Value, keys: &[&str]) -> bool {
    v.as_object()
        .is_some_and(|m| m.keys().all(|k| keys.contains(&k.as_str())))
}
fn text(v: &Value, budget: usize) -> bool {
    v.as_str().is_some_and(|s| {
        !s.trim().is_empty()
            && s.chars().count() <= budget
            && !s.chars().any(char::is_control)
            && !s.trim().starts_with(['/', '\\'])
            && !s.contains(':')
    })
}
pub fn record(v: &Value) -> Result<String, String> {
    if !allowed(
        v,
        &[
            "id",
            "kind",
            "name",
            "version",
            "review_status",
            "source_sha256",
            "source_task_id",
            "source_artifact_sha256",
            "model_sha256",
            "references",
            "origin_kind",
            "origin_schema",
            "units",
            "algorithm",
            "artifact_version",
        ],
    ) || ![
        "project", "artifact", "model", "dataset", "report", "evidence",
    ]
    .contains(&v["kind"].as_str().unwrap_or(""))
    {
        return Err("package_record_schema_invalid".into());
    }
    let id = identity(v["id"].as_str().ok_or("package_resource_id_invalid")?)?;
    if v.get("artifact_version").is_some_and(|v| {
        !v.as_str().is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".+_-".contains(&b))
        })
    }) {
        return Err("package_artifact_version_invalid".into());
    }
    if v.get("algorithm").is_some_and(|a| {
        !a.as_str().is_some_and(|a| {
            [
                "linear_regression",
                "elasticnet",
                "random_forest",
                "hist_gradient_boosting",
                "lightgbm",
                "xgboost",
                "svr",
                "mlp",
                "transformer",
            ]
            .contains(&a)
        })
    }) {
        return Err("package_algorithm_invalid".into());
    }
    for key in ["name", "review_status", "origin_kind", "origin_schema"] {
        if v.get(key).is_some_and(|a| !text(a, 128)) {
            return Err("package_record_schema_invalid".into());
        }
    }
    for key in ["source_sha256", "source_artifact_sha256", "model_sha256"] {
        if v.get(key)
            .is_some_and(|a| !a.is_null() && a.as_str().is_none_or(|s| !valid_sha(s)))
        {
            return Err("package_record_digest_invalid".into());
        }
    }
    if v.get("source_task_id")
        .is_some_and(|a| !a.is_null() && a.as_str().is_none_or(|s| identity(s).is_err()))
    {
        return Err("package_source_task_invalid".into());
    }
    if v.get("version")
        .is_some_and(|a| !a.is_null() && a.as_u64().is_none_or(|n| n == 0))
    {
        return Err("package_record_version_invalid".into());
    }
    if let Some(units) = v.get("units") {
        let units = units
            .as_array()
            .filter(|a| a.len() <= 256)
            .ok_or("package_unit_schema_invalid")?;
        if units.iter().any(|u| {
            !allowed(u, &["name", "unit"])
                || u.as_object().is_none_or(|o| o.len() != 2)
                || !text(&u["name"], 128)
                || (!u["unit"].is_null() && !text(&u["unit"], 64))
        }) {
            return Err("package_unit_schema_invalid".into());
        }
    }
    if let Some(refs) = v.get("references") {
        let refs = refs
            .as_array()
            .filter(|a| a.len() <= 2048)
            .ok_or("package_reference_invalid")?;
        if refs
            .iter()
            .any(|r| r.as_str().is_none_or(|r| identity(r).is_err()))
        {
            return Err("package_reference_invalid".into());
        }
    }
    Ok(id)
}
pub fn lineage(
    objects: &[(String, Value, String)],
    dependencies: &[Value],
    edges: &[Value],
) -> Result<(), String> {
    if dependencies.len() > 2048 || edges.len() > 10000 {
        return Err("package_lineage_budget".into());
    }
    let ids: BTreeSet<_> = objects.iter().map(|(id, _, _)| id.clone()).collect();
    if ids.len() != objects.len() {
        return Err("duplicate_package_resource_id".into());
    }
    let mut external = BTreeSet::new();
    for dep in dependencies {
        if !allowed(
            dep,
            &["resource_id", "status", "kind", "expected_sha256", "unit"],
        ) || dep["status"] != "missing"
            || !text(&dep["kind"], 128)
        {
            return Err("package_dependency_invalid".into());
        }
        let id = identity(
            dep["resource_id"]
                .as_str()
                .ok_or("package_dependency_invalid")?,
        )?;
        if !external.insert(id) {
            return Err("duplicate_package_dependency".into());
        }
        if dep
            .get("expected_sha256")
            .is_some_and(|v| !v.is_null() && v.as_str().is_none_or(|s| !valid_sha(s)))
        {
            return Err("package_dependency_invalid".into());
        }
        if dep
            .get("unit")
            .is_some_and(|v| !v.is_null() && !text(v, 64))
        {
            return Err("package_dependency_invalid".into());
        }
    }
    let mut adjacency: BTreeMap<String, BTreeSet<String>> =
        ids.iter().map(|id| (id.clone(), BTreeSet::new())).collect();
    let mut degree: BTreeMap<String, usize> = ids.iter().map(|id| (id.clone(), 0)).collect();
    let mut connect = |from: &str, to: &str| -> Result<(), String> {
        if from == to {
            return Err("package_lineage_cycle".into());
        }
        if !ids.contains(from) || !ids.contains(to) {
            return Err("package_lineage_missing_reference".into());
        }
        if adjacency.get_mut(from).unwrap().insert(to.into()) {
            *degree.get_mut(to).unwrap() += 1;
        }
        Ok(())
    };
    for (id, body, _) in objects {
        record(body)?;
        if let Some(refs) = body["references"].as_array() {
            for r in refs {
                let source = identity(r.as_str().ok_or("package_reference_invalid")?)?;
                if ids.contains(&source) {
                    connect(&source, id)?;
                } else if !external.contains(&source) {
                    return Err("package_lineage_missing_reference".into());
                }
            }
        }
    }
    let mut seen = BTreeSet::new();
    for edge in edges {
        if !allowed(edge, &["from_id", "to_id", "relation"])
            || edge.as_object().is_none_or(|m| m.len() != 3)
        {
            return Err("package_edge_invalid".into());
        }
        let from = identity(edge["from_id"].as_str().ok_or("package_edge_invalid")?)?;
        let to = identity(edge["to_id"].as_str().ok_or("package_edge_invalid")?)?;
        let relation = edge["relation"]
            .as_str()
            .filter(|r| RELATIONS.contains(r))
            .ok_or("package_edge_invalid")?;
        if !seen.insert((from.clone(), to.clone(), relation.to_string())) {
            return Err("duplicate_package_edge".into());
        }
        connect(&from, &to)?;
    }
    let mut ready: VecDeque<_> = degree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for target in &adjacency[&id] {
            let n = degree.get_mut(target).unwrap();
            *n -= 1;
            if *n == 0 {
                ready.push_back(target.clone());
            }
        }
    }
    if visited != ids.len() {
        return Err("package_lineage_cycle".into());
    }
    Ok(())
}
