//! The macOS release set: package inventory and README (portable), the bundle identity, and the
//! packager and SBOM generator that need macOS tools. The package check lives here too, beside the
//! packager whose output it checks; `stickymd-smoke` calls it on the workflow's and on the
//! promoted bytes.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

#[cfg(target_os = "macos")]
pub mod bundle;
// Pure; part of it is used only by the macOS-only packager and promotion.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub mod content;
pub mod listing;
#[cfg(target_os = "macos")]
pub mod package;
#[cfg(target_os = "macos")]
pub mod sbom;

use std::path::Path;

#[cfg(target_os = "macos")]
pub fn package_command(
    root: &Path,
    app: &Path,
    output: &Path,
    allow_dirty: bool,
) -> Result<(), String> {
    println!("{}", package::build(root, app, output, allow_dirty)?.json());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn package_command(_: &Path, _: &Path, _: &Path, _: bool) -> Result<(), String> {
    Err("the macOS package is built on macOS".to_owned())
}

#[cfg(target_os = "macos")]
pub fn sbom_command(root: &Path, zip: &Path) -> Result<(), String> {
    let published = sbom::publish(root, zip)?;
    println!(
        "{{\"sbom_sha256\":\"{}\",\"checksums_sha256\":\"{}\"}}",
        published.sbom_sha256, published.checksums_sha256
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn sbom_command(_: &Path, _: &Path) -> Result<(), String> {
    Err("the macOS SBOM is generated on macOS".to_owned())
}
