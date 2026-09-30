//! Portable external SHA-256 adapter; Windows uses the in-process CNG backend.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::{path::Path, process::Command};

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    let output = Command::new("sha256sum")
        .arg("--")
        .arg(path)
        .output()
        .map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!("SHA-256 command failed for {}", path.display()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // GNU sha256sum prefixes an escaped filename record with a backslash.
    let hash = text
        .strip_prefix('\\')
        .unwrap_or(&text)
        .split_once(' ')
        .map(|(hash, _)| hash)
        .ok_or("SHA-256 output has no digest field")?;
    super::validate_sha256(hash, "SHA-256")?;
    Ok(hash.to_ascii_lowercase())
}
