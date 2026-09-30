//! Fresh byte identity and private host/session inputs for diagnostic cohorts.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use crate::qualification::{module_ledger::fingerprint, receipt};
use std::{path::Path, time::Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub(super) fingerprint: String,
    pub(super) source: String,
    pub(super) executable: String,
    pub(super) harness: String,
    pub(super) inputs: String,
    pub(super) environment: String,
    pub(super) dirty: bool,
}

#[cfg(windows)]
pub(super) fn host() -> Result<String, String> {
    // Never persist these identifiers or command output: only their digest enters the cache key.
    command(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
$ErrorActionPreference = 'Stop'
$os = Get-CimInstance Win32_OperatingSystem
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$cpu = @(Get-CimInstance Win32_Processor | Sort-Object DeviceID | Select-Object ProcessorId, Name, NumberOfLogicalProcessors)
if (-not $os.BuildNumber -or -not $os.LastBootUpTime -or $os.TotalVisibleMemorySize -le 0 -or
    -not [Environment]::MachineName -or -not $identity.User.Value -or $cpu.Count -eq 0 -or
    @($cpu | Where-Object { -not $_.Name -or $_.NumberOfLogicalProcessors -le 0 }).Count -ne 0) {
    throw 'incomplete host identity'
}
[ordered]@{
  host = [Environment]::MachineName
  build = $os.BuildNumber
  boot = $os.LastBootUpTime.ToUniversalTime().ToString('o')
  ram = $os.TotalVisibleMemorySize
  user = $identity.User.Value
  cpu = $cpu
} | ConvertTo-Json -Depth 4 -Compress
"#,
        ],
    )
}

#[cfg(not(windows))]
pub(super) fn host() -> Result<String, String> {
    Err("diagnostic reuse requires Windows".into())
}

pub(super) fn capture(root: &Path, host: &str) -> Result<Identity, String> {
    let started = Instant::now();
    let source = receipt::command_text(root, "git", &["rev-parse", "HEAD"])?;
    let dirty = !receipt::command_text(
        root,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?
    .is_empty();
    let git_seconds = started.elapsed().as_secs_f64();
    let timer = Instant::now();
    let executable = crate::integrity::sha256(&crate::qualification::release_executable(root)?)?;
    let harness = crate::integrity::sha256(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    let artifact_seconds = timer.elapsed().as_secs_f64();
    let timer = Instant::now();
    let mut material = Vec::new();
    for value in [
        "diagnostic resource environment v2",
        host,
        &root
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
        &desktop()?,
        &power_scheme()?,
        &std::thread::available_parallelism()
            .map_err(|e| e.to_string())?
            .get()
            .to_string(),
    ] {
        part(&mut material, value.as_bytes());
    }
    let mut settings: Vec<_> = std::env::vars_os()
        .filter(|(key, _)| {
            let key = key.to_string_lossy().to_ascii_uppercase();
            key.starts_with("STICKYMD_")
                || key.starts_with("RUST")
                || key.starts_with("CARGO_")
                || ["PATH", "TEMP", "TMP", "SYSTEMROOT", "PROCESSOR_IDENTIFIER"]
                    .contains(&key.as_str())
        })
        .collect();
    settings.sort();
    for (key, value) in settings {
        part(&mut material, key.as_encoded_bytes());
        part(&mut material, value.as_encoded_bytes());
    }
    let environment = super::digest::bytes(&material)?;
    let environment_seconds = timer.elapsed().as_secs_f64();
    let timer = Instant::now();
    let inputs = fingerprint::workspace_inputs(root, &[])?;
    let input_seconds = timer.elapsed().as_secs_f64();
    let mut material = Vec::new();
    for value in [
        "diagnostic resource units v2",
        &source,
        &executable,
        &harness,
        &inputs,
        &environment,
    ] {
        part(&mut material, value.as_bytes());
    }
    let fingerprint = super::digest::bytes(&material)?;
    eprintln!(
        "RESOURCE_IDENTITY git_seconds={git_seconds:.6} artifact_hash_seconds={artifact_seconds:.6} environment_seconds={environment_seconds:.6} input_hash_seconds={input_seconds:.6} total_seconds={:.6}",
        started.elapsed().as_secs_f64()
    );
    Ok(Identity {
        fingerprint,
        source,
        executable,
        harness,
        inputs,
        environment,
        dirty,
    })
}

fn part(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(windows)]
fn desktop() -> Result<String, String> {
    crate::qualification_environment::diagnostic::capture()
}
#[cfg(not(windows))]
fn desktop() -> Result<String, String> {
    Err("diagnostic reuse requires Windows".into())
}
#[cfg(windows)]
fn power_scheme() -> Result<String, String> {
    command("powercfg.exe", &["/getactivescheme"])
}
#[cfg(not(windows))]
fn power_scheme() -> Result<String, String> {
    Err("diagnostic reuse requires Windows".into())
}

#[cfg(windows)]
fn command(program: &str, args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    let result = std::process::Command::new(program)
        .args(args)
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|_| format!("cannot inspect diagnostic environment using {program}"))?;
    if !result.status.success() || result.stdout.is_empty() {
        return Err(format!("incomplete diagnostic environment from {program}"));
    }
    // powercfg may use an OEM code page. Hash bytes losslessly; do not print identifiers.
    Ok(result
        .stdout
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}
