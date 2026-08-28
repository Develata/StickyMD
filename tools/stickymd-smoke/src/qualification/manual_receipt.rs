//! Phase 12 manual-case model and exact-candidate receipt serialization.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::cli::{GuidedSession, ManualSession};

use super::exact_groups;
use super::receipt::Candidate;
use super::{guided, json, receipt};

pub(super) const MANUAL_RECEIPT: &str = "dist/evidence/manual-acceptance.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManualCase {
    pub(super) id: String,
    pub(super) action: String,
    pub(super) expected: String,
    pub(super) session: ManualSession,
    pub(super) tier: ManualTier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ManualTier {
    A,
    B,
    C,
}

impl ManualTier {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ManualObservation {
    pub(super) status: String,
    pub(super) note: String,
}

pub(super) fn persist(
    root: &Path,
    candidate: &Candidate,
    cases: &[ManualCase],
    observations: &BTreeMap<String, ManualObservation>,
) -> Result<(), String> {
    receipt::write_receipt(
        root,
        MANUAL_RECEIPT,
        &render_manual_receipt(candidate, cases, observations),
    )
}

pub(super) fn read_manual_cases(root: &Path) -> Result<Vec<ManualCase>, String> {
    let path = root.join("docs/acceptance-cases/phase-12.md");
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    parse_manual_cases(&content)
}

fn parse_manual_cases(content: &str) -> Result<Vec<ManualCase>, String> {
    let mut cases = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if !line.trim_start().starts_with("| P12-M") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 {
            return Err(format!(
                "phase-12 acceptance row {} must have five columns",
                index + 1
            ));
        }
        if cells[2] == "Automated exact candidate"
            && exact_groups::group_for_phase12_case(cells[0]).is_some()
        {
            continue;
        }
        if cells[2] != "Manual" {
            return Err(format!(
                "phase-12 acceptance row {} has unsupported mode {}",
                index + 1,
                cells[2]
            ));
        }
        cases.push(ManualCase {
            id: cells[0].to_owned(),
            action: cells[1].to_owned(),
            expected: cells[3].to_owned(),
            session: session_for_case(cells[0])?,
            tier: tier_for_case(cells[0])?,
        });
    }
    if cases.len() != 24 {
        return Err(format!(
            "phase-12 manual matrix must contain exactly 24 cases after G3/G4/G5 automation; observed {}",
            cases.len()
        ));
    }
    Ok(cases)
}

fn case_number(id: &str) -> Result<u8, String> {
    id.strip_prefix("P12-M")
        .ok_or_else(|| format!("invalid manual case ID `{id}`"))?
        .parse::<u8>()
        .map_err(|error| format!("invalid manual case ID `{id}`: {error}"))
}

fn session_for_case(id: &str) -> Result<ManualSession, String> {
    match case_number(id)? {
        1 | 2 | 21 | 24 | 25 => Ok(ManualSession::M1),
        3..=5 | 11 | 12 | 18..=20 | 22 | 23 => Ok(ManualSession::M2),
        26 => Ok(ManualSession::M3),
        35..=40 | 43 => Ok(ManualSession::M4),
        34 | 41 | 42 => Ok(ManualSession::M5),
        _ => Err(format!("manual case `{id}` has no Phase 13/14 session")),
    }
}

fn tier_for_case(id: &str) -> Result<ManualTier, String> {
    match case_number(id)? {
        1..=33 => Ok(ManualTier::A),
        34..=40 => Ok(ManualTier::B),
        41..=44 => Ok(ManualTier::C),
        _ => Err(format!("manual case `{id}` has no risk tier")),
    }
}

pub(super) fn load_observations(
    root: &Path,
    candidate: &Candidate,
    cases: &[ManualCase],
) -> Result<BTreeMap<String, ManualObservation>, String> {
    let mut observations = cases
        .iter()
        .map(|case| {
            (
                case.id.clone(),
                ManualObservation {
                    status: "NOT_TESTED".to_owned(),
                    note: String::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let path = root.join(MANUAL_RECEIPT);
    if !path.is_file() {
        return Ok(observations);
    }
    let document = receipt::read_receipt(&path)?;
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
        ("zip_sha256", candidate.zip_sha256.as_str()),
        ("version", candidate.version.as_str()),
    ] {
        let actual = json::string_field(&document, key)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: manual {key} is {actual}, expected {expected}"
            ));
        }
    }
    for (id, observation) in parse_observations(&document)? {
        if !observations.contains_key(&id) {
            return Err(format!("manual receipt contains unknown case `{id}`"));
        }
        observations.insert(id, observation);
    }
    Ok(observations)
}

