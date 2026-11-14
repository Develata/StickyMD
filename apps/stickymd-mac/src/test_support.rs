//! Test-only filesystem fixture helpers.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_TEMP_PATH: AtomicU64 = AtomicU64::new(0);

/// A process-local unique path, not yet created. The timestamp avoids reusing a stale path after
/// PID reuse.
pub(crate) fn unique_temp_path(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_TEMP_PATH.fetch_add(1, Ordering::Relaxed);
    // The system temporary directory lies under a symbolic link, and a program directory never
    // does: it is canonicalized before use.
    let base = std::fs::canonicalize(std::env::temp_dir()).expect("the temporary directory");
    base.join(format!(
        "stickymd-test-{label}-{}-{timestamp}-{sequence}",
        std::process::id()
    ))
}
