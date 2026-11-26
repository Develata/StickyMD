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

/// One standing download-path record: its row, signing, system and outcome.
pub(super) type Download<'a> = (&'static str, &'a str, &'a str, Option<&'a str>);

/// The standing download-path records for `candidate`, or the rows that have none.
pub(super) fn downloads<'a>(
    records: &'a [Record],
    candidate: &str,
) -> Result<Vec<Download<'a>>, Vec<&'static str>> {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for row in DOWNLOAD_ROWS {
        match standing(records, Wanted::Download, row, candidate, None).map(|r| &r.kind) {
            Some(ReleaseKind::Download {
                signing,
                system,
                outcome,
            }) => {
                found.push((row, signing.as_str(), system.as_str(), outcome.as_deref()));
            }
            _ => missing.push(row),
        }
    }
    if missing.is_empty() {
        Ok(found)
    } else {
        Err(missing)
    }
}

/// The macOS major version in a `sw_vers` text (`ProductVersion: 27.2` gives 27): exactly one
/// `ProductVersion` label in the whole text, followed by one to three dot-separated numbers
/// (review round 3, S7; review round 4, BLOCKER 5: a label at the end, cut off from its value,
/// is still a label; fix round, BLOCKER 6: so is one with its value attached, as in
/// `ProductVersion:28.0`; M5 gate review B3: the label is counted wherever it starts, so a
/// quoted `>ProductVersion:` is one too; fix-round review R1-4: but the one label must start a
/// field name, so `TargetProductVersion:` is none). A format check of what was recorded, not a
/// proof of the machine.
pub(super) fn major_version(system: &str) -> Option<u32> {
    const LABEL: &str = "ProductVersion:";
    let (before, rest) = system.split_once(LABEL)?;
    if rest.contains(LABEL) {
        return None;
    }
    // A field name is letters, digits and `_`; the label starts one only after anything else.
    if before
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_')
    {
        return None;
    }
    // The value is the first word after the label, attached to it or not.
    let version = rest.split_whitespace().next()?;
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = (1..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    numeric.then(|| parts[0].parse().ok()).flatten()
}