fn parse_observations(document: &str) -> Result<Vec<(String, ManualObservation)>, String> {
    let marker = "\"case_id\":";
    let mut observations = Vec::new();
    let mut offset = 0;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative;
        let next = document[start + marker.len()..]
            .find(marker)
            .map_or(document.len(), |next| start + marker.len() + next);
        let object = &document[start..next];
        let status = json::string_field(object, "status")?;
        if !matches!(
            status.as_str(),
            "MANUAL_PASS" | "MANUAL_FAIL" | "NOT_TESTED"
        ) {
            return Err(format!("manual receipt contains invalid status `{status}`"));
        }
        observations.push((
            json::string_field(object, "case_id")?,
            ManualObservation {
                status,
                note: json::string_field(object, "note").unwrap_or_default(),
            },
        ));
        offset = next;
    }
    Ok(observations)
}

fn render_manual_receipt(
    candidate: &Candidate,
    cases: &[ManualCase],
    observations: &BTreeMap<String, ManualObservation>,
) -> String {
    let windows_build = windows_build();
    let cpu = environment_value("PROCESSOR_IDENTIFIER");
    let mut output = format!(
        concat!(
            "{{\"schema_version\":1,",
            "\"source_commit\":\"{}\",",
            "\"exe_sha256\":\"{}\",",
            "\"zip_sha256\":\"{}\",",
            "\"version\":\"{}\",",
            "\"operator\":\"USER\",",
            "\"environment\":{{\"windows\":\"{}\",\"cpu\":\"{}\"}},",
            "\"cases\":["
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(&candidate.version),
        json::escape(&windows_build),
        json::escape(&cpu),
    );
    for (index, case) in cases.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        let observation = observations.get(&case.id);
        let status = observation.map_or("NOT_TESTED", |value| value.status.as_str());
        let note = observation.map_or("", |value| value.note.as_str());
        output.push_str(&format!(
            concat!(
                "{{\"case_id\":\"{}\",",
                "\"session\":\"{}\",",
                "\"guided_session\":\"{}\",",
                "\"tier\":\"{}\",",
                "\"status\":\"{}\",",
                "\"source_commit\":\"{}\",",
                "\"exe_sha256\":\"{}\",",
                "\"note\":\"{}\"}}"
            ),
            json::escape(&case.id),
            case.session.as_str(),
            guided::session_for_case(&case.id).map_or("", GuidedSession::as_str),
            case.tier.as_str(),
            status,
            json::escape(&candidate.source_commit),
            json::escape(&candidate.exe_sha256),
            json::escape(note),
        ));
    }
    output.push_str("]}\n");
    output
}

fn environment_value(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| "UNKNOWN".to_owned())
}

pub(super) fn windows_build() -> String {
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| environment_value("OS"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        ManualCase, ManualObservation, ManualTier, parse_manual_cases, render_manual_receipt,
        session_for_case, tier_for_case,
    };
    use crate::cli::ManualSession;
    use crate::qualification::receipt::Candidate;

    #[test]
    fn matrix_parser_exposes_all_cases_and_sessions() {
        let mut content = String::new();
        for number in 1..=44 {
            let mode = if matches!(number, 3 | 4 | 6..=10 | 13..=17 | 27..=33 | 44) {
                "Automated exact candidate"
            } else {
                "Manual"
            };
            content.push_str(&format!(
                "| P12-M{number:02} | action | {mode} | expected | NOT TESTED |\n"
            ));
        }
        let cases = parse_manual_cases(&content).expect("manual cases");
        assert_eq!(cases.len(), 24);
        assert_eq!(cases[0].session, ManualSession::M1);
        assert!(
            (1..=44)
                .filter(|number| !matches!(number, 3 | 4 | 6..=10 | 13..=17 | 27..=33 | 44))
                .all(|number| session_for_case(&format!("P12-M{number:02}")).is_ok())
        );
    }

    #[test]
    fn risk_tiers_match_the_approved_phase14_policy() {
        for number in 1..=33 {
            assert_eq!(
                tier_for_case(&format!("P12-M{number:02}")),
                Ok(ManualTier::A)
            );
        }
        for number in 34..=40 {
            assert_eq!(
                tier_for_case(&format!("P12-M{number:02}")),
                Ok(ManualTier::B)
            );
        }
        for number in 41..=44 {
            assert_eq!(
                tier_for_case(&format!("P12-M{number:02}")),
                Ok(ManualTier::C)
            );
        }
    }

    #[test]
    fn receipt_binds_version_windows_session_and_case_identity() {
        let candidate = Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        };
        let case = ManualCase {
            id: "P12-M01".to_owned(),
            action: "action".to_owned(),
            expected: "expected".to_owned(),
            session: ManualSession::M1,
            tier: ManualTier::A,
        };
        let observations = BTreeMap::from([(
            case.id.clone(),
            ManualObservation {
                status: "MANUAL_PASS".to_owned(),
                note: "observed".to_owned(),
            },
        )]);
        let receipt = render_manual_receipt(&candidate, &[case], &observations);
        for marker in [
            "\"version\":\"0.1.0\"",
            "\"windows\":",
            "\"case_id\":\"P12-M01\"",
            "\"session\":\"M1\"",
            "\"guided_session\":\"G1\"",
        ] {
            assert!(receipt.contains(marker), "missing receipt marker {marker}");
        }
    }
}
