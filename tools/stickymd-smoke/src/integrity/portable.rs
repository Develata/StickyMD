//! Portable external SHA-256 adapter; Windows uses the in-process CNG backend.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Longest one external digest may take; inputs are evidence files and screenshots.
const DIGEST_LIMIT: Duration = Duration::from_secs(60);
/// Longest a killed digest process may take to be reaped before giving up on it.
const REAP_LIMIT: Duration = Duration::from_secs(5);

/// Digest bytes the caller already holds, so it validates and hashes one snapshot. The
/// bytes go to an exclusively created, owner-only file that `sha256sum` reads, so no
/// pipe or helper thread can block, and the external process runs under a deadline.
pub(super) fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    let snapshot = Snapshot::create(bytes)?;
    sha256(&snapshot.0)
}

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    let output = run_bounded(
        Command::new("sha256sum").arg("--").arg(path),
        DIGEST_LIMIT,
        REAP_LIMIT,
    )
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

/// Run `command` to completion within `limit` and return its stdout. The output is one
/// short line, far below a pipe buffer, so the child never blocks on stdout while it is
/// polled; stderr is inherited so its diagnostics reach the caller's log without a pipe.
/// Past the limit, or on a wait error, the child is killed and reaped within `reap`.
fn run_bounded(command: &mut Command, limit: Duration, reap: Duration) -> Result<Vec<u8>, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("cannot start sha256sum: {error}"))?;
    let status = match wait_until(&mut child, Instant::now() + limit) {
        Ok(Some(status)) => status,
        Ok(None) => {
            return Err(stop(
                &mut child,
                format!("sha256sum exceeded {} ms", limit.as_millis()),
                reap,
            ));
        }
        Err(error) => {
            return Err(stop(
                &mut child,
                format!("cannot wait for sha256sum: {error}"),
                reap,
            ));
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

/// Poll `child` until it exits or `deadline` passes (`None`); never blocks past it.
fn wait_until(child: &mut Child, deadline: Instant) -> std::io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Kill `child` and reap it within `reap`, reporting each step that fails alongside
/// `problem`. A child that does not exit in time (for example one stuck in an
/// uninterruptible kernel wait) is reported as not reaped instead of blocking forever.
fn stop(child: &mut Child, problem: String, reap: Duration) -> String {
    let mut message = problem;
    if let Err(error) = child.kill() {
        message.push_str(&format!("; stopping sha256sum failed: {error}"));
    }
    match wait_until(child, Instant::now() + reap) {
        Ok(Some(_)) => {}
        Ok(None) => message.push_str(&format!(
            "; sha256sum was not reaped within {} ms",
            reap.as_millis()
        )),
        Err(error) => message.push_str(&format!("; reaping sha256sum failed: {error}")),
    }
    message
}

/// A private copy of in-memory bytes, removed when dropped.
struct Snapshot(PathBuf);

impl Snapshot {
    /// Names combine the process, a per-process start nonce and a sequence, so a name a
    /// crashed run left behind is skipped rather than reused or overwritten.
    fn create(bytes: &[u8]) -> Result<Self, String> {
        const ATTEMPTS: usize = 16;
        static NONCE: OnceLock<u128> = OnceLock::new();
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = *NONCE.get_or_init(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        });
        Self::create_first_free(
            (0..ATTEMPTS).map(|_| {
                std::env::temp_dir().join(format!(
                    "stickymd-sha256-{}-{nonce}-{}.bin",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ))
            }),
            bytes,
        )
    }

    /// Write `bytes` to the first of `names` that does not exist yet.
    fn create_first_free(
        names: impl IntoIterator<Item = PathBuf>,
        bytes: &[u8],
    ) -> Result<Self, String> {
        let mut tried = 0;
        for path in names {
            tried += 1;
            // Exclusive creation (an existing file is never adopted or overwritten),
            // owner-only so no other account can alter what is hashed.
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = match options.open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!(
                        "cannot create hash snapshot {}: {error}",
                        path.display()
                    ));
                }
            };
            let snapshot = Self(path);
            file.write_all(bytes)
                .and_then(|()| file.flush())
                .map_err(|error| format!("cannot write hash snapshot: {error}"))?;
            return Ok(snapshot);
        }
        Err(format!(
            "cannot create a hash snapshot: all {tried} candidate names already exist"
        ))
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

    use super::{Snapshot, run_bounded};

    #[test]
    fn a_hung_or_failing_command_is_stopped_and_reported_within_the_limit() {
        let started = Instant::now();
        let error = run_bounded(
            Command::new("sleep").arg("30"),
            Duration::from_millis(200),
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("exceeded"), "{error}");
        assert!(
            !error.contains("failed") && !error.contains("not reaped"),
            "the child is killed and reaped: {error}"
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        let error = run_bounded(
            &mut Command::new("false"),
            Duration::from_secs(10),
            Duration::from_secs(5),
        )
        .unwrap_err();
        assert!(error.contains("exited"), "{error}");
        assert_eq!(
            run_bounded(
                Command::new("printf").arg("ok"),
                Duration::from_secs(10),
                Duration::from_secs(5)
            )
            .unwrap(),
            b"ok"
        );
    }

    /// A name left by a crashed run is skipped, and the leftover is never touched.
    #[test]
    fn an_occupied_snapshot_name_is_skipped_without_being_overwritten() {
        let directory = std::env::temp_dir();
        let name = |label: &str| {
            directory.join(format!(
                "stickymd-sha256-test-{}-{label}.bin",
                std::process::id()
            ))
        };
        let (occupied, free) = (name("occupied"), name("free"));
        std::fs::write(&occupied, b"leftover").unwrap();
        let snapshot =
            Snapshot::create_first_free([occupied.clone(), free.clone()], b"fresh").unwrap();
        assert_eq!(snapshot.0, free);
        assert_eq!(std::fs::read(&free).unwrap(), b"fresh");
        assert_eq!(std::fs::read(&occupied).unwrap(), b"leftover");
        drop(snapshot);
        assert!(!free.exists(), "the snapshot is removed when dropped");
        let error = Snapshot::create_first_free([occupied.clone()], b"x")
            .err()
            .unwrap();
        assert!(error.contains("already exist"), "{error}");
        std::fs::remove_file(occupied).unwrap();
    }
}
