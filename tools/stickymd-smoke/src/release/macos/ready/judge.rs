//! The macOS release gate, row by row, as a pure function of what was read: the resolved
//! candidate, the ledger (runs, and the USER's release receipts, waivers and download records),
//! and the source-test receipt (plan 11 #macos-release-gate, #macos-readiness-check). It
//! recomputes every verdict from recorded facts; no status field or exit code is taken on
//! trust, a waiver stands only for a waivable row that is NOT TESTED (never for a FAIL), and
//! nothing forces a row through.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::groups::{Ledger, Scope, failure, joint};
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
    /// The macOS major version this release's download paths must have run on (decision (b)).
    pub macos_major: u32,
    /// The SHA-256 of R's `mac-gui` driver, lowercase (USER 2026-11-24, `--driver`).
    pub driver: &'a str,
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
    let (tests, same_key, s06) = match (&inputs.source_tests, &inputs.targets) {
        (Read::Present(receipt), Read::Present(targets)) => {
            let verdicts = source_tests::judge(receipt, inputs.release, targets);
            (
                from_outcome(
                    "source-tests",
                    verdicts.tests,
                    "the eight packages' default tests passed on R, test by test",
                ),
                from_outcome(
                    "same-key",
                    verdicts.same_key,
                    "both same-key tests passed on R: the case-insensitive temporary volume and the case-sensitive image",
                ),
                from_outcome(
                    "P16-S06",
                    verdicts.s06,
                    "the exFAT test passed on R in its explicit run",
                ),
            )
        }
        // Without R's target list the receipt cannot be shown complete, but a failure it records
        // is still a failure (review round 3, S2); the explicit runs do not need the list.
        (Read::Present(receipt), Read::Missing(reason) | Read::Invalid(reason)) => {
            let verdicts = source_tests::judge(receipt, inputs.release, &[]);
            let tests = match verdicts.tests {
                source_tests::Outcome::Fail(failure) => {
                    line("source-tests", Verdict::Fail, failure)
                }
                _ => line("source-tests", Verdict::NotTested, reason.clone()),
            };
            (
                tests,
                from_outcome(
                    "same-key",
                    verdicts.same_key,
                    "both same-key tests passed on R: the case-insensitive temporary volume and the case-sensitive image",
                ),
                from_outcome(
                    "P16-S06",
                    verdicts.s06,
                    "the exFAT test passed on R in its explicit run",
                ),
            )
        }
        (Read::Missing(reason) | Read::Invalid(reason), _) => (
            line("source-tests", Verdict::NotTested, reason.clone()),
            line("same-key", Verdict::NotTested, reason.clone()),
            line("P16-S06", Verdict::NotTested, reason.clone()),
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
            // Its test is #[ignore]; the receipt's explicit run stands for it (decision (a)).
            Class::IgnoredSource => Line {
                item,
                ..s06.clone()
            },
            Class::Package => Line {
                item,
                ..package.clone()
            },
            // A mac-resources row is judged from the ledger as a mac-gui row is.
            Class::Gui | Class::Resources => waived(inputs, gui(inputs, &item, row), row),
            Class::Translocation => translocation(inputs, &item, row),
            Class::Manual => waived(inputs, manual(inputs, &item, row), row),
            Class::Comparison => waived(
                inputs,
                line(
                    &item,
                    Verdict::NotTested,
                    "no producer: V03 alternates a baseline through mac-latency, which is not in the ledger and has no release rule (plan 11 #macos-readiness-check)",
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
    if judged.verdict != Verdict::NotTested {
        return judged;
    }
    let Ok((records, candidate)) = evidence(inputs, &judged.item) else {
        return judged;
    };
    // The one place a NOT TESTED line meets the ledger's failures: any FAIL of the row, or of a
    // row it needs, on the candidate's bytes under any identity makes it FAIL, so neither missing
    // evidence nor a waiver can stand for a failure observed (plan 11 #macos-readiness-check).
    let scope = scope(inputs, records, candidate);
    if let Some(reason) = members(row)
        .iter()
        .find_map(|member| failure(&scope, member))
    {
        return line(&judged.item, Verdict::Fail, reason);
    }
    if !super::rows::waivable(row) {
        return judged;
    }
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
    // A program FAIL on the candidate's bytes under any identity fails the row, whatever another
    // identity's PASS or a USER receipt says (plan 11 #macos-readiness-check).
    if let Some(reason) = failure(&scope(inputs, records, candidate), row) {
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
/// candidate, every record names the candidate's signing and ran on the macOS major version the
/// gate is told to check (`--macos-major`), and P18-D04 and P18-D05 record their observed outcome
/// (USER 2026-11-24, decision (b)). A record without that outcome has no path to PASS; a record of
/// another candidate does not count, and it cannot be waived for the first public package
/// (plan 11 #macos-readiness-check).
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
    for (row, signing, system, outcome) in &found {
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
            Some(major) if major == inputs.macos_major => {}
            Some(major) => {
                return line(
                    item,
                    Verdict::NotTested,
                    format!(
                        "{row} ran on macOS {major}, the release is checked on macOS {}",
                        inputs.macos_major
                    ),
                );
            }
            None => {
                return line(
                    item,
                    Verdict::NotTested,
                    format!("{row} names no macOS version: {system:?}"),
                );
            }
        }
        let wanted = crate::mac_ledger::release::outcomes(row);
        let recorded = match (wanted, outcome) {
            (None, None) => true,
            (Some(allowed), Some(found)) => allowed.contains(found),
            _ => false,
        };
        if !recorded {
            return line(
                item,
                Verdict::NotTested,
                format!("{row} records no valid outcome ({outcome:?})"),
            );
        }
    }
    line(
        item,
        Verdict::Pass,
        format!(
            "P18-D01-D05 recorded by the USER for this candidate on macOS {}",
            inputs.macos_major
        ),
    )
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

/// The rows a row's ledger verdict needs: W12 counts only with its three real logout cases
/// (plan 11 #phase-16-macos-host); every other row is itself.
fn members(row: &str) -> Vec<String> {
    let (parent, parts) = crate::mac_ledger::decide::composite();
    if row == parent {
        std::iter::once(row.to_owned()).chain(parts).collect()
    } else {
        vec![row.to_owned()]
    }
}

fn scope<'a>(inputs: &'a Inputs<'_>, records: &'a [Record], candidate: &'a Candidate) -> Scope<'a> {
    Scope {
        records,
        built: &candidate.built,
        batch: inputs.batch,
        zip: None,
        driver: inputs.driver,
    }
}

/// A `mac-gui` row: a group in which it and every row it needs hold a clean PASS. The rows are
/// judged together inside each group, so a FAIL in every complete group is a FAIL (plan 11
/// #macos-readiness-check).
fn gui(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    match joint(&scope(inputs, records, candidate), &members(row)) {
        Ledger::Clean(evidence) => line(
            item,
            Verdict::Pass,
            format!("ledger PASS on the candidate's bytes ({evidence})"),
        ),
        Ledger::Fail(reason) => line(item, Verdict::Fail, reason),
        Ledger::None(reason) => line(item, Verdict::NotTested, reason),
    }
}

/// R03 is judged on the release ZIP itself (plan 11 #macos-release-gate): a clean group counts
/// only when a PASS in it opened the candidate's ZIP (`mac-gui --zip`) under R's driver, with
/// "nothing written" read through the write watch; one from a ZIP zipped from the `.app`, even
/// around the same bundle, or recorded without the watch (an older driver's), does not. It cannot
/// be waived. A FAIL on these bytes in this batch, through any ZIP, is an observed violation and
/// counts first (review round 3, B1 and S1).
fn translocation(inputs: &Inputs<'_>, item: &str, row: &str) -> Line {
    let (records, candidate) = match evidence(inputs, item) {
        Ok(found) => found,
        Err(line) => return line,
    };
    let scope = Scope {
        zip: Some(&candidate.zip_sha256),
        ..scope(inputs, records, candidate)
    };
    // A FAIL recorded on the candidate's bytes in this batch, under any identity or ZIP, stays a
    // FAIL, whatever another group shows (review round 3, S1).
    if let Some(reason) = failure(&scope, row) {
        return line(item, Verdict::Fail, reason);
    }
    match joint(&scope, &[row.to_owned()]) {
        Ledger::Clean(evidence) => line(
            item,
            Verdict::Pass,
            format!(
                "ledger PASS on the candidate ZIP {} under the write watch ({evidence})",
                candidate.zip_sha256
            ),
        ),
        Ledger::Fail(reason) => line(item, Verdict::Fail, reason),
        Ledger::None(reason) => line(item, Verdict::NotTested, reason),
    }
}
