//! The macOS release gate, row by row, as a pure function of what was read: the resolved
//! candidate, the ledger (runs, and the USER's release receipts, waivers and download records),
//! and the source-test receipt (plan 11 #macos-release-gate, #macos-readiness-check). It
//! recomputes every verdict from recorded facts; no status field or exit code is taken on
//! trust, a waiver stands only for a waivable row that is NOT TESTED (never for a FAIL), and
//! nothing forces a row through.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::receipts::{self, Wanted};
use super::rows::{Class, ROWS};
use super::source_tests::{self, Outcome};
use crate::mac_ledger::decide::NEVER_REUSED;
use crate::mac_ledger::record::{Built, Identity, Record, Revoked};

/// The resolved candidate: the digest of its receipt and its two byte identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub receipt_sha256: String,
    pub zip_sha256: String,
    pub built: Built,
}

/// One input as read: present, absent (nothing recorded yet), or unusable.
#[derive(Clone, Debug)]
pub(super) enum Read<T> {
    Present(T),
    Missing(String),
    Invalid(String),
}

pub(super) struct Inputs<'a> {
    pub release: &'a str,
    pub batch: Option<&'a str>,
    pub candidate: Read<&'a Candidate>,
    pub ledger: Read<&'a [Record]>,
    pub source_tests: Read<&'a source_tests::Receipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Verdict {
    Pass,
    Fail,
    NotTested,
    /// NOT TESTED, with the USER's waiver bound to this candidate (plan 11 #macos-release-gate).
    Waived,
    /// Not a macOS release row (a Windows regression); it neither passes nor blocks.
    Excluded,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotTested => "NOT_TESTED",
            Self::Waived => "WAIVED",
            Self::Excluded => "EXCLUDED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Line {
    /// A gate class (`package`, `source-tests`, `same-key`, `download-path`) or a `P16-` row.
    pub item: String,
    pub verdict: Verdict,
    pub detail: String,
}

fn line(item: &str, verdict: Verdict, detail: impl Into<String>) -> Line {
    Line {
        item: item.to_owned(),
        verdict,
        detail: detail.into(),
    }
}

fn from_outcome(item: &str, outcome: Outcome, pass: &str) -> Line {
    match outcome {
        Outcome::Pass => line(item, Verdict::Pass, pass),
        Outcome::Fail(reason) => line(item, Verdict::Fail, reason),
        Outcome::NotTested(reason) => line(item, Verdict::NotTested, reason),
    }
}

/// Whether every row holds: each is PASS, waived or excluded.
pub(super) fn ready(lines: &[Line]) -> bool {
    lines.iter().all(|line| {
        matches!(
            line.verdict,
            Verdict::Pass | Verdict::Waived | Verdict::Excluded
        )
    })
}

pub(super) fn judge(inputs: &Inputs<'_>) -> Vec<Line> {
    let package = match &inputs.candidate {
        Read::Present(candidate) => line(
            "package",
            Verdict::Pass,
            format!(
                "the staged candidate matches receipt {} (signing AD_HOC)",
                candidate.receipt_sha256
            ),
        ),
        Read::Missing(reason) => line("package", Verdict::NotTested, reason.clone()),
        Read::Invalid(reason) => line("package", Verdict::Fail, reason.clone()),
    };
    let (tests, same_key) = match &inputs.source_tests {
        Read::Present(receipt) => {
            let (tests, same_key) = source_tests::judge(receipt, inputs.release);
            (
                from_outcome(
                    "source-tests",
                    tests,
                    "the eight packages' default tests passed on R",
                ),
                from_outcome(
                    "same-key",
                    same_key,
                    "both same-key tests (two volume kinds) passed on R",
                ),
            )
        }
        Read::Missing(reason) | Read::Invalid(reason) => (
            line("source-tests", Verdict::NotTested, reason.clone()),
            line("same-key", Verdict::NotTested, reason.clone()),
        ),
    };
    let download = download(inputs);
    let mut lines = vec![package.clone(), tests.clone(), same_key, download];
    for (row, class) in ROWS {
        let item = format!("P16-{row}");
        let judged = match class {
            Class::Source => Line {
                item,
                ..tests.clone()
            },
            Class::Package => Line {
                item,
                ..package.clone()
            },
            Class::Gui => waived(inputs, gui(inputs, &item, row), row),
            Class::Translocation => translocation(inputs, &item, row),
            Class::Manual => waived(inputs, manual(inputs, &item, row), row),
            Class::Resources => waived(
                inputs,
                line(
                    &item,
                    Verdict::NotTested,
                    "no evidence binds both byte identities: mac-resources records only the executable and is not in the ledger",
                ),
                row,
            ),
            Class::Windows => line(
                &item,
                Verdict::Excluded,
                "a Windows regression; Windows readiness judges it",
            ),
        };
        lines.push(judged);
    }
    lines
}

