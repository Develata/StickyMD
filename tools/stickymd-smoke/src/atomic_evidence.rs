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
    let result = write_and_publish(&temporary, path, contents);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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

fn write_and_publish(temporary: &Path, target: &Path, contents: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    file.write_all(contents)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    file.sync_all()
        .map_err(|error| format!("cannot flush {}: {error}", temporary.display()))?;
    drop(file);
    replace(temporary, target)
}

#[cfg(windows)]
fn replace(temporary: &Path, target: &Path) -> Result<(), String> {
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
    // not retain either pointer, and the flags request an atomic replace plus write-through.
    let moved = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
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
