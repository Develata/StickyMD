//! Evidence required by the approved M05 and M35 recorder procedures.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::log::Status;
use std::collections::BTreeMap;

pub(in crate::qualification) fn wake_kind(case: &str, scenario: &str) -> Option<&'static str> {
    if case != "P12-M05" {
        return None;
    }
    match scenario {
        "second-launch-hidden" => Some("hidden"),
        "second-launch-minimized" => Some("minimized"),
        "second-launch-docked" => Some("docked"),
        _ => None,
    }
}

pub(in crate::qualification) fn validate(
    case: &str,
    scenario: &str,
    status: Status,
    facts: &BTreeMap<String, String>,
) -> Result<(), String> {
    // A started product may fail before procedure setup. The recorder refuses PASS,
    // while a human's FAIL must remain appendable and blocking.
    if status == Status::Fail {
        return Ok(());
    }
    let required = |key: &str, expected: &str| {
        if facts.get(key).map(String::as_str) == Some(expected) {
            Ok(())
        } else {
            Err(format!(
                "{case}/{scenario} lacks procedure fact {key}={expected}"
            ))
        }
    };
    if let Some(kind) = wake_kind(case, scenario) {
        required("precondition", kind)?;
        for key in [
            "continuous-until-launch",
            "launch-before-restore",
            "typed-original",
            "marker-durable",
            "restored",
            "activated",
        ] {
            required(key, "yes")?;
        }
        let epoch = facts
            .get("sample-origin-unix-ms")
            .and_then(|value| value.parse::<u64>().ok());
        if epoch.is_none_or(|value| value == 0) {
            return Err("missing sample clock origin".to_owned());
        }
        let mut samples = String::new();
        for (index, (key, value)) in facts
            .iter()
            .filter(|(key, _)| key.starts_with("samples-"))
            .enumerate()
        {
            if *key != format!("samples-{index:02}") || index >= 8 || value.is_empty() {
                return Err("wake sample chunks are missing or out of order".to_owned());
            }
            samples.push_str(value);
        }
        validate_samples(&samples)?;
    }
    if case == "P12-M05" && scenario == "exit-save-failure" {
        for key in [
            "readonly-applied",
            "readonly-restored",
            "before-quit-unchanged",
            "quit-failure-title",
            "retained-memory-text",
            "original-alive",
            "visible",
        ] {
            required(key, "yes")?;
        }
        crate::integrity::validate_sha256(
            facts
                .get("baseline-sha256")
                .ok_or("missing baseline digest")?,
            "baseline",
        )?;
    }
    if case == "P12-M35" && matches!(scenario, "secondary-right" | "secondary-below") {
        let primary = parse_rect(
            facts
                .get("primary-rect")
                .ok_or("missing primary rectangle")?,
        )?;
        let secondary = parse_rect(
            facts
                .get("secondary-rect")
                .ok_or("missing secondary rectangle")?,
        )?;
        if !aligned(scenario, primary, secondary) {
            return Err("recorded monitor rectangles do not match the scenario".to_owned());
        }
        required("topology-retained", "yes")?;
    }
    Ok(())
}

