//! Tier-aware manual receipt validation for exact Phase 14 candidates.

use std::path::Path;

use super::receipt::Candidate;
use super::{decisions, json, receipt};

const MANUAL_RECEIPT: &str = "dist/evidence/manual-acceptance.json";

pub(super) fn check(
    root: &Path,
    candidate: &Candidate,
    decisions: &[decisions::Decision],
    automated_ok: bool,
    blockers: &mut Vec<String>,
) {
    let document = match receipt::read_receipt(&root.join(MANUAL_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("mandatory manual acceptance receipt: {error}"));
            return;
        }
    };
    match json::u64_field(&document, "schema_version") {
        Ok(1) => {}
        Ok(version) => blockers.push(format!("manual receipt schema is {version}, expected 1")),
        Err(error) => blockers.push(format!("manual receipt schema: {error}")),
    }
    check_identity(&document, candidate, blockers);
    match json::string_field(&document, "version") {
        Ok(version) if version == candidate.version => {}
        Ok(version) => blockers.push(format!(
            "STALE RECEIPT: manual version is {version}, expected {}",
            candidate.version
        )),
        Err(error) => blockers.push(format!("manual version: {error}")),
    }
    match json::string_field(&document, "windows") {
        Ok(build) if !build.trim().is_empty() && build != "UNKNOWN" => {}
        Ok(_) => blockers.push("manual receipt Windows build is unavailable".to_owned()),
        Err(error) => blockers.push(format!("manual Windows build: {error}")),
    }
    match json::string_field(&document, "zip_sha256") {
        Ok(hash) if hash == candidate.zip_sha256 => {}
        Ok(hash) => blockers.push(format!(
            "STALE RECEIPT: manual zip_sha256 is {hash}, expected {}",
            candidate.zip_sha256
        )),
        Err(error) => blockers.push(format!("manual ZIP identity: {error}")),
    }
    let cases = match case_statuses(&document) {
        Ok(cases) if !cases.is_empty() => cases,
        Ok(_) => {
            blockers.push("manual receipt contains no cases".to_owned());
            return;
        }
        Err(error) => {
            blockers.push(format!("manual receipt statuses: {error}"));
            return;
        }
    };
    let observed_ids: Vec<_> = cases.iter().map(|case| case.case_id.as_str()).collect();
    let expected_ids: Vec<_> = (1..=44)
        .map(|number| format!("P12-M{number:02}"))
        .filter(|id| super::exact_groups::group_for_phase12_case(id).is_none())
        .collect();
    if observed_ids != expected_ids.iter().map(String::as_str).collect::<Vec<_>>() {
        blockers.push(format!(
            "manual receipt cases must contain the 24 non-G3/G4/G5 P12 manual cases; observed {observed_ids:?}"
        ));
        return;
    }
    for case in cases {
        if case.source_commit != candidate.source_commit || case.exe_sha256 != candidate.exe_sha256
        {
            blockers.push(format!(
                "STALE RECEIPT: {} carries a different source/EXE identity",
                case.case_id
            ));
            continue;
        }
        let expected_tier = tier(&case.case_id).map_or("INVALID", ManualTier::as_str);
        if case.tier != expected_tier {
            blockers.push(format!(
                "manual acceptance {} tier is {}, expected {expected_tier}",
                case.case_id, case.tier
            ));
            continue;
        }
        if !matches!(case.session.as_str(), "M1" | "M2" | "M3" | "M4" | "M5") {
            blockers.push(format!(
                "manual acceptance {} has invalid session {}",
                case.case_id, case.session
            ));
            continue;
        }
        match case.status.as_str() {
            "MANUAL_PASS" => {}
            "MANUAL_FAIL" => blockers.push(format!("manual acceptance {} failed", case.case_id)),
            "NOT_TESTED" => {
                let tier = tier(&case.case_id).unwrap_or(ManualTier::A);
                if let Some(blocker) = not_tested_blocker(
                    tier,
                    &case.case_id,
                    &candidate.version,
                    automated_ok,
                    decisions,
                ) {
                    blockers.push(blocker);
                }
            }
            _ => blockers.push(format!(
                "manual acceptance {} contains invalid status {}",
                case.case_id, case.status
            )),
        }
    }
}

