//! Atomic persistence for verification evidence files.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| format!("evidence path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create evidence directory {}: {error}",
            parent.display()
        )
    })?;

    let temporary = temporary_path(path)?;
    write_and_publish(&temporary, contents, || replace(&temporary, path))
}

fn temporary_path(target: &Path) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("evidence file name is not UTF-8: {}", target.display()))?;
    Ok(target.with_file_name(format!(".{name}.tmp-{}-{nonce}", std::process::id())))
}

/// Atomically create a new complete file; preserve the old CreateNew no-overwrite interface.
pub(crate) fn write_new(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("new file path has no parent")?;
    if !parent.is_dir() {
        return Err(format!(
            "destination directory does not exist: {}",
            parent.display()
        ));
    }
    let temporary = temporary_path(path)?;
    write_and_publish(&temporary, contents, || publish_new(&temporary, path))
}

fn write_and_publish(
    temporary: &Path,
    contents: &[u8],
    publish: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    // Cleanup is authorized only after this invocation actually created the temp file.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    let written = file.write_all(contents).and_then(|()| file.sync_all());
    drop(file);
    let result = written
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))
        .and_then(|()| publish());
    let cleanup = fs::remove_file(temporary);
    result?;
    match cleanup {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "cannot remove temporary output {}: {error}",
            temporary.display()
        )),
    }
}

#[cfg(windows)]
fn replace(temporary: &Path, target: &Path) -> Result<(), String> {
    move_windows(temporary, target, true)
}

#[cfg(windows)]
fn publish_new(temporary: &Path, target: &Path) -> Result<(), String> {
    move_windows(temporary, target, false)
}

#[cfg(windows)]
fn move_windows(temporary: &Path, target: &Path, overwrite: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }

    let from = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: `from` and `to` are live, NUL-terminated UTF-16 buffers for the duration of the
    // call. Both paths are same-directory evidence files owned by this process. The API does
    // not retain either pointer. Replacement is enabled only for the overwrite interface;
    // new notices retain CreateNew semantics using a same-directory no-replace move.
    let moved = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_WRITE_THROUGH
                | if overwrite {
                    MOVEFILE_REPLACE_EXISTING
                } else {
                    0
                },
        )
    };
    if moved == 0 {
        Err(format!(
            "cannot atomically publish {} as {}: {}",
            temporary.display(),
            target.display(),
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn publish_new(temporary: &Path, target: &Path) -> Result<(), String> {
    fs::hard_link(temporary, target)
        .map_err(|error| format!("cannot publish new file {}: {error}", target.display()))
}

#[cfg(not(windows))]
fn replace(temporary: &Path, target: &Path) -> Result<(), String> {
    fs::rename(temporary, target).map_err(|error| {
        format!(
            "cannot atomically publish {} as {}: {error}",
            temporary.display(),
            target.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::write;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn create_new_publishes_once_and_preserves_existing_bytes_under_races() {
        let root = std::env::temp_dir().join(format!(
            "stickymd-atomic-new-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("中文 output.txt");
        let handles = [b"first".as_slice(), b"second".as_slice()]
            .into_iter()
            .map(|bytes| {
                let path = target.clone();
                std::thread::spawn(move || super::write_new(&path, bytes))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(Result::is_ok)
                .count(),
            1
        );
        let before = std::fs::read(&target).unwrap();
        assert!(before == b"first" || before == b"second");
        assert!(super::write_new(&target, b"overwrite").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), before);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn occupied_temporary_file_is_never_removed_without_ownership() {
        let root = std::env::temp_dir().join(format!(
            "stickymd-atomic-collision-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let occupied = root.join("occupied.tmp");
        std::fs::write(&occupied, b"belongs to another operation").unwrap();
        assert!(
            super::write_and_publish(&occupied, b"replacement", || panic!("must not publish"))
                .is_err()
        );
        assert_eq!(
            std::fs::read(&occupied).unwrap(),
            b"belongs to another operation"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_write_atomically_replaces_previous_evidence() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-atomic-evidence-{nonce}"));
        let target = root.join("module.json");
        write(&target, b"old").expect("initial write");
        write(&target, b"new").expect("replacement write");
        assert_eq!(fs::read(&target).expect("read evidence"), b"new");
        assert_eq!(fs::read_dir(&root).expect("read root").count(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
