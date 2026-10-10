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
    let document = json::parse_object(&receipt::read_receipt(&path)?)?;
    for case in validated_cases(&document, candidate)? {
        let Some(observation) = observations.get_mut(&case.id) else {
            return Err(format!(
                "manual receipt contains unknown case `{}`",
                case.id
            ));
        };
        *observation = ManualObservation {
            status: case.status,
            note: case.note,
        };
    }
    Ok(observations)
}

/// One case of a manual receipt that is valid for its candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RecordedCase {
    pub(super) id: String,
    pub(super) tier: ManualTier,
    /// `MANUAL_PASS`, `MANUAL_FAIL` or `NOT_TESTED`.
    pub(super) status: String,
    pub(super) note: String,
}

/// The cases of a manual receipt, provided the whole receipt is valid for `candidate`:
/// schema 1; the candidate's source, EXE, ZIP and version at the top and its source and
/// EXE on every case; a known Windows build; each case's registered tier and session; a
/// known status; and no case twice. Resuming and readiness accept a receipt only through
/// this check, so resuming can never regenerate provenance that readiness would refuse.
pub(super) fn validated_cases(
    document: &json::Value,
    candidate: &Candidate,
) -> Result<Vec<RecordedCase>, String> {
    let schema = json::u64_field(document, "schema_version")?;
    if schema != 1 {
        return Err(format!("manual receipt schema is {schema}, expected 1"));
    }
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
        ("zip_sha256", candidate.zip_sha256.as_str()),
        ("version", candidate.version.as_str()),
    ] {
        let actual = json::string_field(document, key)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: manual {key} is {actual}, expected {expected}"
            ));
        }
    }
    let windows = json::string_field(json::object_field(document, "environment")?, "windows")?;
    if !super::windows_build::is_known(&windows) {
        return Err(format!(
            "manual receipt Windows build `{windows}` names no version and build"
        ));
    }
    let mut cases: Vec<RecordedCase> = Vec::new();
    for object in json::objects(document, "cases")? {
        let id = json::string_field(object, "case_id")?;
        for (key, expected) in [
            ("source_commit", candidate.source_commit.as_str()),
            ("exe_sha256", candidate.exe_sha256.as_str()),
        ] {
            let actual = json::string_field(object, key)?;
            if actual != expected {
                return Err(format!(
                    "STALE RECEIPT: manual case {id} {key} is {actual}, expected {expected}"
                ));
            }
        }
        let tier = tier_for_case(&id)?;
        let recorded_tier = json::string_field(object, "tier")?;
        if recorded_tier != tier.as_str() {
            return Err(format!(
                "manual case {id} tier is {recorded_tier}, expected {}",
                tier.as_str()
            ));
        }
        let session = session_for_case(&id)?;
        let recorded_session = json::string_field(object, "session")?;
        if recorded_session != session.as_str() {
            return Err(format!(
                "manual case {id} session is {recorded_session}, expected {}",
                session.as_str()
            ));
        }
        let status = json::string_field(object, "status")?;
        if !matches!(
            status.as_str(),
            "MANUAL_PASS" | "MANUAL_FAIL" | "NOT_TESTED"
        ) {
            return Err(format!("manual case {id} has invalid status `{status}`"));
        }
        // A note is optional; one that is present must be a string.
        let note = if json::has_field(object, "note") {
            json::string_field(object, "note")?
        } else {
            String::new()
        };
        if cases.iter().any(|case| case.id == id) {
            return Err(format!("manual receipt records case `{id}` more than once"));
        }
        cases.push(RecordedCase {
            id,
            tier,
            status,
            note,
        });
    }
    Ok(cases)
}

