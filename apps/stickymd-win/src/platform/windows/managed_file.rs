//! Handle-bound rename/delete operations for proven managed assets.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

use std::fs::{File, OpenOptions};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FILE_DISPOSITION_INFO, FILE_RENAME_INFO, FileDispositionInfo, FileRenameInfo,
    SetFileInformationByHandle,
};

const GENERIC_READ_AND_DELETE: u32 = 0x8000_0000 | 0x0001_0000;
const GENERIC_READ_WRITE_AND_DELETE: u32 = 0x8000_0000 | 0x4000_0000 | 0x0001_0000;
const SHARE_READ: u32 = 0x0000_0001;
const SHARE_READ_WRITE: u32 = 0x0000_0001 | 0x0000_0002;
const FILE_FLAG_OPEN_REPARSE_POINT_VALUE: u32 = 0x0020_0000;
const FILE_FLAG_BACKUP_SEMANTICS_VALUE: u32 = 0x0200_0000;

/// Open the pathname itself (not a reparse target) with delete authority. The
/// returned handle is the identity subsequently hashed and mutated.
pub fn open_for_managed_mutation(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(GENERIC_READ_AND_DELETE)
        // Ownership proof is useful only if the bytes cannot change before
        // the handle-bound rename/delete. Read sharing preserves harmless
        // observers; write and delete sharing stay denied until mutation ends.
        .share_mode(SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE)
        .open(path)
}

/// Exclusively create a transaction-owned managed temporary and retain the
/// exact writable/delete-capable handle through flush, publish or cleanup.
pub fn create_new_for_managed_publish(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .access_mode(GENERIC_READ_WRITE_AND_DELETE)
        .share_mode(SHARE_READ)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE)
        .open(path)
}

/// Open the exact directory without delete sharing. Keeping this handle alive
/// prevents the proven managed root from being renamed or replaced while a
/// handle-bound source asset rename is in progress.
pub fn open_managed_directory(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(GENERIC_READ_AND_DELETE)
        .share_mode(SHARE_READ_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE | FILE_FLAG_BACKUP_SEMANTICS_VALUE)
        .open(path)
}

/// Open the canonical note for a destructive asset boundary. Sharing only
/// reads prevents ordinary writes and atomic replacement while the caller
/// verifies durable references and deletes proven trash.
pub fn open_guarded_note_read(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(0x8000_0000)
        .share_mode(0x0000_0001)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE)
        .open(path)
}

/// Rename the exact file referenced by `file`; a concurrently-created target
/// is never replaced. `target_directory` is intentionally held open without
/// delete sharing for the whole call, keeping the already-proven absolute
/// destination root stable while Windows resolves the absolute target path.
pub fn rename_open_file(
    file: &File,
    _target_directory: &File,
    target: &Path,
) -> std::io::Result<()> {
    let target = target.as_os_str().encode_wide().collect::<Vec<_>>();
    let name_bytes = target
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| std::io::Error::other("managed asset target path is too long"))?;
    let file_name_offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let total_bytes = file_name_offset
        .checked_add(name_bytes as usize)
        .and_then(|length| length.checked_add(std::mem::size_of::<u16>()))
        .ok_or_else(|| std::io::Error::other("managed rename buffer length overflow"))?;
    let word = std::mem::size_of::<usize>();
    let words = total_bytes.div_ceil(word);
    let mut storage = vec![0usize; words];
    let information = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: `storage` is usize-aligned, zero-initialized, and large enough for
    // the fixed header, every UTF-16 code unit and a trailing NUL. `information`
    // is used only while storage is live. The union's `ReplaceIfExists=false`
    // field is the documented legacy
    // FILE_RENAME_INFO disposition, and the synchronous API retains no pointer.
    unsafe {
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).RootDirectory = HANDLE::default();
        (*information).FileNameLength = name_bytes;
        std::ptr::copy_nonoverlapping(
            target.as_ptr(),
            (*information).FileName.as_mut_ptr(),
            target.len(),
        );
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileRenameInfo,
            information.cast(),
            u32::try_from(total_bytes)
                .map_err(|_| std::io::Error::other("managed rename buffer too large"))?,
        )
        .map_err(std::io::Error::other)
    }
}

/// Mark the exact proven file handle for deletion on close.
pub fn delete_open_file(file: File) -> std::io::Result<()> {
    let information = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: the handle is borrowed from a live File opened with DELETE
    // access; `information` is valid for the synchronous call and is not
    // retained. Dropping File afterwards closes the same handle-bound object.
    unsafe {
        SetFileInformationByHandle(
            HANDLE(file.as_raw_handle()),
            FileDispositionInfo,
            std::ptr::from_ref(&information).cast(),
            u32::try_from(std::mem::size_of::<FILE_DISPOSITION_INFO>())
                .map_err(|_| std::io::Error::other("disposition size overflow"))?,
        )
        .map_err(std::io::Error::other)?;
    }
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::io::Write;

    fn fixture(name: &str) -> std::path::PathBuf {
        let root = unique_temp_path(&format!("managed-handle-{name}"));
        std::fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn proof_handle_denies_concurrent_write_and_rename_until_released() {
        let root = fixture("proof-lock");
        let path = root.join("asset.png");
        std::fs::write(&path, b"proved bytes").unwrap();
        let proof = open_for_managed_mutation(&path).unwrap();

        assert!(OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::rename(&path, root.join("replaced.png")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"proved bytes");

        drop(proof);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_publish_cleanup_is_bound_to_the_created_handle() {
        let root = fixture("owned-cleanup");
        let path = root.join("owned.tmp");
        let mut owned = create_new_for_managed_publish(&path).unwrap();
        owned.write_all(b"transaction bytes").unwrap();
        owned.sync_all().unwrap();

        assert!(std::fs::rename(&path, root.join("replacement.tmp")).is_err());
        delete_open_file(owned).unwrap();
        assert!(!path.exists());

        std::fs::remove_dir_all(root).unwrap();
    }
}
