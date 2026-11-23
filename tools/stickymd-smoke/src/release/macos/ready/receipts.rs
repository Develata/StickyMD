//! The USER's release records for one candidate, as the gate counts them (plan 11
//! #macos-readiness-check): a release receipt, a waiver, or a download-path record holds when it
//! names this candidate's receipt digest and no revocation of the row for that digest follows it
//! in the ledger (records are appended where the USER confirms, so ledger order is the order of
//! confirmation). For I01-I03 only records of the batch being judged count, as for every other
//! result of those rows.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use crate::mac_ledger::decide::NEVER_REUSED;
use crate::mac_ledger::record::{Record, Revoked};
use crate::mac_ledger::release::{DOWNLOAD_ROWS, ReleaseKind, ReleaseRecord};

/// What to look for.
#[derive(Clone, Copy)]
pub(super) enum Wanted {
    Receipt,
    Waiver,
    Download,
}

impl Wanted {
    fn matches(self, kind: &ReleaseKind) -> bool {
        matches!(
            (self, kind),
            (Self::Receipt, ReleaseKind::Receipt)
                | (Self::Waiver, ReleaseKind::Waiver)
                | (Self::Download, ReleaseKind::Download { .. })
        )
    }
}

/// The latest standing record of `wanted` for `row` and `candidate`.
pub(super) fn standing<'a>(
    records: &'a [Record],
    wanted: Wanted,
    row: &str,
    candidate: &str,
    batch: Option<&str>,
) -> Option<&'a ReleaseRecord> {
    let in_batch = |record: &ReleaseRecord| {
        !NEVER_REUSED.contains(&row) || (batch.is_some() && record.batch.as_deref() == batch)
    };
    records
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, record)| match record {
            Record::Release(release)
                if release.row == row
                    && release.candidate == candidate
                    && wanted.matches(&release.kind)
                    && in_batch(release) =>
            {
                let revoked_after = records[at + 1..].iter().any(|later| {
                    matches!(later, Record::Revocation { row: r, revoked: Revoked::Candidate(d), .. }
                        if r == row && d == candidate)
                });
                (!revoked_after).then_some(release)
            }
            _ => None,
        })
}

/// The download-path rows without a standing record for `candidate`.
pub(super) fn missing_downloads(records: &[Record], candidate: &str) -> Vec<&'static str> {
    DOWNLOAD_ROWS
        .into_iter()
        .filter(|row| standing(records, Wanted::Download, row, candidate, None).is_none())
        .collect()
}
