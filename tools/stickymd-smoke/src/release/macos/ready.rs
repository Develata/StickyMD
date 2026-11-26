//! M5, the macOS readiness check (plan 11 #macos-release-gate, #macos-readiness-check): the
//! source tests on the release commit R and their receipt, and the row-by-row check of the
//! candidate, the ledger and that receipt. Judging is pure and tested on every host; reading the
//! candidate and running the tests happen on a Mac.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

#[cfg(target_os = "macos")]
mod candidate;
mod groups;
/// The source identity; promotion requires it too (M5 gate review B2).
pub(super) mod identity;
mod judge;
mod promotion;
mod receipts;
mod rows;
pub(crate) use rows::{receipt_row, waivable};
mod source_tests;
#[cfg(test)]
mod tests;

#[cfg(target_os = "macos")]
use std::path::Path;

/// `release macos-source-tests`: runs the tests on a clean checkout of R and writes the receipt;
/// fails unless the receipt it wrote judges PASS on both rows it carries.
#[cfg(target_os = "macos")]
pub(in super::super) fn source_tests_command(root: &Path, release: &str) -> Result<(), String> {
    let receipt = source_tests::run::run(root, release)?;
    let verdicts = source_tests::judge(&receipt, release, &listed_targets(root)?);
    println!(
        "{{\"release_source\":\"{}\",\"source_tests\":\"{}\",\"same_key\":\"{}\",\"s06\":\"{}\"}}",
        receipt.release_source,
        label(&verdicts.tests),
        label(&verdicts.same_key),
        label(&verdicts.s06)
    );
    match verdicts {
        source_tests::Verdicts {
            tests: source_tests::Outcome::Pass,
            same_key: source_tests::Outcome::Pass,
            s06: source_tests::Outcome::Pass,
        } => Ok(()),
        verdicts => Err(format!(
            "the receipt does not establish its rows: {verdicts:?}"
        )),
    }
}

/// R's test binaries and doc-tested libraries from `cargo metadata` on the checkout, listed
/// apart from any receipt (plan 11 #macos-readiness-check).
#[cfg(target_os = "macos")]
fn listed_targets(root: &Path) -> Result<Vec<source_tests::Target>, String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--locked"])
        .current_dir(root)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".to_owned());
    }
    source_tests::output::listed_targets(
        std::str::from_utf8(&output.stdout).map_err(|_| "cargo metadata is not UTF-8")?,
    )
}

#[cfg(target_os = "macos")]
fn label(outcome: &source_tests::Outcome) -> &'static str {
    match outcome {
        source_tests::Outcome::Pass => "PASS",
        source_tests::Outcome::Fail(_) => "FAIL",
        source_tests::Outcome::NotTested(_) => "NOT_TESTED",
    }
}

