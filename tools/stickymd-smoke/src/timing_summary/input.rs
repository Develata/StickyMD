//! Interpret fixed receipt schemas without changing their acceptance status.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Summary, seconds};
use crate::release::json::Value;
use std::collections::BTreeSet;

pub(super) fn summarize(value: &Value) -> Result<Summary, String> {
    let version = value.field("schema_version")?.unsigned()?;
    if optional(value, "results").is_some() {
        if version != 2 {
            return Err("unsupported evidence schema_version (expected 2)".into());
        }
        return evidence(value);
    }
    if version == 1
        && optional(value, "evidence_class")
            .map(Value::string)
            .transpose()?
            == Some("SOURCE_BOUND")
    {
        if value.field("scope")?.string()? != "FULL_WORKSPACE"
            || value.field("status")?.string()? != "PASSED"
        {
            return Err("unsupported source-bound workspace receipt".into());
        }
        let mut summary = Summary {
            format: "workspace_source_receipt_json",
            status: "PASSED".into(),
            suite: Some(value.field("command")?.string()?.into()),
            source: Some(value.field("source_commit")?.string()?.into()),
            ..Summary::default()
        };
        let elapsed = number(value.field("run_seconds")?)?;
        summary.record(
            "current_nested",
            "workspace tests",
            "run_seconds",
            Some(elapsed),
        );
        return Ok(summary);
    }
    if optional(value, "kind").map(Value::string).transpose()? == Some("DIAGNOSTIC_PLAN") {
        if version != 1 || value.field("status")?.string()? != "NOT_RUN" {
            return Err("expected schema 1 NOT_RUN diagnostic plan".into());
        }
        let mut summary = Summary {
            format: "diagnostic_plan_json",
            status: "NOT_RUN".into(),
            ..Summary::default()
        };
        for field in ["planned_fixed_wait_seconds", "avoided_fixed_wait_seconds"] {
            budget(&mut summary, value, "diagnostic plan", field)?;
        }
        return Ok(summary);
    }
    if version == 1 && value.field("status")?.string()? == "INCOMPLETE" {
        let mut summary = Summary {
            format: "resource_progress_json",
            status: "INCOMPLETE".into(),
            ..Summary::default()
        };
        let stage = value.field("stage")?.string()?;
        budget(&mut summary, value, stage, "remaining_fixed_wait_seconds")?;
        if optional(value, "phase_fixed_seconds").is_some() {
            budget(&mut summary, value, stage, "phase_fixed_seconds")?;
        }
        return Ok(summary);
    }
    Err("unsupported timing JSON schema".into())
}

fn evidence(value: &Value) -> Result<Summary, String> {
    let mut summary = Summary {
        format: "evidence_json",
        suite: Some(value.field("suite")?.string()?.into()),
        source: Some(value.field("commit")?.string()?.into()),
        status: optional(value, "status")
            .map(Value::string)
            .transpose()?
            .unwrap_or("RECORDED_RESULTS")
            .into(),
        ..Summary::default()
    };
    let mut ids = BTreeSet::new();
    for result in value.field("results")?.array()? {
        let id = result.field("id")?.string()?;
        if id.is_empty() || !ids.insert(id) {
            return Err(format!("empty or duplicate result id: {id:?}"));
        }
        let status = result.field("status")?.string()?;
        if !matches!(status, "PASSED" | "FAILED" | "NOT_TESTED") {
            return Err(format!("invalid result status: {status}"));
        }
        let detail = optional(result, "detail")
            .map(Value::optional_string)
            .transpose()?
            .flatten();
        let mut found = false;
        let mut names = BTreeSet::new();
        for measurement in result.field("measurements")?.array()? {
            let name = measurement.field("name")?.string()?;
            let kind = match name {
                "task.execution_seconds" => "current_task",
                "group.execution_seconds" => "current_group",
                "workspace.origin_run_seconds" => "historical_origin",
                name if name.ends_with(".origin_execution_seconds") => "historical_origin",
                "workspace.run_seconds" => "current_nested",
                // Resource case timers wrap the current lookup or execution;
                // explicit origin timers alone describe the historical run.
                name if name.ends_with(".execution_seconds") => "current_nested",
                name if name.starts_with("workspace.identity_") && name.ends_with("_seconds") => {
                    "identity_component"
                }
                _ => continue,
            };
            if !names.insert(name) {
                return Err(format!("duplicate timing measurement {name} for {id}"));
            }
            if measurement.field("unit")?.string()? != "seconds" {
                return Err(format!("timing measurement {name} must use seconds"));
            }
            let elapsed = number(measurement.field("value")?)?;
            let observation = summary.record(kind, id, name, Some(elapsed));
            observation.status = Some(status.into());
            if kind == "historical_origin" {
                // This is the original run's observation, never time spent by
                // the current lookup. Report provenance without validating it.
                observation.origin = detail.and_then(origin_source).map(str::to_owned);
            }
            found |= name == "task.execution_seconds";
        }
        if !found {
            summary.missing_task_timings.push(id.into());
        }
    }
    if summary.observations.is_empty() && summary.missing_task_timings.is_empty() {
        return Err("no timing observations or result timing gaps".into());
    }
    Ok(summary)
}

fn origin_source(detail: &str) -> Option<&str> {
    if detail.starts_with("REUSED_PASS SOURCE_BOUND ") {
        return detail
            .split_whitespace()
            .find_map(|part| part.strip_prefix("origin_source="));
    }
    if !detail.starts_with("DIAGNOSTIC_REUSED ") {
        return None;
    }
    detail
        .split("; ")
        .find_map(|part| part.strip_prefix("origin_source="))
}

fn budget(summary: &mut Summary, value: &Value, label: &str, field: &str) -> Result<(), String> {
    let number = value.field(field)?;
    let seconds = if *number == Value::Null {
        None
    } else {
        Some(self::number(number)?)
    };
    summary.record("fixed_wait_budget", label, field, seconds);
    Ok(())
}

fn number(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(value) => seconds(value),
        _ => Err("timing value must be a JSON number".into()),
    }
}

fn optional<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(fields) => fields.get(key),
        _ => None,
    }
}
