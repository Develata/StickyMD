//! The macOS release set: package inventory and README (portable), and the packager and package
//! check that need macOS tools.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

// Pure; part of it is used only by the macOS-only packager and promotion.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod content;
pub(crate) mod listing;
#[cfg(target_os = "macos")]
pub(crate) mod package;
#[cfg(target_os = "macos")]
pub(crate) mod promote;
#[cfg(target_os = "macos")]
pub(crate) mod sbom;

use std::path::Path;

#[cfg(target_os = "macos")]
pub(super) fn package_command(
    root: &Path,
    app: &Path,
    output: &Path,
    allow_dirty: bool,
) -> Result<(), String> {
    println!("{}", package::build(root, app, output, allow_dirty)?.json());
    Ok(())
}

#[cfg(target_os = "macos")]
pub(super) fn verify_command(root: &Path, zip: &Path, source: Option<&str>) -> Result<(), String> {
    let version = crate::repository::workspace_version(root)?;
    println!("{}", package::verify(zip, source, &version)?.json());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn package_command(_: &Path, _: &Path, _: &Path, _: bool) -> Result<(), String> {
    Err("the macOS package is built on macOS".to_owned())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn verify_command(_: &Path, _: &Path, _: Option<&str>) -> Result<(), String> {
    Err("the macOS package is checked on macOS".to_owned())
}

#[cfg(target_os = "macos")]
pub(super) fn sbom_command(root: &Path, zip: &Path) -> Result<(), String> {
    let published = sbom::publish(root, zip)?;
    println!(
        "{{\"sbom_sha256\":\"{}\",\"checksums_sha256\":\"{}\"}}",
        published.sbom_sha256, published.checksums_sha256
    );
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn sbom_command(_: &Path, _: &Path) -> Result<(), String> {
    Err("the macOS SBOM is generated on macOS".to_owned())
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
