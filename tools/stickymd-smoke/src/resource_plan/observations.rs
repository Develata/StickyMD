//! Validate independently archived resource observations, aliases and existing hard gates.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::evidence::EvidenceGate;
use crate::release::json::Value;
use std::collections::BTreeMap;

pub(super) type Measurements<'a> = BTreeMap<&'a str, (f64, &'a str)>;

pub(super) fn measurements(items: &Value) -> Result<Measurements<'_>, String> {
    let mut values = BTreeMap::new();
    for item in items.array()? {
        let name = item.field("name")?.string()?;
        let value = number(item.field("value")?)?;
        if values
            .insert(name, (value, item.field("unit")?.string()?))
            .is_some()
        {
            return Err(format!("duplicate resource measurement {name}"));
        }
    }
    Ok(values)
}

fn number(value: &Value) -> Result<f64, String> {
    let Value::Number(text) = value else {
        return Err("resource measurement is not numeric".into());
    };
    let value = text.parse::<f64>().map_err(|_| "invalid resource number")?;
    if !value.is_finite() {
        return Err("non-finite resource measurement".into());
    }
    Ok(value)
}

pub(super) fn required_gates(group: ResourceModule) -> Vec<EvidenceGate> {
    let mut gates = Vec::new();
    let mut add = |metric: String, value, unit: &str| {
        gates.push(EvidenceGate {
            metric,
            comparator: "<=".into(),
            value,
            unit: unit.into(),
            source: "docs/plan/10_performance_reliability.md#initial-engineering-targets".into(),
        })
    };
    for (name, cpu, _) in group.cohorts() {
        if cpu {
            add(
                format!("{name}.idle_cpu_max"),
                IDLE_CPU_PERCENT_LIMIT,
                "percent",
            );
        }
        let memory_limit = match group {
            ResourceModule::Window if name == "hidden-to-tray" => {
                Some(HIDDEN_PRIVATE_WORKING_SET_LIMIT)
            }
            ResourceModule::Zoom => zoom::CASES
                .iter()
                .find(|case| case.fixture.label == name)
                .and_then(|case| baseline_private_working_set_limit(case.fixture.view_mode)),
            _ => group
                .cases()
                .iter()
                .find(|case| case.label == name)
                .and_then(|case| case.private_working_set_limit()),
        };
        if let Some(limit) = memory_limit {
            add(
                format!("{name}.private_working_set_max"),
                limit as f64,
                "bytes",
            );
        }
    }
    if group == ResourceModule::Zoom {
        add(
            "zoom_cycles.private_growth".into(),
            ZOOM_PRIVATE_GROWTH_LIMIT as f64,
            "bytes",
        );
    }
    gates
}

pub(super) fn validate(
    result: &Value,
    group: ResourceModule,
    summary: &Measurements<'_>,
) -> Result<(), String> {
    let cohorts = group.cohorts();
    validate_cohorts(
        result,
        &cohorts,
        &required_gates(group),
        summary,
        |name, origin| validate_origin(group, name, origin),
    )
}

pub(crate) fn validate_case(result: &Value, case: ResourceCase) -> Result<(), String> {
    if !GROUPS.iter().any(|g| g.cases().contains(&case))
        || result.field("id")?.string()? != case.label
        || result.field("status")?.string()? != "PASSED"
    {
        return Err("diagnostic cache requires one complete registered case".into());
    }
    let summary = measurements(result.field("measurements")?)?;
    for expected in coverage::cohort_coverage(
        case.label,
        REPETITIONS,
        if case.measure_cpu { REPETITIONS } else { 0 },
        WARMUP_SECONDS,
    ) {
        if summary.get(expected.name.as_str()) != Some(&(expected.value, expected.unit.as_str())) {
            return Err(format!("incomplete diagnostic protocol {}", expected.name));
        }
    }
    // Reuse the authoritative group rules for a diagnostic case, including
    // memory gates. A cached case must not be weaker than a full group receipt.
    let group = GROUPS
        .into_iter()
        .find(|group| group.cases().contains(&case))
        .ok_or("unregistered resource case")?;
    let prefix = format!("{}.", case.label);
    let gates: Vec<_> = required_gates(group)
        .into_iter()
        .filter(|gate| gate.metric.starts_with(&prefix))
        .collect();
    validate_cohorts(
        result,
        &[(case.label, case.measure_cpu, WARMUP_SECONDS)],
        &gates,
        &summary,
        |_, origin| {
            if origin.is_none() {
                Ok(())
            } else {
                Err("cache accepts fresh observations only".into())
            }
        },
    )
}

