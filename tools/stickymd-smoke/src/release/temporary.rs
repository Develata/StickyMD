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
        // Same-process callers can read the same coarse clock value; the sequence keeps
        // concurrent scratch directories distinct instead of failing create_dir.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "stickymd-{label}-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        Self::create(path)
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
