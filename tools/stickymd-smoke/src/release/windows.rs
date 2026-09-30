//! Windows archive fact collection; validation remains in Rust.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::json::{self, Value};
use std::{path::Path, process::Command};

pub(super) fn archive(
    root: &Path,
    operation: &str,
    zip: &Path,
    extra: &[&str],
) -> Result<Value, String> {
    let mut arguments = vec![
        "-Operation",
        operation,
        "-ZipPath",
        zip.to_str().ok_or("ZIP path is not Unicode")?,
    ];
    arguments.extend_from_slice(extra);
    json::parse(&script(root, "archive-facts.ps1", &arguments)?)
}

pub(super) fn script(root: &Path, script: &str, arguments: &[&str]) -> Result<String, String> {
    let mut output = Vec::new();
    operation(
        root,
        Path::new("powershell.exe"),
        script,
        arguments,
        &mut output,
    )?;
    String::from_utf8(output)
        .map_err(|error| format!("Windows adapter facts are not UTF-8: {error}"))
}

pub(super) fn operation(
    root: &Path,
    shell: &Path,
    script: &str,
    arguments: &[&str],
    log: &mut dyn std::io::Write,
) -> Result<(), String> {
    let output = Command::new(shell)
        // Native intermediaries retain the caller's PowerShell edition-specific module path.
        // These adapters use built-ins only; let Windows PowerShell select its own modules.
        .env_remove("PSModulePath")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(root.join("tools/release").join(script))
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start Windows adapter {script}: {error}"))?;
    log.write_all(&output.stdout).map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Windows adapter {script} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if !output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

pub(super) fn names(root: &Path, zip: &Path) -> Result<Vec<String>, String> {
    archive(root, "List", zip, &[])?
        .array()?
        .iter()
        .map(|value| value.string().map(str::to_owned))
        .collect()
}

pub(super) fn read_entry(root: &Path, zip: &Path, index: usize) -> Result<String, String> {
    archive(root, "Read", zip, &["-EntryIndex", &index.to_string()])?
        .string()
        .map(str::to_owned)
}
