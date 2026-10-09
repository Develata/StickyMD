//! Portable external SHA-256 adapter; Windows uses the in-process CNG backend.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::io::{Read, Write};
use std::process::{Child, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use std::{path::Path, process::Command};

/// Longest an in-memory digest may take; inputs are evidence documents and screenshots.
const DIGEST_LIMIT: Duration = Duration::from_secs(60);

/// Digest bytes the caller already holds, so it validates and hashes one snapshot.
/// The child is reaped on every path: a feeder thread writes stdin while this thread
/// waits with a deadline, and any failure kills the child before reporting.
pub(super) fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let mut child = Command::new("sha256sum")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot start sha256sum: {error}"))?;
    let stdin = child.stdin.take();
    let (written, waited) = std::thread::scope(|scope| {
        let feeder = scope.spawn(move || {
            // Dropping stdin when the write finishes closes the pipe for sha256sum.
            let mut stdin = stdin.ok_or("sha256sum stdin is unavailable")?;
            stdin
                .write_all(bytes)
                .map_err(|error| format!("cannot stream bytes to sha256sum: {error}"))
        });
        // On timeout the child is killed, which also ends a blocked write.
        let waited = wait_bounded(&mut child);
        let written = feeder
            .join()
            .unwrap_or_else(|_| Err("sha256sum feeder thread panicked".to_owned()));
        (written, waited)
    });
    let status = waited?;
    written?;
    if !status.success() {
        return Err("SHA-256 command failed for in-memory bytes".to_owned());
    }
    let mut text = String::new();
    child
        .stdout
        .take()
        .ok_or("sha256sum stdout is unavailable")?
        .read_to_string(&mut text)
        .map_err(|error| format!("cannot read sha256sum output: {error}"))?;
    let hash = text
        .split_once(' ')
        .map(|(hash, _)| hash)
        .ok_or("SHA-256 output has no digest field")?;
    super::validate_sha256(hash, "SHA-256")?;
    Ok(hash.to_ascii_lowercase())
}

/// Wait for `child` until [`DIGEST_LIMIT`]; past it, or on a wait error, kill and reap
/// it and report what happened, including any failure to stop it.
fn wait_bounded(child: &mut Child) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + DIGEST_LIMIT;
    let problem = loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                break format!(
                    "sha256sum exceeded {} s for in-memory bytes",
                    DIGEST_LIMIT.as_secs()
                );
            }
            Err(error) => break format!("cannot wait for sha256sum: {error}"),
        }
    };
    let stopped = child
        .kill()
        .and_then(|()| child.wait().map(|_| ()))
        .map_err(|error| format!("; stopping sha256sum also failed: {error}"));
    Err(format!("{problem}{}", stopped.err().unwrap_or_default()))
}

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
