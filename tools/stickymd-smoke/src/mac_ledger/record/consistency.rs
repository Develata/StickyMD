//! Ledger consistency across lines: what a record may follow.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::{Record, Revoked, label};
use crate::mac_ledger::producers::RESOURCE_ROWS;

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

/// Across the whole ledger (M5 gate review C2): the records of one `mac-resources` evidence are
/// exactly one R01 and one R02 (plan 11 #phase-16-macos-host: one evidence gives both rows), the
/// two alike in everything but row and verdict, as the importer writes them: one identity, the
/// same bytes, evidence, run start and recording. A ledger holding only one of them (cut short
/// or spliced) could otherwise let a lost FAIL be waived. Every program result is grouped, so a
/// third one naming that evidence, of any row, is refused too (gate review round 2, R2-5). A run
/// of `mac-gui` may record one row; a revocation is not a result.
pub(in crate::mac_ledger) fn resource_pairs(records: &[Record]) -> Result<(), String> {
    // Each program result, by its evidence digest.
    let mut groups: std::collections::BTreeMap<&str, Vec<(&str, &Record)>> =
        std::collections::BTreeMap::new();
    for record in records {
        if let Record::Automated {
            row,
            evidence_sha256,
            ..
        } = record
        {
            groups
                .entry(evidence_sha256)
                .or_default()
                .push((row.as_str(), record));
        }
    }
    for (digest, group) in groups {
        if !group.iter().any(|(row, _)| RESOURCE_ROWS.contains(row)) {
            continue;
        }
        let [(first_row, first), (second_row, second)] = group.as_slice() else {
            return Err(incomplete(digest, &group));
        };
        if !matches!((*first_row, *second_row), ("R01", "R02") | ("R02", "R01")) {
            return Err(incomplete(digest, &group));
        }
        // Alike but for row and verdict: the second with the first's.
        let mut aligned = (*second).clone();
        if let (
            Record::Automated { row, verdict, .. },
            Record::Automated {
                verdict: first_verdict,
                ..
            },
        ) = (&mut aligned, first)
        {
            *row = (*first_row).to_owned();
            verdict.clone_from(first_verdict);
        }
        if aligned != **first {
            return Err(format!(
                "the P16-R01 and P16-R02 lines of the mac-resources evidence {digest} are not of one identity, run and recording"
            ));
        }
    }
    Ok(())
}

/// A resource evidence's group that is not its two rows, with the rows it holds.
fn incomplete(digest: &str, group: &[(&str, &Record)]) -> String {
    let rows: Vec<String> = group.iter().map(|(row, _)| label(row)).collect();
    format!(
        "the mac-resources evidence {digest} is recorded as {}, not both P16-R01 and P16-R02",
        rows.join(", ")
    )
}
