//! M5, the macOS readiness check (plan 11 #macos-release-gate, #macos-readiness-check): the
//! source tests on the release commit R and their receipt, and the row-by-row check of the
//! candidate, the ledger and that receipt. Judging is pure and tested on every host; reading the
//! candidate and running the tests happen on a Mac.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

mod judge;
mod rows;
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
    let (tests, same_key) = source_tests::judge(&receipt, release);
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
    let matrix = std::fs::read_to_string(root.join("docs/acceptance-cases/phase-16.md"))
        .map_err(|error| format!("cannot read the Phase 16 matrix: {error}"))?;
    rows::check_against(&crate::governance::phase16_modes(&matrix)?)?;
    let candidate = if super::promote::receipt_present(root) {
        match super::promote::resolve(root, &release) {
            Ok(resolved) => Read::Present(judge::Candidate {
                receipt_sha256: resolved.receipt_sha256,
                zip_sha256: resolved.zip_sha256,
                built: crate::mac_ledger::record::Built {
                    executable: resolved.executable_sha256,
                    bundle: resolved.bundle_sha256,
                },
            }),
            Err(error) => Read::Invalid(format!("the candidate does not resolve: {error}")),
        }
    } else {
        Read::Missing("no macOS candidate has been promoted".to_owned())
    };
    let ledger_path = root
        .join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
        .join("records.jsonl");
    let ledger = match std::fs::read_to_string(&ledger_path) {
        Ok(text) => match crate::mac_ledger::record::read_all(&text) {
            Ok(records) => Read::Present(records),
            Err(error) => Read::Invalid(format!("the ledger is unreadable: {error}")),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Read::Missing("no ledger has been recorded".to_owned())
        }
        Err(error) => Read::Invalid(format!("cannot read the ledger: {error}")),
    };
    let receipt = match std::fs::read_to_string(root.join(source_tests::RECEIPT)) {
        Ok(text) => match source_tests::parse(&text) {
            Ok(receipt) => Read::Present(receipt),
            Err(error) => Read::Invalid(format!("the source-test receipt is invalid: {error}")),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Read::Missing(
            "no source-test receipt: run release macos-source-tests on R".to_owned(),
        ),
        Err(error) => Read::Invalid(format!("cannot read the source-test receipt: {error}")),
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
    };
    let lines = judge::judge(&inputs);
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
            .filter(|line| !matches!(line.verdict, judge::Verdict::Pass | judge::Verdict::Excluded))
            .count();
        Err(format!("macOS readiness: NOT READY ({open} rows do not hold)"))
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
