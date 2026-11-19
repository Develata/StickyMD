//! Finding and opening the Note Root: the directory that holds `note/` and is the note's identity.
//! A bundle placed directly in `/Applications` or `~/Applications` is the installed copy, whose
//! note lives under `~/Library/Application Support/StickyMD/<key>/`; anywhere else the note lives
//! next to the bundle. The rule itself is the pure [`placement`]; this module reads what it needs
//! and opens the chain of directories one handle at a time.
//!
//! plan_ref: docs/plan/05_document_persistence.md#note-root

pub mod placement;
#[cfg(test)]
mod tests;

use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::platform::macos::child_directory::is_not_a_plain_directory;
use crate::platform::macos::file::DirectoryHandle;
use crate::platform::macos::user::{home_directory, real_user_id};
use placement::{Candidate, Identity, KeyError, Placement, bundle_of, note_key, placement};

/// The product's folder in `Application Support`, named for people finding it in Finder.
const PRODUCT_DIRECTORY: &str = "StickyMD";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The executable is not in a bundle; the note lives next to it.
    Development,
    /// The bundle is anywhere but directly in an `Applications` directory.
    Portable,
    /// The bundle is directly in `/Applications` or `~/Applications`.
    Installed { key: String },
}

/// The opened Note Root. `handle` is what the lock, the writable check and `note/` are taken from;
/// `path` only says where it was when opened.
#[derive(Debug)]
pub struct NoteRoot {
    pub mode: Mode,
    pub path: PathBuf,
    pub handle: DirectoryHandle,
}

/// Where the rule looks. The program uses [`Roots::current`]; tests point both at directories of
/// their own, so no test reads or writes the real `Applications` or `Application Support`.
pub struct Roots {
    pub system_applications: PathBuf,
    /// The home directory from the user database, or why it is unknown.
    pub home: Result<PathBuf, String>,
}

impl Roots {
    pub fn current() -> Self {
        Self {
            system_applications: PathBuf::from("/Applications"),
            home: home_directory().map_err(|error| error.to_string()),
        }
    }

    fn application_support(home: &Path) -> PathBuf {
        home.join("Library").join("Application Support")
    }
}

#[derive(Debug, Error)]
pub enum NoteRootError {
    #[error("cannot locate the executable: {0}")]
    CurrentExecutable(io::Error),
    #[error("cannot resolve the executable's path: {0}")]
    Canonicalize(io::Error),
    #[error("the executable has no parent directory")]
    ExecutableHasNoParent,
    #[error("cannot open {}: {source}", path.display())]
    Open { path: PathBuf, source: io::Error },
    #[error("cannot tell whether the app is in Applications: {0}")]
    Undecidable(String),
    #[error("{0}")]
    Key(KeyError),
    #[error("the home directory is unknown: {0}")]
    HomeUnknown(String),
    #[error("{} does not exist or is not a directory", .0.display())]
    ApplicationSupportMissing(PathBuf),
    #[error("{} is a symbolic link or not a directory", .0.display())]
    NotADirectory(PathBuf),
    #[error("{} belongs to another user", .0.display())]
    ForeignOwner(PathBuf),
}

impl NoteRoot {
    pub fn resolve_current() -> Result<Self, NoteRootError> {
        let executable = std::env::current_exe().map_err(NoteRootError::CurrentExecutable)?;
        Self::resolve(&executable, &Roots::current())
    }

