//! The macOS release checks: the package check on a candidate ZIP, the local promotion, the
//! candidate resolver and the readiness check (M5). The packager and the package check itself
//! are in `stickymd-package`.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

// The package inventory, listing and package check live in the packaging crate beside the
// packager; promotion and its resolver are verification and stay here.
#[cfg(target_os = "macos")]
pub(crate) use stickymd_package::macos::package;
pub(crate) use stickymd_package::macos::{content, listing};
#[cfg(target_os = "macos")]
pub(crate) mod promote;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod ready;
#[cfg(target_os = "macos")]
pub(super) use ready::{ready_command, source_tests_command};
#[cfg(any(target_os = "macos", test))]
pub(crate) use ready::{receipt_row, waivable};

use std::path::Path;

pub(super) use stickymd_package::macos::verify_command;

/// The downloaded macOS set of the workflow's `verify-artifact` job: exactly the three files,
/// consistent with each other, built from `source` (plan 11 #platform-artifact-reuse).
#[cfg(target_os = "macos")]
pub(super) fn verify_artifact_command(
    root: &Path,
    directory: &Path,
    source: &str,
) -> Result<(), String> {
    let set = promote::verify_set(root, directory)?;
    if set.facts.source_commit != source {
        return Err(format!(
            "the package names source commit {}, expected {source}",
            set.facts.source_commit
        ));
    }
    println!(
        "{{\"zip\":\"{}\",\"zip_sha256\":\"{}\",\"sbom_sha256\":\"{}\",\"source_commit\":\"{}\"}}",
        crate::evidence::escape_json(&set.zip_name),
        set.zip_sha256,
        set.sbom_sha256,
        set.facts.source_commit
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_artifact_command(_: &Path, _: &Path, _: &str) -> Result<(), String> {
    Err("the macOS artifact is checked on macOS".to_owned())
}

#[cfg(target_os = "macos")]
pub(super) fn promote_command(
    root: &Path,
    run_id: u64,
    attempt: u64,
    release: &str,
    zip: &Path,
) -> Result<(), String> {
    let request = promote::Request {
        run_id,
        attempt,
        release,
        zip,
    };
    print!("{}", promote::promote(root, &request)?);
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn candidate_command(root: &Path, release: &str) -> Result<(), String> {
    let app = promote::resolve(root, release)?.app;
    println!(
        "{{\"app\":\"{}\"}}",
        crate::evidence::escape_json(&app.display().to_string())
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn promote_command(_: &Path, _: u64, _: u64, _: &str, _: &Path) -> Result<(), String> {
    Err("the macOS candidate is promoted on macOS".to_owned())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn candidate_command(_: &Path, _: &str) -> Result<(), String> {
    Err("the macOS candidate is resolved on macOS".to_owned())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn source_tests_command(_: &Path, _: &str) -> Result<(), String> {
    Err("the source tests on R for the macOS release run on macOS".to_owned())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn ready_command(_: &Path, _: &str, _: Option<&str>) -> Result<(), String> {
    Err("macOS readiness is checked on macOS".to_owned())
}