fn validate_cohorts(
    result: &Value,
    cohorts: &[(&str, bool, u64)],
    required: &[EvidenceGate],
    summary: &Measurements<'_>,
    validate_origin: impl Fn(&str, Option<&str>) -> Result<(), String>,
) -> Result<(), String> {
    let samples = result.field("samples")?.array()?;
    if samples.len() != cohorts.len() * REPETITIONS {
        return Err("resource receipt must contain five actual observations per cohort".into());
    }
    let mut observed = BTreeMap::new();
    for sample in samples {
        let name = sample.field("cohort")?.string()?;
        let run = sample.field("run")?.unsigned()?;
        if !(1..=REPETITIONS as u64).contains(&run) || !cohorts.iter().any(|(n, _, _)| *n == name) {
            return Err(format!("unexpected resource observation {name} run {run}"));
        }
        let origin = sample
            .field("shared_from")
            .ok()
            .map(Value::optional_string)
            .transpose()?
            .flatten();
        validate_origin(name, origin)?;
        let values = measurements(sample.field("measurements")?)?;
        if observed.insert((name, run), (values, origin)).is_some() {
            return Err(format!("duplicate resource observation {name} run {run}"));
        }
    }
    for &(name, cpu, _) in cohorts {
        let mut origin = None;
        for run in 1..=REPETITIONS as u64 {
            let (values, shared) = &observed[&(name, run)];
            if run == 1 {
                origin = *shared;
            }
            if *shared != origin || values.len() != if cpu { 5 } else { 4 } {
                return Err(format!(
                    "inconsistent resource observation {name} run {run}"
                ));
            }
            for metric in [
                "private_working_set",
                "private_bytes",
                "peak_working_set",
                "peak_private_bytes",
            ] {
                checked(values, metric, "bytes", true)?;
            }
            if cpu {
                checked(values, "idle_cpu", "percent", true)?;
            }
        }
        for (metric, unit) in [
            ("private_working_set", "bytes"),
            ("private_bytes", "bytes"),
            ("idle_cpu", "percent"),
        ] {
            if metric == "idle_cpu" && !cpu {
                continue;
            }
            let mut values = (1..=REPETITIONS as u64)
                .map(|run| observed[&(name, run)].0[metric].0)
                .collect::<Vec<_>>();
            values.sort_by(f64::total_cmp);
            for (stat, value) in [
                ("median", values[REPETITIONS / 2]),
                ("max", values[REPETITIONS - 1]),
            ] {
                if summary.get(format!("{name}.{metric}_{stat}").as_str()) != Some(&(value, unit)) {
                    return Err(format!(
                        "{name}.{metric}_{stat} differs from its raw observations"
                    ));
                }
            }
            if metric == "idle_cpu"
                && summary.get(format!("{name}.idle_cpu_p95").as_str())
                    != Some(&(values[REPETITIONS - 1], unit))
            {
                return Err(format!(
                    "{name}.idle_cpu_p95 differs from its five observations"
                ));
            }
        }
    }
    let gates = result.field("gates")?.array()?;
    if gates.len() != required.len() {
        return Err("incomplete resource hard gates".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for gate in gates {
        let name = gate.field("metric")?.string()?;
        let expected = required
            .iter()
            .find(|g| g.metric == name)
            .ok_or_else(|| format!("unknown resource hard gate {name}"))?;
        if !seen.insert(name)
            || gate.field("comparator")?.string()? != expected.comparator
            || gate.field("unit")?.string()? != expected.unit
            || number(gate.field("value")?)? != expected.value
            || gate.field("source")?.string()?.is_empty()
        {
            return Err(format!("invalid resource hard gate {name}"));
        }
        let observed = checked(
            summary,
            name,
            &expected.unit,
            name != "zoom_cycles.private_growth",
        )?;
        if observed > expected.value {
            return Err(format!("resource hard gate {name} exceeded"));
        }
    }
    Ok(())
}

fn checked(
    values: &Measurements<'_>,
    name: &str,
    unit: &str,
    nonnegative: bool,
) -> Result<f64, String> {
    match values.get(name) {
        Some((value, actual)) if *actual == unit && (!nonnegative || *value >= 0.0) => Ok(*value),
        _ => Err(format!(
            "missing or invalid resource measurement {name} ({unit})"
        )),
    }
}

fn validate_origin(group: ResourceModule, name: &str, origin: Option<&str>) -> Result<(), String> {
    let Some(origin) = origin else {
        return Ok(());
    };
    let alias = group.cases().iter().find(|c| c.label == name);
    let source = GROUPS
        .iter()
        .flat_map(|g| g.cases())
        .find(|c| c.label == origin);
    if name != origin && alias.zip(source).is_some_and(|(a, b)| a.equivalent(*b)) {
        Ok(())
    } else {
        Err(format!("{name} cannot share the {origin} cohort"))
    }
}