/// A NOT TESTED line of a waivable row becomes WAIVED when the USER's waiver for this candidate
/// stands; any other line is left as it is.
fn waived(inputs: &Inputs<'_>, judged: Line, row: &str) -> Line {
    if judged.verdict != Verdict::NotTested || !super::rows::waivable(row) {
        return judged;
    }
    let Ok((records, candidate)) = evidence(inputs, &judged.item) else {
        return judged;
    };
    match receipts::standing(
        records,
        Wanted::Waiver,
        row,
        &candidate.receipt_sha256,
        inputs.batch,
    ) {
        Some(waiver) => Line {
            verdict: Verdict::Waived,
            detail: format!(
                "USER waiver for this candidate ({:?}); without it: {}",
                waiver.quote, judged.detail
            ),
            ..judged
        },
        None => judged,
    }
}

/// A Manual row holds with the USER's release receipt for this candidate.
fn manual(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    match receipts::standing(
        records,
        Wanted::Receipt,
        row,
        &candidate.receipt_sha256,
        inputs.batch,
    ) {
        Some(receipt) => line(
            item,
            Verdict::Pass,
            format!("USER release receipt ({:?})", receipt.quote),
        ),
        None => line(
            item,
            Verdict::NotTested,
            "no USER release receipt for this candidate (mac-ledger receipt --release-source)",
        ),
    }
}

/// The real download path holds when each of P18-D01-D05 has the USER's record for this
/// candidate. It cannot be waived for the first public package; a record of another candidate
/// does not count (plan 11 #macos-readiness-check).
fn download(inputs: &Inputs<'_>) -> Line {
    let (records, candidate) = match evidence(inputs, "download-path") {
        Ok(found) => found,
        Err(line) => return line,
    };
    let missing = receipts::missing_downloads(records, &candidate.receipt_sha256);
    if missing.is_empty() {
        line(
            "download-path",
            Verdict::Pass,
            "P18-D01-D05 recorded by the USER for this candidate",
        )
    } else {
        line(
            "download-path",
            Verdict::NotTested,
            format!(
                "no USER download-path record for this candidate: {}",
                missing.join(", ")
            ),
        )
    }
}

/// The ledger and candidate both present, or the line saying which is not.
fn evidence<'a>(inputs: &'a Inputs<'_>, item: &str) -> Result<(&'a [Record], &'a Candidate), Line> {
    let candidate = match &inputs.candidate {
        Read::Present(candidate) => *candidate,
        Read::Missing(_) | Read::Invalid(_) => {
            return Err(line(
                item,
                Verdict::NotTested,
                "no resolved candidate to bind the evidence to",
            ));
        }
    };
    match &inputs.ledger {
        Read::Present(records) => Ok((records, candidate)),
        Read::Missing(reason) | Read::Invalid(reason) => {
            Err(line(item, Verdict::NotTested, reason.clone()))
        }
    }
}

/// The ledger's verdict on `row` for the candidate's bytes.
enum Ledger {
    /// A clean group: its identities, and the latest PASS's evidence.
    Clean(Vec<Identity>, String),
    Fail(String),
    None(String),
}

