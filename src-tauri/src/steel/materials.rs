//! Typed materials registry and unit conversion. Never executes user expressions.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub id: String,
    pub aliases: Vec<String>,
    pub role: String,
    pub canonical_unit: String,
    pub value_type: String,
    #[serde(default)]
    pub nullable: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub schema_version: String,
    pub material_system: String,
    pub fields: Vec<Field>,
    #[serde(default)]
    pub templates: Vec<Value>,
}
pub fn base() -> Registry {
    serde_json::from_str::<Value>(include_str!(
        "../../../domain-packs/steel/assets/data-mappings.json"
    ))
    .and_then(|v| serde_json::from_value(v["materials_schema"].clone()))
    .expect("bundled verified materials schema")
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Match {
    pub field_id: String,
    pub confidence: f64,
    pub reason: String,
}
pub fn suggest(registry: &Registry, name: &str) -> Vec<Match> {
    let n = name.trim().to_lowercase();
    let found: Vec<&Field> = registry
        .fields
        .iter()
        .filter(|f| f.id.to_lowercase() == n || f.aliases.iter().any(|a| a.to_lowercase() == n))
        .collect();
    let ambiguous = found.len() > 1;
    found
        .into_iter()
        .map(|f| Match {
            field_id: f.id.clone(),
            confidence: if ambiguous {
                0.4
            } else if f.id.to_lowercase() == n {
                1.0
            } else {
                0.9
            },
            reason: if ambiguous {
                "ambiguous_alias_requires_user_choice"
            } else {
                "exact_id_or_alias"
            }
            .into(),
        })
        .collect()
}
fn canonical(unit: &str) -> &str {
    match unit.trim() {
        "wt.%" | "wt.% " | "percent_mass" => "wt%",
        "mass_fraction" => "fraction",
        "°C" | "C" | "celsius" => "degC",
        "kelvin" => "K",
        "minutes" => "min",
        "hours" => "h",
        "μm" | "µm" => "um",
        other => other,
    }
}
/// y = scale*x + shift; dimension/measurement-scale compatibility is explicit.
pub fn conversion(from: &str, to: &str) -> Result<(f64, f64), String> {
    let f = canonical(from);
    let t = canonical(to);
    let known = [
        "wt%", "fraction", "%", "degC", "K", "min", "h", "s", "K/s", "MPa", "Pa", "GPa", "HV",
        "HB", "HRC", "J", "um", "mm", "m", "1",
    ];
    if !known.contains(&f) || !known.contains(&t) {
        return Err(format!("unknown unit {from} or {to}"));
    }
    if f == t {
        return Ok((1., 0.));
    }
    match (f, t) {
        ("fraction", "wt%") => Ok((100., 0.)),
        ("wt%", "fraction") => Ok((0.01, 0.)),
        ("K", "degC") => Ok((1., -273.15)),
        ("degC", "K") => Ok((1., 273.15)),
        ("h", "min") => Ok((60., 0.)),
        ("min", "h") => Ok((1. / 60., 0.)),
        ("s", "min") => Ok((1. / 60., 0.)),
        ("min", "s") => Ok((60., 0.)),
        ("Pa", "MPa") => Ok((1e-6, 0.)),
        ("MPa", "Pa") => Ok((1e6, 0.)),
        ("GPa", "MPa") => Ok((1000., 0.)),
        ("MPa", "GPa") => Ok((0.001, 0.)),
        ("mm", "um") => Ok((1000., 0.)),
        ("um", "mm") => Ok((0.001, 0.)),
        ("m", "um") => Ok((1e6, 0.)),
        ("um", "m") => Ok((1e-6, 0.)),
        _ => Err(format!(
            "incompatible units: {from} -> {to}; hardness scales are distinct"
        )),
    }
}
pub fn convert(value: f64, from: &str, to: &str) -> Result<f64, String> {
    if !value.is_finite() {
        return Err("finite unit value required".into());
    }
    let (s, b) = conversion(from, to)?;
    let output = value * s + b;
    if !output.is_finite() {
        return Err("conversion_overflow".into());
    }
    Ok(output)
}
pub fn overlay(
    mut registry: Registry,
    fields: Vec<Field>,
    overrides: &[String],
) -> Result<Registry, String> {
    let mut seen = HashSet::new();
    if fields.len() > 256 {
        return Err("too_many_material_fields".into());
    }
    for field in fields {
        if !seen.insert(field.id.clone())
            || field.id.is_empty()
            || field.id.len() > 128
            || !field
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
            || !matches!(
                field.role.as_str(),
                "composition"
                    | "process"
                    | "microstructure"
                    | "property"
                    | "sample_metadata"
                    | "derived"
            )
            || !matches!(field.value_type.as_str(), "float" | "string")
            || field.aliases.len() > 64
            || field.aliases.iter().any(|s| s.len() > 128)
            || field.min.is_some_and(|v| !v.is_finite())
            || field.max.is_some_and(|v| !v.is_finite())
            || field.min.zip(field.max).is_some_and(|(a, b)| a > b)
        {
            return Err("invalid_material_field".into());
        }
        conversion(&field.canonical_unit, &field.canonical_unit)?;
        if let Some(i) = registry.fields.iter().position(|f| f.id == field.id) {
            if !overrides.contains(&field.id) {
                return Err(format!(
                    "schema_conflict_requires_explicit_override:{}",
                    field.id
                ));
            }
            registry.fields[i] = field;
        } else {
            registry.fields.push(field)
        }
    }
    Ok(registry)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub field_id: String,
    pub unit: String,
    pub min: f64,
    pub max: f64,
    pub fixed_value: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub r#type: String,
    pub source_ref: String,
    pub source_version: String,
    pub reviewed: bool,
    #[serde(default)]
    pub demonstration: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expression {
    Range {
        #[serde(rename = "fieldId")]
        field_id: String,
        unit: String,
        min: Option<f64>,
        max: Option<f64>,
    },
    Linear {
        coefficients: BTreeMap<String, f64>,
        units: BTreeMap<String, String>,
        operator: String,
        value: f64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub severity: String,
    pub enabled: bool,
    pub provenance: Provenance,
    pub expression: Expression,
    #[serde(default = "weight")]
    pub penalty: f64,
}
fn weight() -> f64 {
    1.
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Compiled {
    pub rule_id: String,
    pub name: String,
    pub severity: String,
    pub coefficients: BTreeMap<String, f64>,
    pub operator: String,
    pub value: f64,
    pub penalty: f64,
    pub provenance: Provenance,
}
pub fn compile(
    registry: &Registry,
    variables: &[Variable],
    rules: &[Rule],
) -> Result<Vec<Compiled>, String> {
    let mut names = HashSet::new();
    let mut fields = HashSet::new();
    for v in variables {
        if !names.insert(&v.name)
            || !fields.insert(&v.field_id)
            || !v.min.is_finite()
            || !v.max.is_finite()
            || v.min > v.max
            || v.fixed_value
                .is_some_and(|x| !x.is_finite() || x < v.min || x > v.max)
        {
            return Err("invalid_or_duplicate_material_variable".into());
        }
        let field = registry
            .fields
            .iter()
            .find(|f| f.id == v.field_id)
            .ok_or_else(|| format!("unknown_material_field:{}", v.field_id))?;
        if field.value_type != "float" || field.role == "sample_metadata" || field.role == "derived"
        {
            return Err(
                "derived_or_metadata_variable_requires_explicit_precomputed_mapping".into(),
            );
        }
        conversion(&v.unit, &field.canonical_unit)?;
        let lo = convert(v.min, &v.unit, &field.canonical_unit)?;
        let hi = convert(v.max, &v.unit, &field.canonical_unit)?;
        if field.min.is_some_and(|m| lo < m) || field.max.is_some_and(|m| hi > m) {
            return Err(format!("variable_outside_unit_legal_domain:{}", v.name));
        }
    }
    let mut out = vec![];
    let mut ids = HashSet::new();
    for r in rules {
        if !ids.insert(&r.id)
            || r.id.is_empty()
            || r.name.is_empty()
            || r.name.len() > 300
            || !matches!(r.severity.as_str(), "hard" | "soft" | "support")
            || !r.penalty.is_finite()
            || r.penalty <= 0.
            || !matches!(
                r.provenance.r#type.as_str(),
                "standard" | "user" | "literature" | "equipment" | "dataset"
            )
            || r.provenance.source_ref.trim().is_empty()
            || r.provenance.source_version.trim().is_empty()
        {
            return Err("invalid_material_rule_metadata".into());
        }
        if !r.enabled {
            continue;
        }
        if !r.provenance.reviewed || r.provenance.demonstration {
            return Err(format!("material_rule_requires_review:{}", r.id));
        }
        let mut push = |coefficients: BTreeMap<String, f64>, operator: &str, value: f64| {
            out.push(Compiled {
                rule_id: r.id.clone(),
                name: r.name.clone(),
                severity: r.severity.clone(),
                coefficients,
                operator: operator.into(),
                value,
                penalty: r.penalty,
                provenance: r.provenance.clone(),
            })
        };
        match &r.expression {
            Expression::Range {
                field_id,
                unit,
                min,
                max,
            } => {
                let v = variables
                    .iter()
                    .find(|v| &v.field_id == field_id)
                    .ok_or_else(|| format!("rule_variable_missing:{field_id}"))?;
                if min.zip(*max).is_some_and(|(a, b)| a > b) || min.is_none() && max.is_none() {
                    return Err("invalid_material_rule_range".into());
                }
                if let Some(low) = min {
                    push(
                        BTreeMap::from([(v.name.clone(), 1.)]),
                        "ge",
                        convert(*low, unit, &v.unit)?,
                    )
                }
                if let Some(high) = max {
                    push(
                        BTreeMap::from([(v.name.clone(), 1.)]),
                        "le",
                        convert(*high, unit, &v.unit)?,
                    )
                }
            }
            Expression::Linear {
                coefficients,
                units,
                operator,
                value,
            } => {
                if coefficients.is_empty()
                    || coefficients.len() > 64
                    || units.len() != coefficients.len()
                    || !value.is_finite()
                    || !matches!(operator.as_str(), "ge" | "le" | "eq")
                {
                    return Err("invalid_typed_linear_rule".into());
                }
                let mut c = BTreeMap::new();
                let mut rhs = *value;
                for (field, coefficient) in coefficients {
                    if !coefficient.is_finite() {
                        return Err("nonfinite_rule_coefficient".into());
                    }
                    let v = variables
                        .iter()
                        .find(|v| &v.field_id == field)
                        .ok_or_else(|| format!("rule_variable_missing:{field}"))?;
                    let (s, b) = conversion(&v.unit, units.get(field).ok_or("rule_unit_missing")?)?;
                    c.insert(v.name.clone(), coefficient * s);
                    rhs -= coefficient * b;
                }
                if !rhs.is_finite() || c.values().any(|v| !v.is_finite()) {
                    return Err("rule_conversion_overflow".into());
                }
                push(c, operator, rhs);
            }
        }
    }
    feasibility(variables, &out)?;
    Ok(out)
}
pub fn worker_constraints(compiled: &[Compiled]) -> Vec<Value> {
    compiled.iter().filter(|r|r.severity=="hard").map(|r|{let sign=if r.operator=="le"{-1.}else{1.};serde_json::json!({"kind":if r.operator=="eq"{"equality"}else{"inequality"},"coefficients":r.coefficients.iter().map(|(k,v)|(k.clone(),v*sign)).collect::<BTreeMap<_,_>>(),"value":r.value*sign,"tolerance":1e-8})}).collect()
}
/// Fourier–Motzkin feasibility for small typed linear systems. Complexity overflow
/// rejects submission explicitly instead of dropping constraints or guessing.
fn feasibility(vars: &[Variable], rules: &[Compiled]) -> Result<(), String> {
    let n = vars.len();
    if n > 24 {
        return Err("material_preflight_supports_up_to_24_variables".into());
    }
    let mut rows: Vec<(Vec<f64>, f64)> = vec![];
    for (i, v) in vars.iter().enumerate() {
        let mut a = vec![0.; n];
        a[i] = 1.;
        rows.push((a.clone(), v.fixed_value.unwrap_or(v.max)));
        a[i] = -1.;
        rows.push((a, -v.fixed_value.unwrap_or(v.min)));
    }
    for r in rules.iter().filter(|r| r.severity == "hard") {
        let a: Vec<f64> = vars
            .iter()
            .map(|v| *r.coefficients.get(&v.name).unwrap_or(&0.))
            .collect();
        if r.operator != "ge" {
            rows.push((a.clone(), r.value));
        }
        if r.operator != "le" {
            rows.push((a.iter().map(|x| -x).collect(), -r.value));
        }
    }
    for i in 0..n {
        let (pos, neg, mut zero) = (
            rows.iter()
                .filter(|(a, _)| a[i] > 1e-12)
                .cloned()
                .collect::<Vec<_>>(),
            rows.iter()
                .filter(|(a, _)| a[i] < -1e-12)
                .cloned()
                .collect::<Vec<_>>(),
            rows.iter()
                .filter(|(a, _)| a[i].abs() <= 1e-12)
                .cloned()
                .collect::<Vec<_>>(),
        );
        if pos.len() * neg.len() + zero.len() > 4096 {
            return Err("material_preflight_complexity_limit: simplify rules".into());
        }
        for (p, bp) in &pos {
            for (m, bm) in &neg {
                let mut a: Vec<f64> = p.iter().zip(m).map(|(x, y)| x / p[i] - y / m[i]).collect();
                a[i] = 0.;
                let b = bp / p[i] - bm / m[i];
                if !b.is_finite() || a.iter().any(|v| !v.is_finite()) {
                    return Err("material_preflight_overflow".into());
                }
                if a.iter().all(|v| v.abs() < 1e-10) {
                    if b < -1e-8 {
                        return Err(format!("hard_rule_conflict: infeasible residual {}", -b));
                    }
                } else {
                    zero.push((a, b));
                }
            }
        }
        rows = zero;
    }
    if rows
        .iter()
        .any(|(a, b)| a.iter().all(|v| v.abs() < 1e-10) && *b < -1e-8)
    {
        return Err("hard_rule_conflict".into());
    }
    Ok(())
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub rule_id: String,
    pub severity: String,
    pub satisfied: bool,
    pub margin: f64,
    pub penalty: f64,
}
pub fn assess(
    rules: &[Compiled],
    values: &BTreeMap<String, f64>,
) -> Result<Vec<Assessment>, String> {
    rules
        .iter()
        .map(|r| {
            let mut lhs = 0.;
            for (k, c) in &r.coefficients {
                let v = values
                    .get(k)
                    .filter(|v| v.is_finite())
                    .ok_or("candidate_variable_missing")?;
                lhs += c * v;
            }
            let margin = match r.operator.as_str() {
                "ge" => lhs - r.value,
                "le" => r.value - lhs,
                "eq" => -(lhs - r.value).abs(),
                _ => return Err("invalid_compiled_operator".into()),
            };
            Ok(Assessment {
                rule_id: r.rule_id.clone(),
                severity: r.severity.clone(),
                satisfied: margin >= -1e-7,
                margin,
                penalty: if r.severity == "soft" {
                    r.penalty * (-margin).max(0.)
                } else {
                    0.
                },
            })
        })
        .collect()
}

pub fn verify_worker_system(payload: &Value) -> Result<(), String> {
    let names = payload["feature_names"]
        .as_array()
        .ok_or("feature_schema_missing")?;
    let bounds = payload["bounds"].as_array().ok_or("bounds_missing")?;
    let mut variables = vec![];
    for (i, name) in names.iter().enumerate() {
        let name = name.as_str().ok_or("invalid_feature_name")?;
        variables.push(Variable {
            name: name.into(),
            field_id: String::new(),
            unit: String::new(),
            min: bounds[i]["min"].as_f64().ok_or("invalid_bound")?,
            max: bounds[i]["max"].as_f64().ok_or("invalid_bound")?,
            fixed_value: payload["fixed_values"][name].as_f64(),
        });
    }
    let mut compiled = vec![];
    for (i, c) in payload["constraints"]
        .as_array()
        .ok_or("constraints_missing")?
        .iter()
        .enumerate()
    {
        compiled.push(Compiled {
            rule_id: format!("manual-{i}"),
            name: "merged constraint".into(),
            severity: "hard".into(),
            coefficients: serde_json::from_value(c["coefficients"].clone())
                .map_err(|e| e.to_string())?,
            operator: if c["kind"] == "equality" { "eq" } else { "ge" }.into(),
            value: c["value"].as_f64().ok_or("invalid_constraint_value")?,
            penalty: 1.,
            provenance: Provenance {
                r#type: "user".into(),
                source_ref: "submitted_named_constraints".into(),
                source_version: "1".into(),
                reviewed: true,
                demonstration: false,
            },
        });
    }
    feasibility(&variables, &compiled)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn variable() -> Variable {
        Variable {
            name: "temperature".into(),
            field_id: "process.quench_temperature".into(),
            unit: "K".into(),
            min: 1073.15,
            max: 1173.15,
            fixed_value: None,
        }
    }
    fn rule(id: &str, min: Option<f64>, max: Option<f64>) -> Rule {
        Rule {
            id: id.into(),
            name: id.into(),
            severity: "hard".into(),
            enabled: true,
            penalty: 1.,
            provenance: Provenance {
                r#type: "equipment".into(),
                source_ref: "合成设备夹具".into(),
                source_version: "1".into(),
                reviewed: true,
                demonstration: false,
            },
            expression: Expression::Range {
                field_id: "process.quench_temperature".into(),
                unit: "degC".into(),
                min,
                max,
            },
        }
    }
    #[test]
    fn explicit_units_and_hardness_scales() {
        assert_eq!(convert(0.004, "fraction", "wt%").unwrap(), 0.4);
        assert_eq!(convert(2., "h", "min").unwrap(), 120.);
        assert!((convert(1173.15, "K", "degC").unwrap() - 900.).abs() < 1e-10);
        assert!(convert(50., "HRC", "HV").is_err());
        assert!(convert(0.4, "%", "wt%").is_err());
        assert!(convert(5., "unknown", "unknown").is_err());
        assert_eq!(suggest(&base(), "硬度").len(), 3);
        assert!(suggest(&base(), "硬度").iter().all(|s| s.confidence < 0.5));
    }
    #[test]
    fn mixed_unit_rules_compile_and_recheck() {
        let c = compile(
            &base(),
            &[variable()],
            &[rule("range", Some(850.), Some(900.))],
        )
        .unwrap();
        let constraints = worker_constraints(&c);
        assert_eq!(constraints[1]["coefficients"]["temperature"], -1.);
        let a = assess(&c, &BTreeMap::from([("temperature".into(), 1153.15)])).unwrap();
        assert!(a.iter().all(|a| a.satisfied));
        let outside = assess(&c, &BTreeMap::from([("temperature".into(), 1100.)])).unwrap();
        assert!(outside.iter().any(|a| !a.satisfied));
    }
    #[test]
    fn contradictory_ranges_missing_fields_unreviewed_rules_fail() {
        assert!(compile(
            &base(),
            &[variable()],
            &[
                rule("lower", Some(890.), None),
                rule("upper", None, Some(880.))
            ]
        )
        .unwrap_err()
        .contains("conflict"));
        assert!(compile(&base(), &[], &[rule("missing", Some(850.), None)]).is_err());
        let mut r = rule("unreviewed", Some(850.), None);
        r.provenance.reviewed = false;
        assert!(compile(&base(), &[variable()], &[r]).is_err());
        assert!(compile(
            &base(),
            &[variable()],
            &[rule("inverted", Some(900.), Some(800.))]
        )
        .is_err());
    }
    #[test]
    fn coupled_linear_contradictions_are_detected() {
        let a = Variable {
            name: "C".into(),
            field_id: "chem.c".into(),
            unit: "wt%".into(),
            min: 0.,
            max: 1.,
            fixed_value: None,
        };
        let b = Variable {
            name: "Cr".into(),
            field_id: "chem.cr".into(),
            unit: "wt%".into(),
            min: 0.,
            max: 1.,
            fixed_value: None,
        };
        let mut r = rule("sum", None, None);
        r.expression = Expression::Linear {
            coefficients: BTreeMap::from([("chem.c".into(), 1.), ("chem.cr".into(), 1.)]),
            units: BTreeMap::from([
                ("chem.c".into(), "wt%".into()),
                ("chem.cr".into(), "wt%".into()),
            ]),
            operator: "ge".into(),
            value: 1.5,
        };
        let mut other = r.clone();
        other.id = "upper".into();
        if let Expression::Linear {
            ref mut operator,
            ref mut value,
            ..
        } = other.expression
        {
            *operator = "le".into();
            *value = 1.;
        }
        assert!(compile(&base(), &[a, b], &[r, other]).is_err());
    }
}
