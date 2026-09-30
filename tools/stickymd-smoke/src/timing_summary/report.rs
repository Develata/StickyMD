//! Render observations without inventing totals from overlapping durations.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::Summary;
use crate::evidence::escape_json;
use std::path::PathBuf;

pub(super) fn render(inputs: &[(PathBuf, Summary)], json: bool) -> String {
    if json {
        return render_json(inputs);
    }
    let mut output = String::from(
        "TIMING_SUMMARY OBSERVATION_ONLY\nWall clock: unknown; agent work: unknown. 'Current' means the recorded run, not this tool invocation. Nested timings, repeated snapshots and historical origins are not added.\n",
    );
    for (path, summary) in inputs {
        output.push_str(&format!(
            "\nInput: {:?} ({}, {})\n",
            path.display().to_string(),
            summary.format,
            summary.status
        ));
        if let Some(suite) = &summary.suite {
            output.push_str(&format!("Recorded suite: {suite:?}\n"));
        }
        if let Some(source) = &summary.source {
            output.push_str(&format!("Recorded source: {source:?} (unverified)\n"));
        }
        for (kind, heading) in [
            ("current_task", "Current task observations"),
            ("cargo_build", "Cargo reported build observations"),
            (
                "cargo_test_binary",
                "Cargo reported test-binary observations",
            ),
            (
                "current_group",
                "Current group observations (overlap task timings)",
            ),
            (
                "current_nested",
                "Current nested observations (overlap task/group timings)",
            ),
            (
                "identity_total",
                "Identity capture totals (overlap task/group timings)",
            ),
            (
                "identity_component",
                "Identity components (overlap capture totals)",
            ),
            (
                "historical_origin",
                "Historical origin observations (not current elapsed time)",
            ),
            (
                "fixed_wait_budget",
                "Fixed-wait budgets and snapshots (not elapsed time)",
            ),
        ] {
            let rows: Vec<_> = summary
                .observations
                .iter()
                .filter(|row| row.kind == kind)
                .collect();
            if rows.is_empty() {
                continue;
            }
            output.push_str(&format!("{heading}:\n"));
            for row in rows {
                let seconds = row
                    .seconds
                    .map_or_else(|| "unknown".into(), |value| format!("{value:.6} s"));
                output.push_str(&format!("  {:?} {} = {seconds}", row.label, row.metric));
                if let Some(status) = &row.status {
                    output.push_str(&format!(" [{status:?}]"));
                }
                if let Some(origin) = &row.origin {
                    output.push_str(&format!(" origin_source={origin:?} (unverified)"));
                }
                if let Some(line) = row.line {
                    output.push_str(&format!(" line={line}"));
                }
                output.push('\n');
            }
        }
        if !summary.missing_task_timings.is_empty() {
            output.push_str("Missing task timings (unknown, not zero):\n");
            for id in &summary.missing_task_timings {
                output.push_str(&format!("  {id:?}\n"));
            }
        }
    }
    output.trim_end().into()
}

fn nullable(value: Option<&str>) -> String {
    value.map_or_else(
        || "null".into(),
        |value| format!("\"{}\"", escape_json(value)),
    )
}

fn render_json(inputs: &[(PathBuf, Summary)]) -> String {
    let mut output = String::from(
        "{\"schema_version\":1,\"kind\":\"TIMING_SUMMARY\",\"status\":\"OBSERVATION_ONLY\",\"wall_clock_seconds\":null,\"agent_work_seconds\":null,\"aggregation\":\"NONE_OVERLAPPING_SCOPES\",\"inputs\":[",
    );
    for (index, (path, summary)) in inputs.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!("{{\"input\":\"{}\",\"format\":\"{}\",\"input_status\":\"{}\",\"suite\":{},\"recorded_source\":{},\"source_verified\":false,\"missing_task_timings\":[{}],\"observations\":[", escape_json(&path.display().to_string()), summary.format, escape_json(&summary.status), nullable(summary.suite.as_deref()), nullable(summary.source.as_deref()), summary.missing_task_timings.iter().map(|value| format!("\"{}\"", escape_json(value))).collect::<Vec<_>>().join(",")));
        for (index, row) in summary.observations.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(&format!("{{\"kind\":\"{}\",\"label\":\"{}\",\"metric\":\"{}\",\"seconds\":{},\"status\":{},\"origin_source\":{},\"origin_verified\":false,\"line\":{}}}", row.kind, escape_json(&row.label), escape_json(&row.metric), row.seconds.map_or_else(|| "null".into(), |value| value.to_string()), nullable(row.status.as_deref()), nullable(row.origin.as_deref()), row.line.map_or_else(|| "null".into(), |line| line.to_string())));
        }
        output.push_str("]}");
    }
    output.push_str("]}");
    output
}
