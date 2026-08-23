//! Test-only filesystem fixture helpers.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_PATH: AtomicU64 = AtomicU64::new(0);

/// Returns a process-local unique path without creating it.
///
/// The atomic sequence prevents parallel tests from colliding on Windows,
/// where the wall-clock timestamp can have coarser resolution than fixture
/// creation. The timestamp also avoids reusing a stale path after PID reuse.
pub(crate) fn unique_temp_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_TEMP_PATH.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stickymd-test-{label}-{}-{timestamp}-{sequence}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::unique_temp_path;
    use std::collections::HashSet;

    #[test]
    fn parallel_fixture_paths_are_unique() {
        let handles = (0..32)
            .map(|_| std::thread::spawn(|| unique_temp_path("parallel")))
            .collect::<Vec<_>>();
        let paths = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<HashSet<_>>();
        assert_eq!(paths.len(), 32);
    }
}
