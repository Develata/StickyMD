//! Extract explicit Cargo duration reports, without adding nested scopes.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::{Summary, seconds};

pub(super) fn record(line: &str, number: usize, summary: &mut Summary) -> Result<bool, String> {
    let (kind, label, elapsed, status) = if let Some(tail) = line.strip_prefix("Finished ") {
        if !tail.contains(" profile ") {
            return Ok(false);
        }
        let (profile, elapsed) = tail
            .rsplit_once(" in ")
            .ok_or("incomplete Cargo Finished timing")?;
        (
            "cargo_build",
            format!(
                "{} at line {number}",
                profile.split(" profile ").next().unwrap_or(profile)
            ),
            elapsed,
            "FINISHED",
        )
    } else if let Some(tail) = line.strip_prefix("test result: ") {
        let status = match tail.split_once(' ').map(|value| value.0) {
            Some("ok.") => "PASSED",
            Some("FAILED.") => "FAILED",
            _ => return Err("invalid Cargo test result status".into()),
        };
        let elapsed = tail
            .rsplit_once(" finished in ")
            .ok_or("incomplete Cargo test result timing")?
            .1;
        (
            "cargo_test_binary",
            format!("test binary at line {number}"),
            elapsed,
            status,
        )
    } else {
        return Ok(false);
    };
    let elapsed = duration(elapsed).map_err(|error| format!("line {number}: {error}"))?;
    let row = summary.record(kind, &label, "reported_elapsed_seconds", Some(elapsed));
    row.line = Some(number);
    row.status = Some(status.into());
    Ok(true)
}

fn duration(value: &str) -> Result<f64, String> {
    let mut total = 0.0;
    let mut previous = f64::INFINITY;
    let mut found = false;
    for part in value.split_whitespace() {
        let (value, multiplier) = if let Some(value) = part.strip_suffix("ms") {
            (value, 0.001)
        } else if let Some(value) = part.strip_suffix('s') {
            (value, 1.0)
        } else if let Some(value) = part.strip_suffix('m') {
            (value, 60.0)
        } else {
            return Err("unsupported Cargo duration unit".into());
        };
        if multiplier >= previous {
            return Err("duplicate or unordered Cargo duration units".into());
        }
        previous = multiplier;
        total += seconds(value)? * multiplier;
        found = true;
    }
    if !found || !total.is_finite() {
        return Err("invalid Cargo duration".into());
    }
    Ok(total)
}
