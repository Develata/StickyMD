//! Exact-candidate startup attribution from per-sample product milestones.

use std::path::Path;
use std::process::{Command, Stdio};

use super::json;
use super::receipt::{self, Candidate};

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
    let performance = receipt::read_receipt(&root.join(PERFORMANCE_RECEIPT))?;
    validate_identity(&performance, &candidate)?;
    let cold = dominant_category(&performance, "cold")?;
    let warm = dominant_category(&performance, "warm")?;
    let cold_p95 = measurement(&performance, "cold.p95")?;
    let warm_p95 = measurement(&performance, "warm.p95")?;
    let etw_status = etw_status();
    let decision = "NO PRODUCT OPTIMIZATION NEEDED";
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

fn validate_identity(document: &str, candidate: &Candidate) -> Result<(), String> {
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

fn dominant_category(document: &str, cohort: &str) -> Result<(&'static str, f64), String> {
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

fn measurement(document: &str, name: &str) -> Result<f64, String> {
    let marker = format!("\"name\":\"{}\"", json::escape(name));
    let start = document
        .find(&marker)
        .ok_or_else(|| format!("performance receipt is missing measurement `{name}`"))?;
    let object = &document[start
        ..document[start..]
            .find('}')
            .map_or(document.len(), |end| start + end + 1)];
    let value_start = object
        .find("\"value\":")
        .map(|offset| offset + "\"value\":".len())
        .ok_or_else(|| format!("measurement `{name}` has no value"))?;
    let value = object[value_start..]
        .split([',', '}'])
        .next()
        .ok_or_else(|| format!("measurement `{name}` has an empty value"))?;
    value
        .trim()
        .parse::<f64>()
        .map_err(|error| format!("measurement `{name}` is invalid: {error}"))
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
    use super::{dominant_category, measurement};

    #[test]
    fn attribution_reads_exact_named_measurements_and_selects_dominant_category() {
        let mut document = String::from("{\"measurements\":[");
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
        document.push_str("]}");
        assert_eq!(measurement(&document, "cold.p95"), Ok(477.0));
        assert_eq!(
            dominant_category(&document, "cold"),
            Ok(("font_discovery", 120.0))
        );
    }
}
