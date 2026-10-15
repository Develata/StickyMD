//! Exact-candidate startup attribution from per-sample product milestones.

use std::path::Path;
use std::process::{Command, Stdio};

use super::json;
use super::receipt::{self, Candidate};
use crate::startup_timing::{RAPID_RESTART_DIAGNOSTIC_IDLE, WARM_CACHE_START_IDLE};

const PERFORMANCE_RECEIPT: &str = "dist/evidence/performance-qualification.json";
const ATTRIBUTION_RECEIPT: &str = "dist/evidence/startup-attribution.json";
const CATEGORIES: &[&str] = &[
    "process_overhead",
    "bootstrap",
    "window_surface",
    "font_discovery",
    "source_layout",
    "shell_setup",
    "focus_guards",
];

pub(super) fn record(root: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let performance = json::parse_object(&receipt::read_receipt(&root.join(PERFORMANCE_RECEIPT))?)?;
    validate_identity(&performance, &candidate)?;
    let cold = dominant_category(&performance, "cold")?;
    let warm = dominant_category(&performance, "warm")?;
    let cold_p95 = measurement(&performance, "cold.p95")?;
    let warm_p95 = measurement(&performance, "warm.p95")?;
    let warm_cache_idle = measurement(&performance, "startup.warm_cache_idle")?;
    let rapid_restart_diagnostic_idle =
        measurement(&performance, "startup.rapid_restart_diagnostic_idle")?;
    validate_interval(
        "startup.warm_cache_idle",
        warm_cache_idle,
        WARM_CACHE_START_IDLE.as_millis() as f64,
    )?;
    validate_interval(
        "startup.rapid_restart_diagnostic_idle",
        rapid_restart_diagnostic_idle,
        RAPID_RESTART_DIAGNOSTIC_IDLE.as_millis() as f64,
    )?;
    let etw_status = etw_status();
    let decision = "ATTRIBUTION ONLY; SEE PERFORMANCE RECEIPT FOR GATE STATUS";
    let document = format!(
        concat!(
            "{{\"schema_version\":1,",
            "\"source_commit\":\"{}\",",
            "\"version\":\"{}\",",
            "\"exe_sha256\":\"{}\",",
            "\"method\":\"per-sample startup milestone intervals\",",
            "\"etw_status\":\"{}\",",
            "\"cold_p95_ms\":{:.6},",
            "\"warm_p95_ms\":{:.6},",
            "\"warm_cache_idle_ms\":{:.6},",
            "\"rapid_restart_diagnostic_idle_ms\":{:.6},",
            "\"preferred_target_ms\":180,",
            "\"engineering_target_ms\":400,",
            "\"release_boundary_ms\":550,",
            "\"cold_dominant\":{{\"category\":\"{}\",\"p95_ms\":{:.6}}},",
            "\"warm_dominant\":{{\"category\":\"{}\",\"p95_ms\":{:.6}}},",
            "\"decision\":\"{}\"}}\n"
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.version),
        json::escape(&candidate.exe_sha256),
        json::escape(&etw_status),
        cold_p95,
        warm_p95,
        warm_cache_idle,
        rapid_restart_diagnostic_idle,
        cold.0,
        cold.1,
        warm.0,
        warm.1,
        decision,
    );
    receipt::write_receipt(root, ATTRIBUTION_RECEIPT, &document)?;
    println!(
        "STARTUP_ATTRIBUTION={}",
        root.join(ATTRIBUTION_RECEIPT).display()
    );
    println!("ETW_STATUS={etw_status}");
    println!("STARTUP_DECISION={decision}");
    Ok(())
}

fn validate_identity(document: &json::Value, candidate: &Candidate) -> Result<(), String> {
    for (field, expected) in [
        ("commit", candidate.source_commit.as_str()),
        ("executable_sha256", candidate.exe_sha256.as_str()),
        ("suite", "phase-14"),
    ] {
        let actual = json::string_field(document, field)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: performance {field} is {actual}, expected {expected}"
            ));
        }
    }
    Ok(())
}

