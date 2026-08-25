//! Flush-before-publish atomic file replacement primitives.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

use thiserror::Error;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    FlushFileBuffers, MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS, ReplaceFileW,
};
use windows::core::PCWSTR;

use super::file_identity::observe_open_file;

const ERROR_UNABLE_TO_REMOVE_REPLACED: u32 = 1175;
const ERROR_UNABLE_TO_MOVE_REPLACEMENT: u32 = 1176;
const ERROR_UNABLE_TO_MOVE_REPLACEMENT_2: u32 = 1177;
const FILE_ATTRIBUTE_REPARSE_POINT_VALUE: u32 = 0x400;
const FILE_FLAG_OPEN_REPARSE_POINT_VALUE: u32 = 0x0020_0000;
const SHARE_READ: u32 = 0x0000_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceFailureKind {
    Unclassified,
    UnableToRemoveReplaced,
    UnableToMoveReplacement,
    PartialMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicStage {
    BeforeTempCreate,
    BeforeTempWrite,
    AfterTempWrite,
    AfterTempFlush,
    BeforeReplace,
}

pub const fn classify_replace_error(code: u32) -> ReplaceFailureKind {
    match code {
        ERROR_UNABLE_TO_REMOVE_REPLACED => ReplaceFailureKind::UnableToRemoveReplaced,
        ERROR_UNABLE_TO_MOVE_REPLACEMENT => ReplaceFailureKind::UnableToMoveReplacement,
        ERROR_UNABLE_TO_MOVE_REPLACEMENT_2 => ReplaceFailureKind::PartialMutation,
        _ => ReplaceFailureKind::Unclassified,
    }
}

/// Publish complete bytes through a fixed same-directory temporary file.
///
/// Existing targets use `ReplaceFileW(..., flags=0)`. A failed replacement is
/// never followed by a blind fallback; the temporary file remains as evidence.
pub fn atomic_publish(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> Result<(), AtomicPublishError> {
    atomic_publish_with_observer(target, temporary, bytes, |_| Ok(()))
}

pub(crate) fn atomic_publish_with_observer<F>(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
    mut observe: F,
) -> Result<(), AtomicPublishError>
where
    F: FnMut(AtomicStage) -> std::io::Result<()>,
{
    if target.parent() != temporary.parent() {
        return Err(AtomicPublishError::DifferentDirectories);
    }

    observe(AtomicStage::BeforeTempCreate).map_err(|source| AtomicPublishError::Injected {
        stage: AtomicStage::BeforeTempCreate,
        source,
    })?;
    let mut file = open_fixed_temporary(temporary)?;
    observe(AtomicStage::BeforeTempWrite).map_err(|source| AtomicPublishError::Injected {
        stage: AtomicStage::BeforeTempWrite,
        source,
    })?;
    file.write_all(bytes)
        .map_err(AtomicPublishError::TempWrite)?;
    observe(AtomicStage::AfterTempWrite).map_err(|source| AtomicPublishError::Injected {
        stage: AtomicStage::AfterTempWrite,
        source,
    })?;
    file.flush().map_err(AtomicPublishError::TempFlush)?;
    flush_windows_handle(&file).map_err(AtomicPublishError::TempFlush)?;
    observe(AtomicStage::AfterTempFlush).map_err(|source| AtomicPublishError::Injected {
        stage: AtomicStage::AfterTempFlush,
        source,
    })?;
    drop(file);
    observe(AtomicStage::BeforeReplace).map_err(|source| AtomicPublishError::Injected {
        stage: AtomicStage::BeforeReplace,
        source,
    })?;

    publish_prepared(target, temporary)
}

/// Write and flush the same-directory temporary file without publishing it.
/// This split lets guarded note persistence perform its final fingerprint check
/// after expensive encoding/write/flush work and immediately before replacement.
pub(crate) fn prepare_temporary(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> Result<(), AtomicPublishError> {
    if target.parent() != temporary.parent() {
        return Err(AtomicPublishError::DifferentDirectories);
    }
    let mut file = open_fixed_temporary(temporary)?;
    file.write_all(bytes)
        .map_err(AtomicPublishError::TempWrite)?;
    file.flush().map_err(AtomicPublishError::TempFlush)?;
    flush_windows_handle(&file).map_err(AtomicPublishError::TempFlush)
}

/// Create, write and flush a caller-selected temporary path only if it does
/// not already exist. This gives export cleanup a proof that the path belongs
/// to the current invocation; canonical note recovery deliberately continues
/// to use the fixed truncate-capable temporary above.
pub(crate) fn prepare_temporary_exclusive(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> Result<(), AtomicPublishError> {
    if target.parent() != temporary.parent() {
        return Err(AtomicPublishError::DifferentDirectories);
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .share_mode(SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE)
        .open(temporary)
        .map_err(AtomicPublishError::TempCreate)?;
    file.write_all(bytes)
        .map_err(AtomicPublishError::TempWrite)?;
    file.flush().map_err(AtomicPublishError::TempFlush)?;
    flush_windows_handle(&file).map_err(AtomicPublishError::TempFlush)
}

fn open_fixed_temporary(path: &Path) -> Result<std::fs::File, AtomicPublishError> {
    let create = || {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .share_mode(SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT_VALUE);
        options
    };
    match create().create_new(true).open(path) {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let file = create()
                .open(path)
                .map_err(AtomicPublishError::TempCreate)?;
            let metadata = file.metadata().map_err(AtomicPublishError::TempInspect)?;
            let links = observe_open_file(&file)
                .map_err(AtomicPublishError::TempInspect)?
                .link_count();
            if !metadata.file_type().is_file()
                || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_VALUE != 0
                || links != 1
            {
                return Err(AtomicPublishError::UnsafeTemporary {
                    path: path.to_path_buf(),
                    links,
                });
            }
            file.set_len(0).map_err(AtomicPublishError::TempCreate)?;
            Ok(file)
        }
        Err(error) => Err(AtomicPublishError::TempCreate(error)),
    }
}

/// Publish a previously flushed temporary file without any fallback retry.
pub(crate) fn publish_prepared(target: &Path, temporary: &Path) -> Result<(), AtomicPublishError> {
    if target
        .try_exists()
        .map_err(|source| AtomicPublishError::StateInspection {
            path: target.to_path_buf(),
            source,
        })?
    {
        publish_prepared_existing(target, temporary)
    } else {
        publish_prepared_new(target, temporary)
    }
}

/// Replace an already-observed target. Callers must choose this disposition
/// before crossing the final OCC check-to-publish boundary.
pub(crate) fn publish_prepared_existing(
    target: &Path,
    temporary: &Path,
) -> Result<(), AtomicPublishError> {
    replace_existing(target, temporary)
}

/// Publish a newly-observed target without replace semantics. If a target
/// appears after the guard, MoveFileExW fails instead of changing disposition.
pub(crate) fn publish_prepared_new(
    target: &Path,
    temporary: &Path,
) -> Result<(), AtomicPublishError> {
    publish_new(target, temporary)
}

fn flush_windows_handle(file: &std::fs::File) -> std::io::Result<()> {
    let handle = HANDLE(file.as_raw_handle());
    // SAFETY: `handle` is borrowed from a live writable `File` and remains valid
    // for the duration of the synchronous call. Ownership stays with `File`.
    unsafe { FlushFileBuffers(handle) }.map_err(std::io::Error::other)
}

fn replace_existing(target: &Path, temporary: &Path) -> Result<(), AtomicPublishError> {
    let target_wide = wide(target);
    let temporary_wide = wide(temporary);
    // SAFETY: both buffers are live, NUL-terminated UTF-16 paths. Backup and
    // reserved pointers are null. Flags are deliberately zero; the documented
    // REPLACEFILE_WRITE_THROUGH value is unsupported.
    let result = unsafe {
        ReplaceFileW(
            PCWSTR(target_wide.as_ptr()),
            PCWSTR(temporary_wide.as_ptr()),
            PCWSTR::null(),
            REPLACE_FILE_FLAGS(0),
            None,
            None,
        )
    };
    if let Err(source) = result {
        let raw_code = (source.code().0 as u32) & 0xffff;
        let kind = classify_replace_error(raw_code);
        let target_exists =
            target
                .try_exists()
                .map_err(|source| AtomicPublishError::StateInspection {
                    path: target.to_path_buf(),
                    source,
                })?;
        let temporary_exists =
            temporary
                .try_exists()
                .map_err(|source| AtomicPublishError::StateInspection {
                    path: temporary.to_path_buf(),
                    source,
                })?;
        return Err(AtomicPublishError::Replace {
            kind,
            raw_code,
            target_exists,
            temporary_exists,
            source,
        });
    }
    Ok(())
}

fn publish_new(target: &Path, temporary: &Path) -> Result<(), AtomicPublishError> {
    let target_wide = wide(target);
    let temporary_wide = wide(temporary);
    // SAFETY: both buffers are live, NUL-terminated same-directory UTF-16 paths.
    // No replace flag is used, so a concurrently-created target fails closed.
    unsafe {
        MoveFileExW(
            PCWSTR(temporary_wide.as_ptr()),
            PCWSTR(target_wide.as_ptr()),
            MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(AtomicPublishError::PublishNew)
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[derive(Debug, Error)]
pub enum AtomicPublishError {
    #[error("target and temporary files must share a directory")]
    DifferentDirectories,
    #[error("cannot create or truncate the temporary file: {0}")]
    TempCreate(std::io::Error),
    #[error("cannot inspect the temporary file: {0}")]
    TempInspect(std::io::Error),
    #[error("temporary path is not a single-link plain file: {path}; links={links}")]
    UnsafeTemporary { path: PathBuf, links: u32 },
    #[error("cannot write the temporary file: {0}")]
    TempWrite(std::io::Error),
    #[error("cannot flush the temporary file: {0}")]
    TempFlush(std::io::Error),
    #[error(
        "ReplaceFileW failed ({kind:?}, code {raw_code}); target_exists={target_exists}, temp_exists={temporary_exists}: {source}"
    )]
    Replace {
        kind: ReplaceFailureKind,
        raw_code: u32,
        target_exists: bool,
        temporary_exists: bool,
        source: windows::core::Error,
    },
    #[error("cannot publish a new target: {0}")]
    PublishNew(windows::core::Error),
    #[error("cannot inspect replacement state at {path}: {source}")]
    StateInspection {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("injected failure at {stage:?}: {source}")]
    Injected {
        stage: AtomicStage,
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::path::PathBuf;

    fn unique_dir() -> PathBuf {
        unique_temp_path("atomic")
    }

    #[test]
    fn rare_replace_failures_are_not_collapsed() {
        assert_eq!(
            classify_replace_error(1175),
            ReplaceFailureKind::UnableToRemoveReplaced
        );
        assert_eq!(
            classify_replace_error(1176),
            ReplaceFailureKind::UnableToMoveReplacement
        );
        assert_eq!(
            classify_replace_error(1177),
            ReplaceFailureKind::PartialMutation
        );
        assert_eq!(classify_replace_error(5), ReplaceFailureKind::Unclassified);
    }

    #[test]
    fn new_and_existing_targets_are_published_as_complete_bytes() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        atomic_publish(&target, &temporary, b"old complete").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old complete");
        for index in 0..100 {
            let bytes = format!("new complete {index} {}", "x".repeat(index));
            atomic_publish(&target, &temporary, bytes.as_bytes()).unwrap();
            assert_eq!(fs::read(&target).unwrap(), bytes.as_bytes());
            assert!(!temporary.exists());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_new_disposition_never_switches_to_replace_when_target_appears() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        prepare_temporary(&target, &temporary, b"local").unwrap();
        fs::write(&target, b"external").unwrap();
        assert!(publish_prepared_new(&target, &temporary).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"external");
        assert_eq!(fs::read(&temporary).unwrap(), b"local");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exclusive_export_temporary_never_truncates_a_preexisting_file() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("copy.md");
        let temporary = root.join(".stickymd-export-existing.tmp");
        fs::write(&temporary, b"user evidence").unwrap();
        let result = prepare_temporary_exclusive(&target, &temporary, b"new export");
        assert!(matches!(result, Err(AtomicPublishError::TempCreate(_))));
        assert_eq!(fs::read(&temporary).unwrap(), b"user evidence");
        assert!(!target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fixed_temporary_rejects_a_hard_link_without_truncating_user_bytes() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let user_file = root.join("user-evidence.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&user_file, b"user evidence").unwrap();
        fs::hard_link(&user_file, &temporary).unwrap();

        let result = atomic_publish(&target, &temporary, b"new note");
        assert!(matches!(
            result,
            Err(AtomicPublishError::UnsafeTemporary { links, .. }) if links >= 2
        ));
        assert_eq!(fs::read(&user_file).unwrap(), b"user evidence");
        assert_eq!(fs::read(&temporary).unwrap(), b"user evidence");
        assert!(!target.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_stages_never_truncate_the_canonical_note() {
        for stage in [
            AtomicStage::BeforeTempCreate,
            AtomicStage::BeforeTempWrite,
            AtomicStage::AfterTempWrite,
            AtomicStage::AfterTempFlush,
            AtomicStage::BeforeReplace,
        ] {
            let root = unique_dir();
            fs::create_dir(&root).unwrap();
            let target = root.join("note.md");
            let temporary = root.join("note.md.tmp");
            fs::write(&target, b"old complete").unwrap();
            let result =
                atomic_publish_with_observer(&target, &temporary, b"new complete", |observed| {
                    if observed == stage {
                        Err(std::io::Error::other("injected"))
                    } else {
                        Ok(())
                    }
                });
            assert!(matches!(result, Err(AtomicPublishError::Injected { .. })));
            assert_eq!(fs::read(&target).unwrap(), b"old complete");
            if !matches!(
                stage,
                AtomicStage::BeforeTempCreate | AtomicStage::BeforeTempWrite
            ) {
                assert_eq!(fs::read(&temporary).unwrap(), b"new complete");
            }
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn phase9_disk_full_injection_before_write_preserves_canonical_note() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"durable old bytes").unwrap();

        let result = atomic_publish_with_observer(
            &target,
            &temporary,
            b"new bytes that must not publish",
            |stage| {
                if stage == AtomicStage::BeforeTempWrite {
                    Err(std::io::Error::from_raw_os_error(112))
                } else {
                    Ok(())
                }
            },
        );

        assert!(matches!(
            result,
            Err(AtomicPublishError::Injected {
                stage: AtomicStage::BeforeTempWrite,
                ..
            })
        ));
        assert_eq!(fs::read(&target).unwrap(), b"durable old bytes");
        assert_eq!(fs::metadata(&temporary).unwrap().len(), 0);
        fs::remove_dir_all(root).unwrap();
    }
}