fn check_identity(document: &str, candidate: &Candidate, blockers: &mut Vec<String>) {
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
    ] {
        match json::string_field(document, key) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => blockers.push(format!(
                "STALE RECEIPT: manual {key} is {actual}, expected {expected}"
            )),
            Err(error) => blockers.push(format!("manual {key}: {error}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManualTier {
    A,
    B,
    C,
}

impl ManualTier {
    const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

fn tier(case_id: &str) -> Result<ManualTier, String> {
    let number = case_id
        .strip_prefix("P12-M")
        .ok_or_else(|| format!("invalid manual case ID `{case_id}`"))?
        .parse::<u8>()
        .map_err(|error| format!("invalid manual case ID `{case_id}`: {error}"))?;
    match number {
        1..=33 => Ok(ManualTier::A),
        34..=40 => Ok(ManualTier::B),
        41..=44 => Ok(ManualTier::C),
        _ => Err(format!("manual case `{case_id}` is out of range")),
    }
}

fn not_tested_blocker(
    tier: ManualTier,
    case_id: &str,
    version: &str,
    automated_ok: bool,
    decisions: &[decisions::Decision],
) -> Option<String> {
    let case_waiver = format!("WAIVER-{case_id}");
    let tier_b_waiver = format!("WAIVER-TIER-B-v{version}");
    let waived = decisions::status(decisions, &case_waiver) == Some("USER APPROVED")
        || (tier == ManualTier::B
            && decisions::status(decisions, &tier_b_waiver) == Some("USER APPROVED"));
    match tier {
        ManualTier::A | ManualTier::B if !waived => Some(format!(
            "Tier {} manual acceptance {case_id} is NOT_TESTED without an exact-bound USER waiver",
            tier.as_str()
        )),
        ManualTier::C if !automated_ok => Some(format!(
            "Tier C manual acceptance {case_id} is NOT_TESTED while automated coverage is not fully PASSED"
        )),
        ManualTier::A | ManualTier::B | ManualTier::C => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManualObservation {
    case_id: String,
    status: String,
    source_commit: String,
    exe_sha256: String,
    session: String,
    tier: String,
}

fn case_statuses(document: &str) -> Result<Vec<ManualObservation>, String> {
    let marker = "\"case_id\":";
    let mut cases = Vec::new();
    let mut offset = 0;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative;
        let next = document[start + marker.len()..]
            .find(marker)
            .map_or(document.len(), |next| start + marker.len() + next);
        let object = &document[start..next];
        cases.push(ManualObservation {
            case_id: json::string_field(object, "case_id")?,
            status: json::string_field(object, "status")?,
            source_commit: json::string_field(object, "source_commit")?,
            exe_sha256: json::string_field(object, "exe_sha256")?,
            session: json::string_field(object, "session")?,
            tier: json::string_field(object, "tier")?,
        });
        offset = next;
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::{ManualTier, not_tested_blocker};
    use crate::qualification::decisions::Decision;

    #[test]
    fn tiers_enforce_waiver_and_version_binding() {
        let none = Vec::new();
        assert!(not_tested_blocker(ManualTier::A, "P12-M01", "0.1.0", true, &none).is_some());
        assert!(not_tested_blocker(ManualTier::C, "P12-M41", "0.1.0", true, &none).is_none());
        assert!(not_tested_blocker(ManualTier::C, "P12-M41", "0.1.0", false, &none).is_some());

        let tier_b_waiver = vec![Decision {
            key: "WAIVER-TIER-B-v0.1.0".to_owned(),
            status: "USER APPROVED".to_owned(),
            evidence: "USER exact group waiver".to_owned(),
        }];
        assert!(
            not_tested_blocker(ManualTier::B, "P12-M34", "0.1.0", true, &tier_b_waiver).is_none()
        );
        assert!(
            not_tested_blocker(ManualTier::B, "P12-M34", "0.1.1", true, &tier_b_waiver).is_some()
        );
    }
}