/// Records of `row` on exactly the candidate's bytes (both digests) and, for the never-reused
/// rows, of `batch`, grouped by identity. A group is clean when it holds a PASS of the row and
/// no FAIL, NOT_TESTED or revoked PASS of it: neither run start nor import order proves which
/// came first (plan 11 #phase-16-macos-host), so any such record spoils the group.
fn ledger(records: &[Record], row: &str, built: &Built, batch: Option<&str>) -> Ledger {
    let in_scope = |record: &&Record| match record {
        Record::Automated {
            row: r,
            built: b,
            batch: rb,
            ..
        } => {
            r == row
                && b == built
                && (!NEVER_REUSED.contains(&row) || (batch.is_some() && rb.as_deref() == batch))
        }
        _ => false,
    };
    let scoped: Vec<&Record> = records.iter().filter(in_scope).collect();
    let revoked = |digest: &str| {
        records.iter().any(|r| {
            matches!(r, Record::Revocation { row: rr, revoked: Revoked::Evidence(d), .. }
                if rr == row && d == digest)
        })
    };
    let mut clean = Vec::new();
    let mut latest = String::new();
    for record in &scoped {
        let Record::Automated { identity, .. } = record else {
            continue;
        };
        if clean.contains(identity) {
            continue;
        }
        let group: Vec<&&Record> = scoped
            .iter()
            .filter(|r| matches!(r, Record::Automated { identity: i, .. } if i == identity))
            .collect();
        let spoiled = group.iter().any(|r| {
            matches!(r, Record::Automated { verdict, evidence_sha256, .. }
                if verdict != "PASS" || revoked(evidence_sha256))
        });
        if !spoiled {
            if let Some(Record::Automated { evidence, .. }) = group.last().map(|r| **r) {
                latest = evidence.clone();
            }
            clean.push(identity.clone());
        }
    }
    if !clean.is_empty() {
        return Ledger::Clean(clean, latest);
    }
    if scoped
        .iter()
        .any(|r| matches!(r, Record::Automated { verdict, .. } if verdict == "FAIL"))
    {
        return Ledger::Fail(format!(
            "a FAIL of P16-{row} is recorded on the candidate's bytes"
        ));
    }
    let same_executable = records.iter().any(|r| {
        matches!(r, Record::Automated { row: rr, built: b, verdict, .. }
            if rr == row && verdict == "PASS" && b.executable == built.executable && b.bundle != built.bundle)
    });
    Ledger::None(if !scoped.is_empty() {
        format!(
            "every record of P16-{row} on the candidate's bytes is NOT_TESTED or revoked, or sits beside one"
        )
    } else if same_executable {
        format!(
            "a PASS of P16-{row} names the candidate's executable but another bundle; it does not count"
        )
    } else if NEVER_REUSED.contains(&row) && batch.is_none() {
        format!("P16-{row} counts only the batch named with --batch")
    } else {
        format!("no PASS of P16-{row} is recorded on the candidate's bytes")
    })
}

fn gui(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    // W12 counts only with its three real logout cases, in one identity (plan 11).
    let (parent, parts) = crate::mac_ledger::decide::composite();
    let group: Vec<String> = if row == parent {
        std::iter::once(row.to_owned()).chain(parts).collect()
    } else {
        vec![row.to_owned()]
    };
    let mut common: Option<Vec<Identity>> = None;
    let mut evidence_of_row = String::new();
    for member in &group {
        match ledger(records, member, &candidate.built, inputs.batch) {
            Ledger::Clean(identities, evidence) => {
                if member == row {
                    evidence_of_row = evidence;
                }
                common = Some(match common {
                    None => identities,
                    Some(kept) => kept
                        .into_iter()
                        .filter(|i| identities.contains(i))
                        .collect(),
                });
            }
            Ledger::Fail(reason) => return line(item, Verdict::Fail, reason),
            Ledger::None(reason) => return line(item, Verdict::NotTested, reason),
        }
    }
    if common.is_some_and(|identities| !identities.is_empty()) {
        line(
            item,
            Verdict::Pass,
            format!("ledger PASS on the candidate's bytes ({evidence_of_row})"),
        )
    } else {
        line(
            item,
            Verdict::NotTested,
            format!("P16-{row} and its logout cases have no clean identity in common"),
        )
    }
}

/// R03 must be judged on the release ZIP itself; the probe still zips the `.app`, so a PASS on
/// the candidate's bundle is not that ZIP (plan 11 #macos-release-gate). A FAIL on these bytes is
/// an observed violation and counts.
fn translocation(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    let found = match ledger(records, row, &candidate.built, inputs.batch) {
        Ledger::Fail(reason) => return line(item, Verdict::Fail, reason),
        Ledger::Clean(..) => "a PASS on the candidate's bundle exists, but",
        Ledger::None(_) => "no evidence binds it:",
    };
    line(
        item,
        Verdict::NotTested,
        format!(
            "{found} R03 must run on the candidate ZIP {} and the probe zips the .app itself (M5 report)",
            candidate.zip_sha256
        ),
    )
}