fn dominant_category(document: &json::Value, cohort: &str) -> Result<(&'static str, f64), String> {
    CATEGORIES
        .iter()
        .map(|category| {
            measurement(document, &format!("{cohort}.category.{category}.p95"))
                .map(|value| (*category, value))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by(|left, right| left.1.total_cmp(&right.1))
        .ok_or_else(|| format!("{cohort} attribution has no categories"))
}

/// The value of the one measurement called `name`, read from the `measurements` array of
/// each result object. Per-task measurements such as `task.execution_seconds` repeat across
/// results; the attribution inputs must appear exactly once.
fn measurement(document: &json::Value, name: &str) -> Result<f64, String> {
    let mut found = None;
    for result in json::objects(document, "results")? {
        if !json::has_field(result, "measurements") {
            continue;
        }
        for entry in json::objects(result, "measurements")? {
            if json::string_field(entry, "name")? != name {
                continue;
            }
            // Every attribution input is a duration in milliseconds.
            let unit = json::string_field(entry, "unit")?;
            let value = json::f64_field(entry, "value")?;
            if unit != "ms" || value < 0.0 {
                return Err(format!(
                    "measurement `{name}` is {value} {unit}, expected non-negative milliseconds"
                ));
            }
            if found.replace(value).is_some() {
                return Err(format!(
                    "performance receipt records measurement `{name}` more than once"
                ));
            }
        }
    }
    found.ok_or_else(|| format!("performance receipt is missing measurement `{name}`"))
}

fn validate_interval(name: &str, actual_ms: f64, expected_ms: f64) -> Result<(), String> {
    if actual_ms == expected_ms {
        Ok(())
    } else {
        Err(format!(
            "STALE RECEIPT: {name} is {actual_ms} ms, expected {expected_ms} ms"
        ))
    }
}

fn etw_status() -> String {
    if command_exists("wpr.exe") && command_exists("wpa.exe") {
        "ETW tools available; milestone attribution used for deterministic exact-candidate summary"
            .to_owned()
    } else {
        "ETW attribution NOT AVAILABLE".to_owned()
    }
}

fn command_exists(command: &str) -> bool {
    Command::new("where.exe")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::{dominant_category, measurement, validate_interval};

    #[test]
    fn attribution_reads_exact_named_measurements_and_selects_dominant_category() {
        let mut document = String::from("{\"results\":[{\"id\":\"startup\",\"measurements\":[");
        for (index, (name, value)) in [
            ("cold.p95", 477.0),
            ("cold.category.process_overhead.p95", 20.0),
            ("cold.category.bootstrap.p95", 80.0),
            ("cold.category.window_surface.p95", 30.0),
            ("cold.category.font_discovery.p95", 120.0),
            ("cold.category.source_layout.p95", 40.0),
            ("cold.category.shell_setup.p95", 25.0),
            ("cold.category.focus_guards.p95", 10.0),
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                document.push(',');
            }
            document.push_str(&format!(
                "{{\"name\":\"{name}\",\"unit\":\"ms\",\"value\":{value}}}"
            ));
        }
        document.push_str("]},{\"id\":\"other\",\"measurements\":[{\"name\":\"cold.p95\",\"unit\":\"ms\",\"value\":1}]}]}");
        let document = crate::qualification::json::parse_object(&document).unwrap();
        // A second `cold.p95` in another result is ambiguous, not "first one wins".
        assert!(measurement(&document, "cold.p95").is_err());
        assert_eq!(
            measurement(&document, "cold.category.bootstrap.p95"),
            Ok(80.0)
        );
        assert_eq!(
            dominant_category(&document, "cold"),
            Ok(("font_discovery", 120.0))
        );
        for entry in [
            r#"{"name":"warm.p95","unit":"s","value":0.4}"#,
            r#"{"name":"warm.p95","unit":"ms","value":1e400}"#,
            r#"{"name":"warm.p95","unit":"ms","value":-1}"#,
        ] {
            let document = crate::qualification::json::parse_object(&format!(
                r#"{{"results":[{{"id":"startup","measurements":[{entry}]}}]}}"#
            ))
            .unwrap();
            assert!(measurement(&document, "warm.p95").is_err(), "{entry}");
        }
    }

    #[test]
    fn attribution_rejects_a_stale_warm_interval_contract() {
        assert_eq!(validate_interval("warm", 1_000.0, 1_000.0), Ok(()));
        assert!(validate_interval("warm", 250.0, 1_000.0).is_err());
    }
}