    pub fn resolve(executable: &Path, roots: &Roots) -> Result<Self, NoteRootError> {
        let real = std::fs::canonicalize(executable).map_err(NoteRootError::Canonicalize)?;
        let Some((parent, bundle_name)) = bundle_of(&real) else {
            let directory = real.parent().ok_or(NoteRootError::ExecutableHasNoParent)?;
            return Ok(Self {
                mode: Mode::Development,
                handle: open(directory)?,
                path: directory.to_owned(),
            });
        };
        // The portable branch keeps this handle rather than opening the path again.
        let handle = open(parent)?;
        let identity = identity_of(&handle).map_err(|source| NoteRootError::Open {
            path: parent.to_owned(),
            source,
        })?;
        let mut candidates = vec![probe(&roots.system_applications)];
        if let Ok(home) = &roots.home {
            candidates.push(probe(&home.join("Applications")));
        }
        match placement(
            identity,
            &candidates,
            roots.home.as_ref().err().map(String::as_str),
        ) {
            Placement::Portable => Ok(Self {
                mode: Mode::Portable,
                path: parent.to_owned(),
                handle,
            }),
            Placement::Undecidable(reason) => Err(NoteRootError::Undecidable(reason)),
            Placement::Installed => {
                let key = note_key(bundle_name).map_err(NoteRootError::Key)?;
                let home = roots
                    .home
                    .as_ref()
                    .map_err(|reason| NoteRootError::HomeUnknown(reason.clone()))?;
                open_installed(&Roots::application_support(home), key)
            }
        }
    }
}

/// The trust chain: `Application Support` by path (never created), then `StickyMD` and `<key>`,
/// each opened relative to the one before without following a link, created when absent, and
/// checked to be a directory of this user before the next level is entered. A level that fails
/// the check stops the start; nothing is created in it, removed or changed.
fn open_installed(start: &Path, key: String) -> Result<NoteRoot, NoteRootError> {
    let support = DirectoryHandle::open_following(start).map_err(|source| {
        if matches!(
            source.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        ) {
            NoteRootError::ApplicationSupportMissing(start.to_owned())
        } else {
            NoteRootError::Open {
                path: start.to_owned(),
                source,
            }
        }
    })?;
    let product_path = start.join(PRODUCT_DIRECTORY);
    let product = private_level(&support, PRODUCT_DIRECTORY, &product_path)?;
    let path = product_path.join(&key);
    let handle = private_level(&product, &key, &path)?;
    Ok(NoteRoot {
        mode: Mode::Installed { key },
        path,
        handle,
    })
}

/// A missing level is created with `0700` requested (the umask can only narrow it). No ACL is
/// evaluated and an existing directory need not be `0700`: as with a portable note, the
/// directory's permissions are the user's business.
fn private_level(
    parent: &DirectoryHandle,
    name: &str,
    path: &Path,
) -> Result<DirectoryHandle, NoteRootError> {
    let handle = parent.open_or_create_child(name, true).map_err(|source| {
        if is_not_a_plain_directory(&source) {
            NoteRootError::NotADirectory(path.to_owned())
        } else {
            NoteRootError::Open {
                path: path.to_owned(),
                source,
            }
        }
    })?;
    let metadata = handle.metadata().map_err(|source| NoteRootError::Open {
        path: path.to_owned(),
        source,
    })?;
    check_level(metadata.is_dir(), metadata.uid(), real_user_id(), path)?;
    Ok(handle)
}

/// A level is entered only when it is a directory that belongs to the user running the program.
fn check_level(is_dir: bool, owner: u32, user: u32, path: &Path) -> Result<(), NoteRootError> {
    if !is_dir {
        Err(NoteRootError::NotADirectory(path.to_owned()))
    } else if owner != user {
        Err(NoteRootError::ForeignOwner(path.to_owned()))
    } else {
        Ok(())
    }
}

fn open(directory: &Path) -> Result<DirectoryHandle, NoteRootError> {
    DirectoryHandle::open(directory).map_err(|source| NoteRootError::Open {
        path: directory.to_owned(),
        source,
    })
}

fn identity_of(handle: &DirectoryHandle) -> io::Result<Identity> {
    let metadata = handle.metadata()?;
    Ok(Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

/// A candidate after following links and firmlinks.
fn probe(candidate: &Path) -> Candidate {
    match std::fs::metadata(candidate) {
        Ok(metadata) => Candidate::Found(Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        }),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Candidate::Absent
        }
        Err(error) => Candidate::Unreadable(format!("{}: {error}", candidate.display())),
    }
}
