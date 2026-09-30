//! Structural coverage checks shared by recording and readiness.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{CPU_SECONDS, REPETITIONS};
use crate::{
    cli::ResourceModule,
    evidence::EvidenceMeasurement,
    release::json::{self, Value},
};
use std::collections::BTreeMap;

pub(crate) fn cohort_coverage(
    name: &str,
    memory_count: usize,
    cpu_count: usize,
    warmup: u64,
) -> Vec<EvidenceMeasurement> {
    [
        ("memory_samples", memory_count as f64, "count"),
        ("cpu_samples", cpu_count as f64, "count"),
        ("warmup_seconds", warmup as f64, "seconds"),
        (
            "cpu_interval_seconds",
            if cpu_count == 0 {
                0.0
            } else {
                CPU_SECONDS as f64
            },
            "seconds",
        ),
    ]
    .into_iter()
    .map(|(metric, value, unit)| EvidenceMeasurement {
        name: format!("{name}.{metric}"),
        unit: unit.into(),
        value,
    })
    .collect()
}

/// Expected protocol markers; the executor records counts from the actual sample vectors.
pub(crate) fn coverage_measurements(group: ResourceModule) -> Vec<EvidenceMeasurement> {
    group
        .cohorts()
        .iter()
        .flat_map(|(name, cpu, warmup)| {
            cohort_coverage(
                name,
                REPETITIONS,
                if *cpu { REPETITIONS } else { 0 },
                *warmup,
            )
        })
        .collect()
}

pub(crate) fn validate_receipt(document: &str, group: ResourceModule) -> Result<(), String> {
    let root = json::parse(document)?;
    if root.field("schema_version")?.unsigned()? != 2
        || root.field("resource_protocol")?.unsigned()? != 1
        || root.field("resource_group")?.string()? != group.name()
        || root.field("suite")?.string()? != "phase-14"
        || root.field("worktree_dirty")? != &Value::Bool(false)
        || root
            .field("qualification_environment")?
            .field("status")?
            .string()?
            != "VALID"
    {
        return Err(format!(
            "{} resource receipt has incompatible scope/protocol",
            group.name()
        ));
    }
    let results = root.field("results")?.array()?;
    if results.len() != 1 {
        return Err("resource receipt must contain one result".into());
    }
    validate_group_result(&results[0], group)
}

pub(crate) fn validate_group_result(result: &Value, group: ResourceModule) -> Result<(), String> {
    if result.field("id")?.string()? != group.task_label()
        || result.field("status")?.string()? != "PASSED"
    {
        return Err(format!(
            "{} resource receipt is not one complete successful group",
            group.name()
        ));
    }
    let values = super::observations::measurements(result.field("measurements")?)?;
    for expected in coverage_measurements(group) {
        if values.get(expected.name.as_str()) != Some(&(expected.value, expected.unit.as_str())) {
            return Err(format!(
                "{} missing or invalid required cohort measurement {}",
                group.name(),
                expected.name
            ));
        }
    }
    for (name, cpu, _) in group.cohorts() {
        for statistic in [
            "private_working_set_median",
            "private_working_set_max",
            "private_bytes_median",
            "private_bytes_max",
        ] {
            require(&values, &format!("{name}.{statistic}"), "bytes")?;
        }
        if cpu {
            for statistic in ["idle_cpu_median", "idle_cpu_p95", "idle_cpu_max"] {
                require(&values, &format!("{name}.{statistic}"), "percent")?;
            }
        }
    }
    if group == ResourceModule::Window {
        for run in 1..=REPETITIONS {
            let name = format!("hidden-to-tray.run_{run}.fixture_bytes");
            if values.get(name.as_str()) != Some(&(20_480.0, "bytes")) {
                return Err(format!("missing or invalid restored window fixture {name}"));
            }
        }
        require(&values, "window.stress_completed", "count")?;
        if values["window.stress_completed"].0 != 1.0 {
            return Err("window stress is incomplete".into());
        }
    }
    if group == ResourceModule::Zoom {
        require(&values, "zoom_cycles.private_growth", "bytes")?;
    }
    super::observations::validate(result, group, &values)
}

fn require(values: &BTreeMap<&str, (f64, &str)>, name: &str, unit: &str) -> Result<(), String> {
    if values.get(name).is_some_and(|(_, actual)| *actual == unit) {
        Ok(())
    } else {
        Err(format!(
            "missing required resource measurement {name} ({unit})"
        ))
    }
}
