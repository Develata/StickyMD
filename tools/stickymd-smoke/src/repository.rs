//! Read-only repository facts shared by development and qualification tooling.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::process::Command;

pub(crate) fn command_text(
    root: &Path,
    program: &str,
    arguments: &[&str],
) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `{program}`: {error}"))?;
    if !output.status.success() {
        return Err(format!("`{program} {}` failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("`{program}` output is not UTF-8: {error}"))
}

pub(crate) fn workspace_version(root: &Path) -> Result<String, String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read Cargo.toml: {error}"))?;
    manifest
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("version = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .map(str::to_owned)
        .ok_or_else(|| "workspace version is missing".to_owned())
}
