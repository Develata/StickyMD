//! Read-only fault injection for a recorder-owned fixture, with checked restoration.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::fs::{self, Permissions};
use std::path::{Path, PathBuf};

/// Call only on a file in a fixture this harness created. Never on the user's note.
pub(super) fn with_readonly<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = ReadOnly::new(path)?;
    let outcome = operation();
    match (outcome, guard.restore()) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; {cleanup}")),
    }
}

struct ReadOnly {
    path: PathBuf,
    original: Permissions,
    restored: bool,
}

impl ReadOnly {
    fn new(path: &Path) -> Result<Self, String> {
        let metadata =
            fs::symlink_metadata(path).map_err(|error| format!("inspect fixture: {error}"))?;
        if !metadata.is_file() || metadata.permissions().readonly() {
            return Err("read-only injection requires a writable fixture file".to_owned());
        }
        // Construct before changing the attribute, so a failed read-back still restores it.
        let mut guard = Self {
            path: path.to_owned(),
            original: metadata.permissions(),
            restored: false,
        };
        let mut readonly = guard.original.clone();
        readonly.set_readonly(true);
        let applied = (|| {
            fs::set_permissions(path, readonly)
                .map_err(|error| format!("make fixture read-only: {error}"))?;
            if !fs::metadata(path)
                .map_err(|error| format!("read back read-only fixture: {error}"))?
                .permissions()
                .readonly()
            {
                return Err("fixture did not become read-only".to_owned());
            }
            Ok(())
        })();
        if let Err(error) = applied {
            return Err(match guard.restore() {
                Ok(()) => error,
                Err(cleanup) => format!("{error}; {cleanup}"),
            });
        }
        Ok(guard)
    }

    fn restore(&mut self) -> Result<(), String> {
        fs::set_permissions(&self.path, self.original.clone()).map_err(|error| {
            format!(
                "restore fixture attributes {}: {error}",
                self.path.display()
            )
        })?;
        let observed = fs::metadata(&self.path)
            .map_err(|error| format!("read back restored fixture: {error}"))?;
        if observed.permissions().readonly() != self.original.readonly() {
            return Err(format!(
                "fixture attributes not restored: {}",
                self.path.display()
            ));
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        if !self.restored
            && let Err(error) = self.restore()
        {
            eprintln!("read-only fixture cleanup failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_are_restored_after_success_error_and_unwind() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stickymd-readonly-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("note.md");
        fs::write(&path, b"original").unwrap();
        with_readonly(&path, || {
            assert!(fs::metadata(&path).unwrap().permissions().readonly());
            assert!(fs::write(&path, b"must fail").is_err());
            Ok(())
        })
        .unwrap();
        assert!(!fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(
            with_readonly(&path, || Err::<(), _>("operation failed".to_owned())).unwrap_err(),
            "operation failed"
        );
        assert!(!fs::metadata(&path).unwrap().permissions().readonly());
        let panicked = std::panic::catch_unwind(|| {
            let _: Result<(), String> = with_readonly(&path, || panic!("injected unwind"));
        });
        assert!(panicked.is_err());
        assert!(!fs::metadata(&path).unwrap().permissions().readonly());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        // A fixture moved during the operation cannot have its original path restored.
        // Even a successful operation must then return an error, not a PASS value.
        let original = fs::metadata(&path).unwrap().permissions();
        let moved = root.join("moved.md");
        let restore_failure = with_readonly(&path, || {
            fs::rename(&path, &moved).unwrap();
            Ok(())
        })
        .unwrap_err();
        fs::set_permissions(&moved, original).unwrap();
        fs::rename(&moved, &path).unwrap();
        assert!(
            restore_failure.contains("restore fixture attributes"),
            "{restore_failure}"
        );
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
