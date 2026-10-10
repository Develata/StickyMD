//! Portable external SHA-256 adapter; Windows uses the in-process CNG backend.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Longest one external digest may take; inputs are evidence files and screenshots.
const DIGEST_LIMIT: Duration = Duration::from_secs(60);

/// Digest bytes the caller already holds, so it validates and hashes one snapshot. The
/// bytes go to a private, exclusively created file that `sha256sum` reads, so no pipe
/// or helper thread can block, and the external process runs under [`DIGEST_LIMIT`].
pub(super) fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let snapshot = Snapshot::create(bytes)?;
    sha256(&snapshot.0)
}

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    let output = run_bounded(Command::new("sha256sum").arg("--").arg(path), DIGEST_LIMIT)
        .map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    let text = String::from_utf8_lossy(&output);
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

/// Run `command` to completion within `limit` and return its stdout. The output
/// is one short line, far below a pipe buffer, so the child never blocks on stdout while
/// it is polled; past the limit, or on a wait error, it is killed and reaped.
fn run_bounded(command: &mut Command, limit: Duration) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start sha256sum: {error}"))?;
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                return Err(stop(
                    &mut child,
                    format!("sha256sum exceeded {} ms", limit.as_millis()),
                ));
            }
            Err(error) => {
                return Err(stop(
                    &mut child,
                    format!("cannot wait for sha256sum: {error}"),
                ));
            }
        }
    };
    if !status.success() {
        return Err(format!("sha256sum exited with {status}"));
    }
    let mut output = Vec::new();
    child
        .stdout
        .take()
        .ok_or("sha256sum stdout is unavailable")?
        .read_to_end(&mut output)
        .map_err(|error| format!("cannot read sha256sum output: {error}"))?;
    Ok(output)
}

/// Kill and reap `child`, reporting each step that fails alongside `problem`. Our own
/// child can only fail to be killed once it has exited, in which case the wait returns.
fn stop(child: &mut Child, problem: String) -> String {
    let mut message = problem;
    if let Err(error) = child.kill() {
        message.push_str(&format!("; stopping sha256sum failed: {error}"));
    }
    if let Err(error) = child.wait() {
        message.push_str(&format!("; reaping sha256sum failed: {error}"));
    }
    message
}

/// A private copy of in-memory bytes, removed when dropped.
struct Snapshot(PathBuf);

impl Snapshot {
    fn create(bytes: &[u8]) -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "stickymd-sha256-{}-{}.bin",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        // Exclusive creation: an existing file is never adopted or overwritten.
        let mut file = File::create_new(&path)
            .map_err(|error| format!("cannot create hash snapshot {}: {error}", path.display()))?;
        let snapshot = Self(path);
        file.write_all(bytes)
            .and_then(|()| file.flush())
            .map_err(|error| format!("cannot write hash snapshot: {error}"))?;
        Ok(snapshot)
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        // The digest does not depend on cleanup; a leftover private file is reported.
        if let Err(error) = fs::remove_file(&self.0) {
            eprintln!(
                "SHA256_SNAPSHOT_CLEANUP_FAILED={}: {error}",
                self.0.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::run_bounded;

    #[test]
    fn a_hung_or_failing_command_is_stopped_and_reported_within_the_limit() {
        let started = Instant::now();
        let error =
            run_bounded(Command::new("sleep").arg("30"), Duration::from_millis(200)).unwrap_err();
        assert!(error.contains("exceeded"), "{error}");
        assert!(
            !error.contains("failed"),
            "the child is killed and reaped: {error}"
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        let error = run_bounded(&mut Command::new("false"), Duration::from_secs(10)).unwrap_err();
        assert!(error.contains("exited"), "{error}");
        assert_eq!(
            run_bounded(Command::new("printf").arg("ok"), Duration::from_secs(10)).unwrap(),
            b"ok"
        );
    }
}
