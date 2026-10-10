//! Tier-aware manual receipt validation for exact Phase 14 candidates. The receipt itself
//! is validated by `manual_receipt::validated_cases`, the same check resuming uses.

use std::path::Path;

use super::manual_receipt::{MANUAL_RECEIPT, ManualTier, validated_cases};
use super::receipt::Candidate;
use super::{decisions, json, receipt};

pub(super) fn check(
    root: &Path,
    candidate: &Candidate,
    decisions: &[decisions::Decision],
    automated_ok: bool,
    blockers: &mut Vec<String>,
) {
    let cases = match receipt::read_receipt(&root.join(MANUAL_RECEIPT))
        .and_then(|text| json::parse_object(&text))
        .and_then(|document| validated_cases(&document, candidate))
    {
        Ok(cases) => cases,
        Err(error) => {
            blockers.push(format!("mandatory manual acceptance receipt: {error}"));
            return;
        }
    };
    let observed_ids: Vec<_> = cases.iter().map(|case| case.id.as_str()).collect();
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
        match case.status.as_str() {
            "MANUAL_FAIL" => blockers.push(format!("manual acceptance {} failed", case.id)),
            "NOT_TESTED" => {
                if let Some(blocker) = not_tested_blocker(
                    case.tier,
                    &case.id,
                    &candidate.version,
                    automated_ok,
                    decisions,
                ) {
                    blockers.push(blocker);
                }
            }
            // `validated_cases` admits only the three statuses.
            _ => {}
        }
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

#[cfg(test)]
mod tests {
    use super::not_tested_blocker;
    use crate::qualification::decisions::Decision;
    use crate::qualification::manual_receipt::ManualTier;

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
