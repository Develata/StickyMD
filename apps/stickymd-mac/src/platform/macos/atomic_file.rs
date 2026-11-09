//! Whole-file publication by path, for files outside the note's own handle: `config.toml` and an
//! export the user placed somewhere.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use thiserror::Error;

use super::file::{DirectoryHandle, full_sync, rename_path_no_replace};

#[derive(Debug, Error)]
pub enum AtomicPublishError {
    #[error("target and temporary paths must name files in one directory")]
    DifferentDirectories,
    #[error("cannot open the directory: {0}")]
    Directory(io::Error),
    #[error("cannot create the temporary file: {0}")]
    TempCreate(io::Error),
    #[error("cannot write the temporary file: {0}")]
    TempWrite(io::Error),
    #[error("cannot flush the temporary file: {0}")]
    TempFlush(io::Error),
    #[error("cannot replace the target: {0}")]
    Replace(io::Error),
    #[error("cannot flush the directory: {0}")]
    DirectoryFlush(io::Error),
}

struct Site {
    directory: DirectoryHandle,
    target: String,
    temporary: String,
}

fn site(target: &Path, temporary: &Path) -> Result<Site, AtomicPublishError> {
    let parent = target
        .parent()
        .filter(|parent| Some(*parent) == temporary.parent())
        .ok_or(AtomicPublishError::DifferentDirectories)?;
    let name = |path: &Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
            .ok_or(AtomicPublishError::DifferentDirectories)
    };
    // The user may reach a destination through a link; only the last component is checked.
    let real = fs::canonicalize(parent).map_err(AtomicPublishError::Directory)?;
    Ok(Site {
        directory: DirectoryHandle::open(&real).map_err(AtomicPublishError::Directory)?,
        target: name(target)?,
        temporary: name(temporary)?,
    })
}

fn write_durably(mut file: fs::File, bytes: &[u8]) -> Result<(), AtomicPublishError> {
    file.write_all(bytes)
        .map_err(AtomicPublishError::TempWrite)?;
    full_sync(&file).map_err(AtomicPublishError::TempFlush)
}

fn publish(site: &Site) -> Result<(), AtomicPublishError> {
    site.directory
        .replace(&site.temporary, &site.target)
        .map_err(AtomicPublishError::Replace)?;
    site.directory
        .sync()
        .map_err(AtomicPublishError::DirectoryFlush)
}

pub fn atomic_publish(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> Result<(), AtomicPublishError> {
    let site = site(target, temporary)?;
    let file = site
        .directory
        .create_or_truncate(&site.temporary)
        .map_err(AtomicPublishError::TempCreate)?;
    write_durably(file, bytes)?;
    publish(&site)
}

/// Fails with `TempCreate(AlreadyExists)` when the name is taken.
pub fn prepare_temporary_exclusive(
    target: &Path,
    temporary: &Path,
    bytes: &[u8],
) -> Result<(), AtomicPublishError> {
    let site = site(target, temporary)?;
    let file = site
        .directory
        .create_new(&site.temporary)
        .map_err(AtomicPublishError::TempCreate)?;
    write_durably(file, bytes)
}

pub fn publish_prepared(target: &Path, temporary: &Path) -> Result<(), AtomicPublishError> {
    publish(&site(target, temporary)?)
}

pub fn move_file_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    rename_path_no_replace(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;

    #[test]
    fn publishing_replaces_the_target_and_consumes_the_temporary() {
        let root = unique_temp_path("atomic");
        fs::create_dir(&root).unwrap();
        let (target, temporary) = (root.join("a.toml"), root.join("a.toml.tmp"));
        fs::write(&target, b"old").unwrap();
        atomic_publish(&target, &temporary, b"new").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new");
        assert!(!temporary.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_exclusive_temporary_refuses_a_taken_name() {
        let root = unique_temp_path("atomic-exclusive");
        fs::create_dir(&root).unwrap();
        let (target, temporary) = (root.join("a.md"), root.join("a.tmp"));
        fs::write(&temporary, b"taken").unwrap();
        let error = prepare_temporary_exclusive(&target, &temporary, b"x").unwrap_err();
        assert!(matches!(
            error,
            AtomicPublishError::TempCreate(cause) if cause.kind() == io::ErrorKind::AlreadyExists
        ));
        assert_eq!(fs::read(&temporary).unwrap(), b"taken");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn files_in_different_directories_are_refused() {
        assert!(matches!(
            atomic_publish(Path::new("/a/x"), Path::new("/b/x"), b""),
            Err(AtomicPublishError::DifferentDirectories)
        ));
    }
}
