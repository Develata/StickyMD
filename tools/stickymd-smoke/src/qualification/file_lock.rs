//! OS file locks that guard qualification stores: bounded waiting, and sidecar lock files
//! that are never replaced or truncated, so every process locks the same file. The OS
//! releases a lock when its process exits, so a crash cannot strand it.
//!
//! Lock order, to rule out deadlock: the manual observation store lock is taken before
//! the release decision lock, never after. Promote and Source Freeze take the manual store
//! lock alone while they replace or invalidate the candidate.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

/// Longest a qualification command waits for another process's store operation.
pub(super) const LOCK_WAIT: Duration = Duration::from_secs(120);

/// Holds a sidecar lock until dropped; closing the handle releases it.
#[must_use = "the lock is released when the guard is dropped"]
pub(super) struct SidecarGuard {
    _file: File,
}

/// Locks the sidecar file at `path`, creating it (and its directory) if needed:
/// exclusive for changes, shared for consistent reads.
pub(super) fn sidecar(path: &Path, exclusive: bool, store: &str) -> Result<SidecarGuard, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    // A linked lock file would let two paths name one lock or one path name two.
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(format!(
            "{} is a link and is not trusted as a lock",
            path.display()
        ));
    }
    // Windows refuses LockFileEx on an append-only handle, so open read-write; never
    // truncate, because other processes may hold the same file.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    if exclusive {
        wait_for_lock(|| file.try_lock(), store, "writing")?;
    } else {
        wait_for_lock(|| file.try_lock_shared(), store, "reading")?;
    }
    Ok(SidecarGuard { _file: file })
}

/// Retries `attempt` until it gets the lock or `LOCK_WAIT` passes. Store operations take
/// seconds, so a holder past the limit is hung or suspended and the caller gets a
/// diagnosable error naming `store` and `purpose` instead of blocking forever.
pub(super) fn wait_for_lock(
    attempt: impl FnMut() -> Result<(), TryLockError>,
    store: &str,
    purpose: &str,
) -> Result<(), String> {
    wait_for_lock_within(attempt, store, purpose, LOCK_WAIT)
}

fn wait_for_lock_within(
    mut attempt: impl FnMut() -> Result<(), TryLockError>,
    store: &str,
    purpose: &str,
    limit: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + limit;
    loop {
        match attempt() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "{store} stayed locked by another qualification process for {} s while {purpose}",
                    limit.as_secs()
                ));
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("cannot lock {store} for {purpose}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{sidecar, wait_for_lock_within};

    #[test]
    fn a_held_lock_times_out_and_a_released_one_is_taken() {
        let directory =
            std::env::temp_dir().join(format!("stickymd-file-lock-{}", std::process::id()));
        let path = directory.join("store.lock");
        let held = sidecar(&path, true, "test store").unwrap();
        let other = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        let error = wait_for_lock_within(
            || other.try_lock_shared(),
            "test store",
            "reading",
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(error.contains("stayed locked"), "{error}");
        drop(held);
        // Unix: a child forked by another test may hold the descriptor until it execs.
        wait_for_lock_within(
            || other.try_lock_shared(),
            "test store",
            "reading",
            Duration::from_secs(5),
        )
        .unwrap();
        drop(other);
        // Created exclusively by this test under the temp directory.
        let _ = std::fs::remove_dir_all(directory);
    }
}
