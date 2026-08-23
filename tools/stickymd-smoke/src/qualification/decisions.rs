//! Exact-candidate projection of USER release decisions.

use std::fs;
use std::path::Path;

use super::json;
use super::receipt::{self, Candidate};

const SOURCE_LEDGER: &str = "docs/report/phase-12-release-decisions.md";
pub(super) const DECISION_RECEIPT: &str = "dist/evidence/release-decisions.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Decision {
    pub(super) key: String,
    pub(super) status: String,
    pub(super) evidence: String,
}

pub(super) fn project(root: &Path, candidate: &Candidate) -> Result<(), String> {
    let source = fs::read_to_string(root.join(SOURCE_LEDGER))
        .map_err(|error| format!("cannot read release decision ledger: {error}"))?;
    let decisions = parse_markdown(&source)?;
    write(root, candidate, &decisions)
}

pub(super) fn update(root: &Path, key: &str, status: &str, evidence: &str) -> Result<(), String> {
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let mut decisions = read(root, &candidate)?;
    let status = normalize_status(status)?;
    if evidence.trim().is_empty() {
        return Err("decision evidence must not be empty".to_owned());
    }
    if let Some(decision) = decisions.iter_mut().find(|decision| decision.key == key) {
        decision.status = status.to_owned();
        decision.evidence = evidence.to_owned();
    } else if valid_manual_waiver_key(key) {
        decisions.push(Decision {
            key: key.to_owned(),
            status: status.to_owned(),
            evidence: evidence.to_owned(),
        });
    } else {
        return Err(format!("unknown release decision `{key}`"));
    }
    write(root, &candidate, &decisions)
}

pub(super) fn read(root: &Path, candidate: &Candidate) -> Result<Vec<Decision>, String> {
    let document = receipt::read_receipt(&root.join(DECISION_RECEIPT))?;
    if json::u64_field(&document, "schema_version")? != 1 {
        return Err("release decision receipt schema is not version 1".to_owned());
    }
    for (field, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
    ] {
        let actual = json::string_field(&document, field)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: decision {field} is {actual}, expected {expected}"
            ));
        }
    }
    parse_json_decisions(&document)
}

pub(super) fn status<'a>(decisions: &'a [Decision], key: &str) -> Option<&'a str> {
    decisions
        .iter()
        .find(|decision| decision.key == key)
        .map(|decision| decision.status.as_str())
}

fn parse_markdown(content: &str) -> Result<Vec<Decision>, String> {
    let mut decisions = Vec::new();
    for line in content.lines() {
        if !line.trim_start().starts_with("| DEC-") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 4 {
            return Err("decision ledger rows must contain four columns".to_owned());
        }
        let status = normalize_status(cells[2])?;
        if decisions
            .iter()
            .any(|decision: &Decision| decision.key == cells[1])
        {
            return Err(format!("duplicate decision key `{}`", cells[1]));
        }
        decisions.push(Decision {
            key: cells[1].to_owned(),
            status: status.to_owned(),
            evidence: cells[3].to_owned(),
        });
    }
    if decisions.is_empty() {
        return Err("decision ledger contains no DEC rows".to_owned());
    }
    Ok(decisions)
}

fn parse_json_decisions(document: &str) -> Result<Vec<Decision>, String> {
    let marker = "\"decision\":";
    let mut decisions = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative;
        let next = document[start + marker.len()..]
            .find(marker)
            .map_or(document.len(), |next| start + marker.len() + next);
        let object = &document[start..next];
        let decision = Decision {
            key: json::string_field(object, "decision")?,
            status: json::string_field(object, "status")?,
            evidence: json::string_field(object, "evidence")?,
        };
        normalize_status(&decision.status)?;
        if decisions
            .iter()
            .any(|existing: &Decision| existing.key == decision.key)
        {
            return Err(format!("duplicate decision key `{}`", decision.key));
        }
        decisions.push(decision);
        offset = next;
    }
    if decisions.is_empty() {
        return Err("release decision receipt contains no decisions".to_owned());
    }
    Ok(decisions)
}

fn write(root: &Path, candidate: &Candidate, decisions: &[Decision]) -> Result<(), String> {
    let mut document = format!(
        "{{\"schema_version\":1,\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",\"decisions\":[",
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
    );
    for (index, decision) in decisions.iter().enumerate() {
        if index > 0 {
            document.push(',');
        }
        document.push_str(&format!(
            "{{\"decision\":\"{}\",\"status\":\"{}\",\"evidence\":\"{}\"}}",
            json::escape(&decision.key),
            json::escape(&decision.status),
            json::escape(&decision.evidence),
        ));
    }
    document.push_str("]}\n");
    receipt::write_receipt(root, DECISION_RECEIPT, &document)
}

fn normalize_status(value: &str) -> Result<&'static str, String> {
    match value.to_ascii_uppercase().replace(['-', '_'], " ").as_str() {
        "PENDING" => Ok("PENDING"),
        "USER APPROVED" => Ok("USER APPROVED"),
        "USER REJECTED" => Ok("USER REJECTED"),
        "NOT APPLICABLE" => Ok("NOT APPLICABLE"),
        _ => Err(format!("invalid USER decision status `{value}`")),
    }
}

fn valid_manual_waiver_key(key: &str) -> bool {
    key.strip_prefix("WAIVER-P12-M").is_some_and(|value| {
        value.len() == 2
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && value
                .parse::<u8>()
                .is_ok_and(|number| (1..=44).contains(&number))
    })
}

#[cfg(test)]
mod tests {
    use super::{normalize_status, parse_markdown, valid_manual_waiver_key};

    #[test]
    fn source_ledger_rejects_duplicate_or_inferred_decisions() {
        let source = "| DEC-01 | WARM-STARTUP-GATE | USER APPROVED | USER message |\n";
        let decisions = parse_markdown(source).expect("valid ledger");
        assert_eq!(decisions[0].key, "WARM-STARTUP-GATE");
        assert!(normalize_status("approved").is_err());
    }

    #[test]
    fn waiver_keys_are_specific_and_bounded() {
        assert!(valid_manual_waiver_key("WAIVER-P12-M01"));
        assert!(valid_manual_waiver_key("WAIVER-P12-M44"));
        assert!(!valid_manual_waiver_key("WAIVER-P12-M1"));
        assert!(!valid_manual_waiver_key("WAIVER-P12-M45"));
        assert!(!valid_manual_waiver_key("MANUAL-WAIVERS"));
    }
}
