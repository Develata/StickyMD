//! The macOS release gate, row by row, as a pure function of what was read: the resolved
//! candidate, the ledger (runs, and the USER's release receipts, waivers and download records),
//! and the source-test receipt (plan 11 #macos-release-gate, #macos-readiness-check). It
//! recomputes every verdict from recorded facts; no status field or exit code is taken on
//! trust, a waiver stands only for a waivable row that is NOT TESTED (never for a FAIL), and
//! nothing forces a row through.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::groups::{Group, Ledger, ledger};
use super::receipts::{self, Wanted};
use super::rows::{Class, ROWS};
use super::source_tests::{self, Outcome};
use crate::mac_ledger::record::{Built, Record};

/// The resolved candidate: the digest of its receipt, its two byte identities and its signing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub receipt_sha256: String,
    pub zip_sha256: String,
    pub built: Built,
    pub signing: String,
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
    /// R's test binaries and doc-tested libraries, listed on R by `cargo metadata` apart from the
    /// receipt (plan 11 #macos-readiness-check).
    pub targets: Read<&'a [source_tests::Target]>,
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
    let (tests, same_key) = match (&inputs.source_tests, &inputs.targets) {
        (Read::Present(receipt), Read::Present(targets)) => {
            let (tests, same_key) = source_tests::judge(receipt, inputs.release, targets);
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
        (Read::Missing(reason) | Read::Invalid(reason), _)
        | (_, Read::Missing(reason) | Read::Invalid(reason)) => (
            line("source-tests", Verdict::NotTested, reason.clone()),
            line("same-key", Verdict::NotTested, reason.clone()),
        ),
    };
    let download = download(inputs);
    let mut lines = vec![package.clone(), tests.clone(), same_key, download];
    for (row, class, _) in ROWS {
        let item = format!("P16-{row}");
        let judged = match class {
            Class::Source => Line {
                item,
                ..tests.clone()
            },
            Class::IgnoredSource => line(
                &item,
                Verdict::NotTested,
                "its test is #[ignore] (it mounts a disk image), so the source-test receipt does not run it; how it gets evidence on R awaits a decision (M5 report)",
            ),
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

/// A Manual row holds with the USER's release receipt for this candidate, unless a program
/// observed it fail on the candidate's bytes: a failed sub-item fails the row, and neither a
/// receipt nor a waiver stands for it (plan 11 #macos-release-gate).
fn manual(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    if let Ledger::Fail(reason) = ledger(records, row, &candidate.built, inputs.batch) {
        return line(item, Verdict::Fail, reason);
    }
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
/// candidate, every record names the candidate's signing, and all five ran on one macOS major
/// version. It cannot be waived for the first public package; a record of another candidate does
/// not count (plan 11 #macos-readiness-check).
fn download(inputs: &Inputs<'_>) -> Line {
    let item = "download-path";
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    let found = match receipts::downloads(records, &candidate.receipt_sha256) {
        Ok(found) => found,
        Err(missing) => {
            return line(
                item,
                Verdict::NotTested,
                format!(
                    "no USER download-path record for this candidate: {}",
                    missing.join(", ")
                ),
            );
        }
    };
    let mut majors = Vec::new();
    for (row, signing, system) in &found {
        if *signing != candidate.signing {
            return line(
                item,
                Verdict::NotTested,
                format!(
                    "{row} names signing {signing}, the candidate is {}",
                    candidate.signing
                ),
            );
        }
        match receipts::major_version(system) {
            Some(major) => majors.push(major),
            None => {
                return line(
                    item,
                    Verdict::NotTested,
                    format!("{row} names no macOS version: {system:?}"),
                );
            }
        }
    }
    majors.sort_unstable();
    majors.dedup();
    match majors.as_slice() {
        [major] => line(
            item,
            Verdict::Pass,
            format!("P18-D01-D05 recorded by the USER for this candidate on macOS {major}"),
        ),
        _ => line(
            item,
            Verdict::NotTested,
            format!("the five paths ran on different macOS major versions {majors:?}"),
        ),
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
    // Every member is judged before any verdict, so a recorded FAIL of a later member is never
    // hidden behind an earlier member's missing evidence (which a waiver could then cover).
    let judged: Vec<(&String, Ledger)> = group
        .iter()
        .map(|member| {
            (
                member,
                ledger(records, member, &candidate.built, inputs.batch),
            )
        })
        .collect();
    if let Some((_, Ledger::Fail(reason))) =
        judged.iter().find(|(_, l)| matches!(l, Ledger::Fail(_)))
    {
        return line(item, Verdict::Fail, reason.clone());
    }
    if let Some((_, Ledger::None(reason))) =
        judged.iter().find(|(_, l)| matches!(l, Ledger::None(_)))
    {
        return line(item, Verdict::NotTested, reason.clone());
    }
    let mut common: Option<Vec<Group>> = None;
    let mut evidence_of_row = String::new();
    for (member, judgement) in judged {
        if let Ledger::Clean(groups, evidence) = judgement {
            if member == row {
                evidence_of_row = evidence;
            }
            common = Some(match common {
                None => groups,
                Some(kept) => kept.into_iter().filter(|g| groups.contains(g)).collect(),
            });
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