/// x, y, width, height in physical pixels. Negative coordinates remain signed.
pub(in crate::qualification) fn parse_rect(text: &str) -> Result<[i64; 4], String> {
    let values = text
        .split(',')
        .map(str::parse::<i64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid monitor rectangle")?;
    let values: [i64; 4] = values
        .try_into()
        .map_err(|_| "monitor rectangle needs four values")?;
    if values
        .iter()
        .any(|value| !(-1_000_000..=1_000_000).contains(value))
        || values[2] <= 0
        || values[3] <= 0
    {
        return Err("invalid monitor dimensions".to_owned());
    }
    Ok(values)
}

pub(in crate::qualification) fn aligned(scenario: &str, p: [i64; 4], s: [i64; 4]) -> bool {
    match scenario {
        "secondary-right" => s[0] >= p[0] + p[2] && s[1] == p[1],
        "secondary-below" => s[1] >= p[1] + p[3] && s[0] == p[0],
        _ => false,
    }
}

/// Run-length encoded samples: first:last:count:state; all runs retain their times.
fn validate_samples(text: &str) -> Result<(), String> {
    let mut previous = None;
    let mut active = false;
    let mut runs = 0;
    for run in text.split_terminator(';') {
        let parts: Vec<_> = run.split(':').collect();
        if parts.len() != 4 {
            return Err("malformed wake sample run".to_owned());
        }
        let numbers = parts[..3]
            .iter()
            .map(|s| s.parse::<u64>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "malformed sample time")?;
        let (first, last, count) = (numbers[0], numbers[1], numbers[2]);
        if last < first
            || last > 120_500
            || count == 0
            || count > 1202
            || previous.is_some_and(|prior| first < prior)
            || !matches!(parts[3], "p" | "r" | "a" | "o")
            || (runs == 0 && (first != 0 || parts[3] != "p"))
        {
            return Err("invalid wake sample chronology or precondition".to_owned());
        }
        previous = Some(last);
        active |= parts[3] == "a";
        runs += 1;
    }
    if !(2..=32).contains(&runs) || !active {
        return Err("wake samples do not show activation after the precondition".to_owned());
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::qualification) fn fixture(case: &str, scenario: &str) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    if let Some(kind) = wake_kind(case, scenario) {
        f.insert("precondition".to_owned(), kind.to_owned());
        f.insert(
            "sample-origin-unix-ms".to_owned(),
            "1791900000000".to_owned(),
        );
        f.insert("samples-00".to_owned(), "0:100:2:p;200:200:1:a;".to_owned());
        for key in [
            "continuous-until-launch",
            "launch-before-restore",
            "typed-original",
            "marker-durable",
            "restored",
            "activated",
        ] {
            f.insert(key.to_owned(), "yes".to_owned());
        }
    } else if case == "P12-M05" && scenario == "exit-save-failure" {
        f.insert("baseline-sha256".to_owned(), "a".repeat(64));
        for key in [
            "readonly-applied",
            "readonly-restored",
            "before-quit-unchanged",
            "quit-failure-title",
            "retained-memory-text",
            "original-alive",
            "visible",
        ] {
            f.insert(key.to_owned(), "yes".to_owned());
        }
    } else if case == "P12-M35" && matches!(scenario, "secondary-right" | "secondary-below") {
        f.insert("primary-rect".to_owned(), "0,0,1920,1080".to_owned());
        f.insert(
            "secondary-rect".to_owned(),
            if scenario == "secondary-right" {
                "1920,0,1920,1080"
            } else {
                "0,1080,1920,1080"
            }
            .to_owned(),
        );
        f.insert("topology-retained".to_owned(), "yes".to_owned());
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn procedure_facts_cannot_be_omitted_from_a_pass() {
        for (case, scenario) in [
            ("P12-M05", "second-launch-hidden"),
            ("P12-M05", "second-launch-minimized"),
            ("P12-M05", "second-launch-docked"),
            ("P12-M05", "exit-save-failure"),
            ("P12-M35", "secondary-right"),
            ("P12-M35", "secondary-below"),
        ] {
            let facts = fixture(case, scenario);
            validate(case, scenario, Status::Pass, &facts).unwrap();
            for key in facts.keys() {
                let mut missing = facts.clone();
                missing.remove(key);
                assert!(
                    validate(case, scenario, Status::Pass, &missing).is_err(),
                    "{case} {scenario} {key}"
                );
            }
            validate(case, scenario, Status::Fail, &BTreeMap::new()).unwrap();
        }
    }
    #[test]
    fn topology_and_wake_chronology_are_strict() {
        let mut facts = fixture("P12-M05", "second-launch-hidden");
        let trace = facts.remove("samples-00").unwrap();
        facts.insert("samples-01".to_owned(), trace);
        assert!(validate("P12-M05", "second-launch-hidden", Status::Pass, &facts).is_err());
        assert!(!aligned(
            "secondary-right",
            [0, 0, 1920, 1080],
            [1920, 1, 1920, 1080]
        ));
        assert!(!aligned(
            "secondary-below",
            [0, 0, 1920, 1080],
            [1, 1080, 1920, 1080]
        ));
        for bad in [
            "",
            "0:10:2:a;",
            "0:10:2:p;20:20:1:r;",
            "0:10:0:p;20:20:1:a;",
            "0:10:1:p;5:5:1:a;",
            "0:10:1:p;120501:120501:1:a;",
        ] {
            assert!(validate_samples(bad).is_err(), "{bad}");
        }
    }
}
