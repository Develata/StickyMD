//! M5, the macOS readiness check (plan 11 #macos-release-gate, #macos-readiness-check): the
//! source tests on the release commit R and their receipt, and the row-by-row check of the
//! candidate, the ledger and that receipt. Judging is pure and tested on every host; reading the
//! candidate and running the tests happen on a Mac.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

mod judge;
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
    let (tests, same_key) = source_tests::judge(&receipt, release, &listed_targets(root)?);
    println!(
        "{{\"release_source\":\"{}\",\"source_tests\":\"{}\",\"same_key\":\"{}\"}}",
        receipt.release_source,
        label(&tests),
        label(&same_key)
    );
    match (tests, same_key) {
        (source_tests::Outcome::Pass, source_tests::Outcome::Pass) => Ok(()),
        (tests, same_key) => Err(format!(
            "the receipt does not establish the rows: source tests {tests:?}, same key {same_key:?}"
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
) -> Result<(), String> {
    use judge::Read;
    crate::integrity::validate_hex(release, 40, "release commit")?;
    let release = release.to_ascii_lowercase();
    if let Some(name) = batch.filter(|name| !crate::mac_ledger::is_batch(name)) {
        return Err(format!("batch {name:?} is not 1-64 of [A-Za-z0-9._-]"));
    }
    // The gate is judged with R's own tools (plan 11 #platform-artifact-reuse, after reuse): its
    // row table and rules are those of a clean checkout of R.
    let here = crate::evidence::SourceIdentity::capture(root);
    if here.commit.to_ascii_lowercase() != release || here.worktree_dirty {
        return Err(format!(
            "macOS readiness is checked on a clean checkout of {release}; HEAD is {} (dirty {})",
            here.commit, here.worktree_dirty
        ));
    }
    let matrix = std::fs::read_to_string(root.join("docs/acceptance-cases/phase-16.md"))
        .map_err(|error| format!("cannot read the Phase 16 matrix: {error}"))?;
    rows::check_against(&crate::governance::phase16_modes(&matrix)?)?;
    let candidate = match super::promote::receipt_present(root) {
        Ok(true) => match super::promote::resolve(root, &release) {
            Ok(resolved) => Read::Present(judge::Candidate {
                receipt_sha256: resolved.receipt_sha256,
                zip_sha256: resolved.zip_sha256,
                built: crate::mac_ledger::record::Built {
                    executable: resolved.executable_sha256,
                    bundle: resolved.bundle_sha256,
                },
                signing: resolved.signing,
            }),
            Err(error) => Read::Invalid(format!("the candidate does not resolve: {error}")),
        },
        Ok(false) => Read::Missing("no macOS candidate has been promoted".to_owned()),
        Err(error) => Read::Invalid(error),
    };
    let ledger_path = root
        .join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
        .join("records.jsonl");
    // The same checks the ledger's writers and `plan` apply: every line, every revocation's
    // target, and one system per VM image (plan 11 #phase-16-macos-host).
    let ledger = match std::fs::read_to_string(&ledger_path) {
        Ok(text) => match crate::mac_ledger::record::read_all(&text).and_then(|records| {
            crate::mac_ledger::decide::check_system(&records)?;
            Ok(records)
        }) {
            Ok(records) => Read::Present(records),
            Err(error) => Read::Invalid(format!("the ledger is unreadable: {error}")),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Read::Missing("no ledger has been recorded".to_owned())
        }
        Err(error) => Read::Invalid(format!("cannot read the ledger: {error}")),
    };
    // Read under the run lock (shared): a run in progress has retired the old receipt and has not
    // published its own, so it reads as not tested.
    let run_lock = source_tests::lock(root, false)?;
    let receipt = if run_lock.is_none() {
        Read::Missing("a source-test run is in progress".to_owned())
    } else {
        match std::fs::read_to_string(root.join(source_tests::RECEIPT)) {
            Ok(text) => match source_tests::parse(&text) {
                Ok(receipt) => Read::Present(receipt),
                Err(error) => Read::Invalid(format!("the source-test receipt is invalid: {error}")),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Read::Missing(
                "no source-test receipt: run release macos-source-tests on R".to_owned(),
            ),
            Err(error) => Read::Invalid(format!("cannot read the source-test receipt: {error}")),
        }
    };
    // R's test targets, listed apart from the receipt (plan 11 #macos-readiness-check).
    let targets = listed_targets(root);
    let targets = match targets {
        Ok(targets) => Read::Present(targets),
        Err(error) => Read::Invalid(format!("cannot list R's test targets: {error}")),
    };
    let inputs = judge::Inputs {
        release: &release,
        batch,
        candidate: borrow(&candidate),
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
    let mut ready = judge::ready(&lines);
    drop(run_lock);
    // The verdicts hold for the candidate read at the start; one promoted meanwhile is another.
    if let Read::Present(candidate) = &candidate {
        let still = super::promote::resolve(root, &release)
            .is_ok_and(|now| now.receipt_sha256 == candidate.receipt_sha256);
        if !still {
            eprintln!("the candidate changed or stopped resolving while it was checked");
            ready = false;
        }
    }
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
    let digest = match &candidate {
        Read::Present(candidate) => format!("\"{}\"", candidate.receipt_sha256),
        _ => "null".to_owned(),
    };
    println!(
        "{{\"release_source\":\"{release}\",\"candidate_receipt_sha256\":{digest},\"batch\":{},\"ready\":{ready},\"rows\":[\n{}\n]}}",
        batch.map_or_else(|| "null".to_owned(), |name| format!("\"{name}\"")),
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

#[cfg(target_os = "macos")]
fn borrow<T>(read: &judge::Read<T>) -> judge::Read<&T> {
    match read {
        judge::Read::Present(value) => judge::Read::Present(value),
        judge::Read::Missing(reason) => judge::Read::Missing(reason.clone()),
        judge::Read::Invalid(reason) => judge::Read::Invalid(reason.clone()),
    }
}