fn render_manual_receipt(
    candidate: &Candidate,
    cases: &[ManualCase],
    observations: &BTreeMap<String, ManualObservation>,
) -> String {
    let windows_build = super::windows_build::current();
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        MANUAL_RECEIPT, ManualCase, ManualObservation, ManualTier, load_observations,
        parse_manual_cases, render_manual_receipt, session_for_case, tier_for_case,
        validated_cases,
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
            target: "x86_64-pc-windows-msvc".to_owned(),
            workflow_run_id: 1,
            workflow_attempt: 1,
            artifact_id: 2,
            artifact_name: crate::qualification::receipt::RELEASE_ARTIFACT_NAME.to_owned(),
            zip_name: "StickyMD-0.1.0-windows-x64-portable.zip".to_owned(),
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
        let receipt = render_manual_receipt(&candidate, std::slice::from_ref(&case), &observations);

        // The build string comes from this host; pin it so the test means the same anywhere.
        let start = receipt.find("\"windows\":\"").unwrap() + "\"windows\":\"".len();
        let end = start + receipt[start..].find('"').unwrap();
        let receipt = format!(
            "{}Microsoft Windows 10.0.26200.9457{}",
            &receipt[..start],
            &receipt[end..]
        );

        // Resuming and readiness accept the same receipts: valid ones once per case, and
        // nothing whose provenance readiness would refuse.
        let parse = |text: &str| crate::qualification::json::parse_object(text).unwrap();
        assert_eq!(
            validated_cases(&parse(&receipt), &candidate).unwrap().len(),
            1
        );
        // Only the case changes; the top-level identity stays valid, so the case check fires.
        let case_tail = format!("\"exe_sha256\":\"{}\",\"note\"", candidate.exe_sha256);
        let case_identity = format!(
            "\"source_commit\":\"{}\",{case_tail}",
            candidate.source_commit
        );
        let foreign_source = receipt.replace(
            &case_identity,
            &format!("\"source_commit\":\"{}\",{case_tail}", "f".repeat(40)),
        );
        let foreign_exe = receipt.replace(
            &case_tail,
            &format!("\"exe_sha256\":\"{}\",\"note\"", "0".repeat(64)),
        );
        assert!(foreign_source != receipt && foreign_exe != receipt);
        for (mutated, reason) in [
            (foreign_source, "manual case P12-M01 source_commit"),
            (foreign_exe, "manual case P12-M01 exe_sha256"),
            (
                receipt.replace("\"schema_version\":1", "\"schema_version\":9"),
                "schema",
            ),
            (
                receipt.replace(
                    "\"windows\":\"Microsoft Windows 10.0.26200.9457\"",
                    "\"windows\":\"UNKNOWN\"",
                ),
                "Windows build",
            ),
            (
                receipt.replace(
                    "\"windows\":\"Microsoft Windows 10.0.26200.9457\"",
                    "\"windows\":\"Windows_NT\"",
                ),
                "Windows build",
            ),
            (receipt.replace("\"tier\":\"A\"", "\"tier\":\"C\""), "tier"),
            (
                receipt.replace("\"session\":\"M1\"", "\"session\":\"M2\""),
                "session",
            ),
        ] {
            let error = validated_cases(&parse(&mutated), &candidate).unwrap_err();
            assert!(error.contains(reason), "{reason}: {error}");
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-manual-resume-{nonce}"));
        let case_object = receipt
            .split_once("\"cases\":[")
            .unwrap()
            .1
            .trim_end()
            .trim_end_matches("]}")
            .to_owned();
        let duplicated = receipt.replacen(&case_object, &format!("{case_object},{case_object}"), 1);
        crate::atomic_evidence::write(&root.join(MANUAL_RECEIPT), duplicated.as_bytes()).unwrap();
        let error = load_observations(&root, &candidate, std::slice::from_ref(&case)).unwrap_err();
        assert!(error.contains("more than once"), "{error}");
        // Resuming refuses before anything is persisted: the receipt stays as it was.
        assert_eq!(
            std::fs::read_to_string(root.join(MANUAL_RECEIPT)).unwrap(),
            duplicated
        );
        std::fs::remove_dir_all(&root).unwrap();
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
