//! Exclusively-created release-tool scratch space, removed on every return path.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{
    fs,
    path::{Path, PathBuf},
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
        let path =
            std::env::temp_dir().join(format!("stickymd-{label}-{}-{nonce}", std::process::id()));
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
