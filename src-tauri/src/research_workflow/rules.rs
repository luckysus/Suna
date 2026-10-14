//! Reviewed rule snapshots read using the existing material repositories.
use crate::steel::materials::{self, Variable};
use crate::storage::repositories::{material_rules as rules, steel};
use rusqlite::Connection;
use serde_json::{json, Value};

pub fn snapshot(
    c: &Connection,
    ws: &str,
    project: &str,
    dataset: &str,
    raw: &[Value],
) -> Result<Option<rules::RuleSnapshot>, String> {
    let ids: Vec<_> = rules::list(c, ws, project)?
        .into_iter()
        .filter(|v| v.rule.enabled)
        .map(|v| v.rule.id)
        .collect();
    if ids.is_empty() {
        return Ok(None);
    }
    let source = steel::get(c, ws, dataset)?.ok_or("workflow_dataset_missing")?;
    let mut variables = Vec::new();
    for value in raw {
        let Some(field) = value["fieldId"].as_str() else {
            continue;
        };
        let name = value["name"].as_str().ok_or("workflow_variable_invalid")?;
        let unit = value["unit"]
            .as_str()
            .ok_or("workflow_variable_unit_missing")?;
        if let Some(column) = source
            .columns
            .iter()
            .find(|col| col.canonical_field.as_deref() == Some(name) || col.original_name == name)
        {
            if let Some(original) = &column.unit {
                let (scale, shift) = materials::conversion(unit, original)?;
                if scale != 1. || shift != 0. {
                    return Err("workflow_rule_variable_requires_normalized_dataset".into());
                }
            }
        }
        variables.push(Variable {
            name: name.into(),
            field_id: field.into(),
            unit: unit.into(),
            min: value["min"].as_f64().ok_or("workflow_variable_invalid")?,
            max: value["max"].as_f64().ok_or("workflow_variable_invalid")?,
            fixed_value: value["fixedValue"].as_f64(),
        });
    }
    rules::compile(c, ws, project, variables, &ids).map(Some)
}
pub fn summary(snapshot: Option<&rules::RuleSnapshot>) -> Value {
    snapshot.map_or(Value::Null,|s|json!({"schemaVersion":s.schema_version,"ruleSetHash":s.rule_set_hash,"ruleIds":s.rules.iter().map(|v|&v.rule.id).collect::<Vec<_>>(),"ruleCount":s.rules.len(),"reviewedRequired":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_workflow::{dry_run_on_connection, testing::fixture};
    #[test]
    fn real_rule_revision_changes_preview_and_unreviewed_rule_blocks_start() {
        let mut f = fixture();
        f.template.variables[0]["unit"] = json!("wt%");
        f.template.variables[0]["min"] = json!(0.1);
        f.template.variables[0]["max"] = json!(0.6);
        f.template.variables[0]["fieldId"] = json!("chem.c");
        let rule:materials::Rule=serde_json::from_value(json!({"id":"synthetic-carbon-rule","name":"合成成分上限","severity":"hard","enabled":true,"penalty":1,"provenance":{"type":"equipment","sourceRef":"synthetic-equipment","sourceVersion":"1","reviewed":true},"expression":{"kind":"range","fieldId":"chem.c","unit":"wt%","max":0.6}})).unwrap();
        rules::save(&f.connection, "local", &f.input.project_id, rule.clone(), 0).unwrap();
        let first = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
        assert_eq!(first["materialRules"]["ruleCount"], 1);
        let mut changed = rule.clone();
        changed.name = "合成修订规则".into();
        rules::save(
            &f.connection,
            "local",
            &f.input.project_id,
            changed.clone(),
            1,
        )
        .unwrap();
        let next = dry_run_on_connection(&f.connection, "local", &f.template, &f.input).unwrap();
        assert_ne!(first["dryRunSha256"], next["dryRunSha256"]);
        assert_ne!(
            first["materialRules"]["ruleSetHash"],
            next["materialRules"]["ruleSetHash"]
        );
        changed.provenance.reviewed = false;
        rules::save(&f.connection, "local", &f.input.project_id, changed, 2).unwrap();
        assert!(dry_run_on_connection(&f.connection, "local", &f.template, &f.input).is_err());
        assert_eq!(
            f.connection
                .query_row("SELECT COUNT(*) FROM background_tasks", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