/// `release macos-ready`: read-only; prints every row and succeeds only when all hold.
#[cfg(target_os = "macos")]
pub(in super::super) fn ready_command(
    root: &Path,
    release: &str,
    batch: Option<&str>,
    macos_major: u32,
    driver: &Path,
) -> Result<(), String> {
    use judge::Read;
    crate::integrity::validate_hex(release, 40, "release commit")?;
    let release = release.to_ascii_lowercase();
    if let Some(name) = batch.filter(|name| !crate::mac_ledger::is_batch(name)) {
        return Err(format!("batch {name:?} is not 1-64 of [A-Za-z0-9._-]"));
    }
    // The gate is judged with R's own tools (plan 11 #platform-artifact-reuse, after reuse): its
    // row table and rules are those of a clean checkout of R, read as committed.
    let start = identity::require(root, &release, "macos-ready")?;
    // Only R's own driver's runs can hold a `mac-gui` PASS (USER 2026-11-24): the same rule as
    // `mac-ledger plan`, which reuses a PASS only for the current driver. A `--driver` that cannot
    // be hashed is judged like a wrong one, so every row is still printed, the ledger's FAILs
    // among them (pre-freeze item 5).
    let driver = crate::integrity::sha256(driver)
        .map(|digest| digest.to_ascii_lowercase())
        .map_err(|error| format!("cannot hash the driver {}: {error}", driver.display()));
    // R must be one the release authorization can name, before any row is judged, with or
    // without a candidate (review round 2, SHOULD_FIX 5).
    super::promote::check_release(root, &release)?;
    let matrix = std::fs::read_to_string(root.join("docs/acceptance-cases/phase-16.md"))
        .map_err(|error| format!("cannot read the Phase 16 matrix: {error}"))?;
    rows::check_against(&crate::governance::phase16_modes(&matrix)?)?;
    // One snapshot: the source-test receipt under the run lock and the ledger under its writers'
    // lock, both held until the verdicts are made, so no run or record lands between the reads.
    let run_lock = source_tests::lock_for_reading(root)?;
    let receipt = match &run_lock {
        source_tests::Reading::Busy => Read::Missing("a source-test run is in progress".to_owned()),
        source_tests::Reading::Free { held, .. } => source_tests::read_receipt(root, *held)?,
    };
    // R's driver is the one the run on R built (review round 4, BLOCKER 1): `--driver` must be
    // that file, or no `mac-gui` run counts as R's; a FAIL still counts whatever the driver.
    let driver_rule = judge::r_driver(
        driver.as_deref().map_err(String::as_str),
        borrow(&receipt),
        &release,
    );
    let ledger_path = root
        .join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
        .join("records.jsonl");
    // The same checks the ledger's writers and `plan` apply: every line, every revocation's
    // target, and one system per VM image (plan 11 #phase-16-macos-host).
    let (ledger, _ledger_lock) = match crate::mac_ledger::read_whole(root, &ledger_path) {
        Ok((Some(text), held)) => (
            match crate::mac_ledger::record::read_all(&text).and_then(|records| {
                crate::mac_ledger::decide::check_system(&records)?;
                Ok(records)
            }) {
                Ok(records) => Read::Present(records),
                Err(error) => Read::Invalid(format!("the ledger is unreadable: {error}")),
            },
            held,
        ),
        Ok((None, held)) => (
            Read::Missing("no ledger has been recorded".to_owned()),
            held,
        ),
        Err(error) => (
            Read::Invalid(format!("cannot read the ledger: {error}")),
            None,
        ),
    };
    let first = candidate::read(root, &release);
    // R's test targets, listed apart from the receipt (plan 11 #macos-readiness-check).
    let targets = match listed_targets(root) {
        Ok(targets) => Read::Present(targets),
        Err(error) => Read::Invalid(format!("cannot list R's test targets: {error}")),
    };
    let inputs = judge::Inputs {
        release: &release,
        batch,
        macos_major,
        driver: driver_rule.as_deref().map_err(String::as_str),
        candidate: borrow(&first.candidate),
        promotion: borrow(&first.promotion),
        ledger: match &ledger {
            Read::Present(records) => Read::Present(records.as_slice()),
            Read::Missing(reason) => Read::Missing(reason.clone()),
            Read::Invalid(reason) => Read::Invalid(reason.clone()),
        },
        source_tests: borrow(&receipt),
        targets: match &targets {
            Read::Present(targets) => Read::Present(targets.as_slice()),
            Read::Missing(reason) => Read::Missing(reason.clone()),
            Read::Invalid(reason) => Read::Invalid(reason.clone()),
        },
    };
    let lines = judge::judge(&inputs);
    // The verdicts stand only for what was read: the same checkout and the same candidate at
    // the end, or nothing is printed as a verdict (review round 2, finding 9, SHOULD_FIX 2).
    // The candidate's whole state (absent, not resolving, or resolving to this receipt, and the
    // promotion-check record) must be the one judged; a promotion or repair meanwhile voids this
    // check (review round 3, S6; M5 gate review B2). It is read under the promotion lock, held
    // until the verdicts are printed, and the checkout after it (M5 gate review B1).
    let _promotion_lock = candidate::confirm(root, &release, &first, &start)?;
    drop(run_lock);
    let ready = judge::ready(&lines);
    let rows: Vec<String> = lines
        .iter()
        .map(|line| {
            format!(
                "{{\"item\":\"{}\",\"verdict\":\"{}\",\"detail\":\"{}\"}}",
                line.item,
                line.verdict.label(),
                crate::evidence::escape_json(&line.detail)
            )
        })
        .collect();
    let digest = match &first.candidate {
        Read::Present(candidate) => format!("\"{}\"", candidate.receipt_sha256),
        _ => "null".to_owned(),
    };
    println!(
        "{{\"release_source\":\"{release}\",\"candidate_receipt_sha256\":{digest},\"batch\":{},\"macos_major\":{macos_major},\"driver_sha256\":{},\"ready\":{ready},\"rows\":[\n{}\n]}}",
        batch.map_or_else(|| "null".to_owned(), |name| format!("\"{name}\"")),
        driver.map_or_else(|_| "null".to_owned(), |digest| format!("\"{digest}\"")),
        rows.join(",\n")
    );
    if ready {
        Ok(())
    } else {
        let open = lines
            .iter()
            .filter(|line| {
                !matches!(
                    line.verdict,
                    judge::Verdict::Pass | judge::Verdict::Waived | judge::Verdict::Excluded
                )
            })
            .count();
        Err(format!(
            "macOS readiness: NOT READY ({open} rows do not hold)"
        ))
    }
}

fn borrow<T>(read: &judge::Read<T>) -> judge::Read<&T> {
    match read {
        judge::Read::Present(value) => judge::Read::Present(value),
        judge::Read::Missing(reason) => judge::Read::Missing(reason.clone()),
        judge::Read::Invalid(reason) => judge::Read::Invalid(reason.clone()),
    }
}
