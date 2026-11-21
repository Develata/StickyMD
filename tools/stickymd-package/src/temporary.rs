//! Exclusive release-tool staging, cleaned up unless ownership is transferred.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct TemporaryDirectory {
    path: PathBuf,
    closed: bool,
}
impl TemporaryDirectory {
    pub fn new(label: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        Self::create(scratch_path(label, nonce))
    }
    pub fn create(path: PathBuf) -> Result<Self, String> {
        fs::create_dir(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        Ok(Self {
            path,
            closed: false,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn keep(mut self) {
        self.closed = true;
    }
    pub fn close(mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.path).map_err(|error| {
            format!(
                "cannot remove release scratch directory {}: {error}",
                self.path.display()
            )
        })?;
        self.closed = true;
        Ok(())
    }
}
impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if !self.closed
            && let Err(error) = fs::remove_dir_all(&self.path)
        {
            eprintln!(
                "cannot remove release scratch directory {}: {error}",
                self.path.display()
            );
        }
    }
}

/// Same-process callers can read the same coarse clock value; the sequence keeps
/// concurrent scratch directories distinct instead of failing `create_dir`.
fn scratch_path(label: &str, nonce: u128) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stickymd-{label}-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_identical_clock_value_still_yields_distinct_scratch_directories() {
        assert_ne!(
            super::scratch_path("probe", 7),
            super::scratch_path("probe", 7)
        );
    }
}
