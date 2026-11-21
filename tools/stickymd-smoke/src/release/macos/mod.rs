//! The macOS release checks: the package check on a candidate ZIP, the local promotion and the
//! candidate resolver. The packager and the check itself are in `stickymd-package`.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

// The package inventory, listing and package check live in the packaging crate beside the
// packager; promotion and its resolver are verification and stay here.
#[cfg(target_os = "macos")]
pub(crate) use stickymd_package::macos::package;
pub(crate) use stickymd_package::macos::{content, listing};
#[cfg(target_os = "macos")]
pub(crate) mod promote;

use std::path::Path;

pub(super) use stickymd_package::macos::verify_command;

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
    let app = promote::resolve(root, release)?;
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
