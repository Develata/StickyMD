//! Shared integration fixtures; assertions stay with each real CLI behavior.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
pub mod powershell;

pub fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("smoke crate lives under tools/")
}

pub struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    pub fn new(label: &str, suffix: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after Unix epoch")
            .as_nanos();
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "stickymd-{label}-{}-{nonce}-{sequence}-{suffix}",
            std::process::id()
        ));
        // Exclusive creation establishes ownership: a collision never authorizes cleanup.
        fs::create_dir(&path).expect("create an exclusively owned fixture directory");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        // Only this freshly created root is owned; never remove repository or shared outputs.
        if let Err(error) = fs::remove_dir_all(&self.path) {
            if std::thread::panicking() {
                eprintln!(
                    "fixture cleanup failed for {}: {error}",
                    self.path.display()
                );
            } else {
                panic!(
                    "fixture cleanup failed for {}: {error}",
                    self.path.display()
                );
            }
        }
    }
}

#[test]
fn fixture_roots_are_independent_and_unwind_removes_only_owned_files() {
    let sibling = TemporaryDirectory::new("fixture-owner", "中文 space");
    fs::write(sibling.path().join("retained.txt"), b"retained").unwrap();
    let mut owned_path = PathBuf::new();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let directory = TemporaryDirectory::new("fixture-owner", "中文 space");
        owned_path = directory.path().to_owned();
        assert_ne!(owned_path, sibling.path());
        fs::write(directory.path().join("owned.txt"), b"owned").unwrap();
        panic!("simulate a failed integration assertion");
    }));
    assert!(unwind.is_err());
    assert!(!owned_path.exists());
    assert_eq!(
        fs::read(sibling.path().join("retained.txt")).unwrap(),
        b"retained"
    );
}
