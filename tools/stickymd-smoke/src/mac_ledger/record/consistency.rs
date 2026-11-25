//! Ledger consistency across lines: what a record may follow.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::{Record, Revoked, label};

/// Whether `record` may follow `earlier`: no second record of the same evidence for a row, and a
/// revocation only of a result already in the ledger. Run order is not checked: wall-clock starts
/// cannot prove it, so the reuse rule does not depend on it (`decide::plan`).
pub(in crate::mac_ledger) fn consistent_with(
    earlier: &[Record],
    record: &Record,
) -> Result<(), String> {
    match record {
        Record::Automated {
            row,
            evidence_sha256,
            ..
        } => {
            let replayed = earlier.iter().any(|r| {
                matches!(r, Record::Automated { row: r_row, evidence_sha256: digest, .. }
                    if r_row == row && digest == evidence_sha256)
            });
            if replayed {
                return Err(format!(
                    "P16-{row} from evidence {evidence_sha256} is already recorded; a replay cannot stand for a new run"
                ));
            }
        }
        Record::Revocation { row, revoked, .. } => {
            let held = earlier.iter().any(|r| match (r, revoked) {
                (
                    Record::Automated {
                        row: r_row,
                        verdict,
                        evidence_sha256,
                        ..
                    },
                    Revoked::Evidence(digest),
                ) => r_row == row && verdict == "PASS" && evidence_sha256 == digest,
                (
                    Record::Manual {
                        row: r_row,
                        inputs: Some(held),
                        ..
                    },
                    Revoked::Inputs(inputs),
                ) => r_row == row && held == inputs,
                (Record::Release(release), Revoked::Candidate(digest)) => {
                    &release.row == row && &release.candidate == digest
                }
                _ => false,
            });
            if !held {
                return Err(format!(
                    "the revocation of {} names no result recorded before it",
                    label(row)
                ));
            }
        }
        Record::Manual { .. } | Record::Release(_) => {}
    }
    Ok(())
}
