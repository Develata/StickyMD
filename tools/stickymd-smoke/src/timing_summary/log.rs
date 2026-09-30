//! Existing timing stderr vocabulary; overlapping scopes remain separate.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Summary, seconds};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn summarize(text: &str) -> Result<Summary, String> {
    let mut summary = Summary {
        format: "timing_log",
        status: "COMPLETENESS_UNKNOWN".into(),
        ..Summary::default()
    };
    let mut tasks = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim_start();
        if super::cargo::record(line, line_number, &mut summary)? {
            continue;
        }
        let (prefix, tail) = line.split_once(' ').unwrap_or((line, ""));
        // Qualification planning also uses RESOURCE_GROUP= for RUN_REQUIRED
        // and REUSED_PASS diagnostics. Only explicit elapsed reports are timing.
        if prefix.starts_with("RESOURCE_GROUP=")
            && !tail
                .split_whitespace()
                .any(|field| field.starts_with("elapsed_seconds="))
        {
            continue;
        }
        let (label_field, metrics, kind): (&str, &[&str], &str) = match prefix {
            "TASK_TIMING" => ("task", &["elapsed_seconds"], "current_task"),
            "RESOURCE_CASE" => ("id", &["elapsed_seconds"], "current_nested"),
            "RESOURCE_IDENTITY" => (
                "",
                &[
                    "git_seconds",
                    "artifact_hash_seconds",
                    "environment_seconds",
                    "input_hash_seconds",
                    "total_seconds",
                ],
                "identity_component",
            ),
            "RESOURCE_PLAN" => (
                "groups",
                &[
                    "minimum_fixed_wait_seconds",
                    "shared_cohort_savings_seconds",
                ],
                "fixed_wait_budget",
            ),
            "RESOURCE_PROGRESS" => (
                "group",
                &["phase_fixed_seconds", "remaining_fixed_wait_seconds"],
                "fixed_wait_budget",
            ),
            "RESOURCE_RESUME_PLAN" if tail.starts_with("status=") => (
                "status",
                &["planned_fixed_wait_seconds", "avoided_fixed_wait_seconds"],
                "fixed_wait_budget",
            ),
            "RESOURCE_RESUME_PLAN" => ("unit", &["fixed_wait_seconds"], "fixed_wait_budget"),
            prefix if prefix.starts_with("RESOURCE_GROUP=") => {
                ("RESOURCE_GROUP", &["elapsed_seconds"], "current_group")
            }
            _ => continue,
        };
        let data = if prefix.starts_with("RESOURCE_GROUP=") {
            line
        } else {
            tail
        };
        let fields = fields(data).map_err(|error| format!("line {line_number}: {error}"))?;
        let label = if label_field.is_empty() {
            format!("identity capture at line {line_number}")
        } else {
            required(&fields, label_field)?.to_owned()
        };
        let status_key = if kind == "current_group" {
            "STATUS"
        } else {
            "status"
        };
        let status = fields.get(status_key).cloned();
        if kind == "current_task" {
            if !tasks.insert(label.clone()) {
                return Err(format!(
                    "duplicate task timing for {label:?}; split concatenated runs into separate inputs"
                ));
            }
            if !matches!(status.as_deref(), Some("PASSED" | "FAILED")) {
                return Err(format!("line {line_number}: invalid TASK_TIMING status"));
            }
        }
        for metric in metrics {
            let elapsed = seconds(required(&fields, metric)?)
                .map_err(|error| format!("line {line_number}: {metric}: {error}"))?;
            let row_kind = if prefix == "RESOURCE_IDENTITY" && *metric == "total_seconds" {
                "identity_total"
            } else {
                kind
            };
            let observation = summary.record(row_kind, &label, metric, Some(elapsed));
            observation.status = status.clone();
            observation.line = Some(line_number);
        }
    }
    if summary.observations.is_empty() {
        return Err("no recognized timing records".into());
    }
    Ok(summary)
}

fn required<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, String> {
    fields
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing log field {key}"))
}

fn fields(mut text: &str) -> Result<BTreeMap<String, String>, String> {
    let mut fields = BTreeMap::new();
    loop {
        text = text.trim_start();
        if text.is_empty() || text.starts_with('(') {
            return Ok(fields);
        }
        let (key, tail) = text.split_once('=').ok_or("expected timing key=value")?;
        if key.is_empty() || key.chars().any(char::is_whitespace) {
            return Err("invalid timing field name".into());
        }
        let (value, remainder) = if tail.starts_with('"') {
            let mut escaped = false;
            let end = tail
                .char_indices()
                .skip(1)
                .find_map(|(index, character)| {
                    if escaped {
                        escaped = false;
                        return None;
                    }
                    match character {
                        '\\' => escaped = true,
                        '"' => return Some(index + 1),
                        _ => {}
                    }
                    None
                })
                .ok_or("unterminated quoted timing field")?;
            let value = crate::release::json::parse(&tail[..end])?
                .string()?
                .to_owned();
            if !tail[end..].is_empty() && !tail[end..].starts_with(char::is_whitespace) {
                return Err("missing separator after quoted timing field".into());
            }
            (value, &tail[end..])
        } else {
            let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
            (tail[..end].to_owned(), &tail[end..])
        };
        if fields.insert(key.into(), value).is_some() {
            return Err(format!("duplicate log field {key}"));
        }
        text = remainder;
    }
}
