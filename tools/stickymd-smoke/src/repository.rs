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
    workspace_version_in(&manifest)
}

fn workspace_version_in(manifest: &str) -> Result<String, String> {
    // Inspect the repository's scalar workspace.package.version representation.
    // Cargo owns full TOML validation; unrelated dependency/metadata versions are
    // never a fallback for a missing or malformed workspace version.
    let mut in_workspace_package = false;
    let mut version = None;
    for line in manifest.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_workspace_package = line
                .split('#')
                .next()
                .is_some_and(|table| table.trim_end() == "[workspace.package]");
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !in_workspace_package || key.trim() != "version" {
            continue;
        }
        if version.is_some() {
            return Err("workspace version is duplicated".to_owned());
        }
        let value = value.trim_start();
        let quote = value.chars().next().ok_or("workspace version is empty")?;
        if !matches!(quote, '\'' | '"') {
            return Err("workspace version must be a quoted scalar".to_owned());
        }
        let (value, tail) = value[1..]
            .split_once(quote)
            .ok_or("workspace version has no closing quote")?;
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
            || (!tail.trim().is_empty() && !tail.trim_start().starts_with('#'))
        {
            return Err("workspace version is not a supported version scalar".to_owned());
        }
        version = Some(value.to_owned());
    }
    version.ok_or_else(|| "workspace version is missing".to_owned())
}

#[cfg(test)]
mod tests {
    use super::workspace_version_in;

    #[test]
    fn workspace_version_preserves_spacing_and_uses_the_workspace_table() {
        for field in [
            "version = \"0.1.0\"",
            "version=\"0.1.0\"",
            "version\t=\t\"0.1.0\"",
            "  version = '0.1.0' # release version",
        ] {
            let manifest = format!(
                "[workspace.metadata.fixture]\nversion = \"9.9.9\"\n[workspace.package] # identity\n{field}\n[workspace.dependencies.example]\nversion = \"2\"\n"
            );
            assert_eq!(workspace_version_in(&manifest).unwrap(), "0.1.0");
        }
    }

    #[test]
    fn missing_ambiguous_or_malformed_workspace_version_has_no_fallback() {
        for manifest in [
            "[package]\nversion = \"0.1.0\"",
            "[workspace.package]\n[workspace.dependencies.example]\nversion = \"2\"",
            "[workspace.package]\nversion = \"0.1.0\"\nversion = \"0.2.0\"",
            "[workspace.package]\nversion = \"0.1.0\" trailing",
            "[workspace.package]\nversion = \"\"",
        ] {
            assert!(workspace_version_in(manifest).is_err(), "{manifest}");
        }
    }
}
