//! One running copy per Note Root: a lock on the directory itself, taken on the handle the host
//! opened it with, and a socket through which a second start asks the first to come forward.
//!
//! plan_ref: docs/plan/05_document_persistence.md#program-directory-identity

use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::file::DirectoryHandle;

/// `sockaddr_un.sun_path` is 104 bytes on macOS, the terminator included.
const SOCKET_PATH_LIMIT: usize = 103;
const WAKE_RETRIES_MS: [u64; 3] = [50, 150, 300];

#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("cannot open the note root: {0}")]
    Open(io::Error),
    #[error("cannot lock the note root: {0}")]
    Lock(io::Error),
    #[error("cannot create the wake socket: {0}")]
    Socket(io::Error),
}

pub enum InstanceDisposition {
    Primary(InstanceGuard),
    /// Another copy runs on this Note Root and was asked to come forward, or could not be
    /// reached; either way this copy must not start a second session (plan 05).
    SecondarySignaled,
}

/// Held for the life of the process. The kernel releases the lock when the process ends, however
/// it ends.
pub struct InstanceGuard {
    _lock: DirectoryHandle,
    socket_path: Option<PathBuf>,
    /// `(st_dev, st_ino)` of the socket this guard bound; only that entry is removed on drop.
    bound: Option<(u64, u64)>,
    listener: Option<UnixListener>,
}

impl InstanceGuard {
    /// Locks the directory `note_root` names, not a path that may by now name another one. No
    /// file is created, so a second start leaves nothing behind (plan 05 startup order).
    pub fn acquire(note_root: &DirectoryHandle) -> Result<InstanceDisposition, InstanceError> {
        // A duplicate shares the open directory, so the lock lives as long as this guard.
        let lock = note_root.try_clone().map_err(InstanceError::Open)?;
        let socket_path = socket_path_for(&identity(&lock)?);
        // SAFETY: the descriptor is valid for the lifetime of `lock`.
        if unsafe { libc::flock(lock.directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                if let Some(path) = &socket_path {
                    let _ = ask_first_copy(path);
                }
                return Ok(InstanceDisposition::SecondarySignaled);
            }
            return Err(InstanceError::Lock(error));
        }
        // Holding the lock proves no other copy owns this path, so a socket there is stale. Any
        // other kind of entry is left alone, and this copy runs without the wake listener.
        let socket_path = socket_path.filter(|path| match entry_at(path) {
            Some((_, true)) => std::fs::remove_file(path).is_ok(),
            Some((_, false)) => false,
            None => true,
        });
        let listener = match &socket_path {
            Some(path) => Some(UnixListener::bind(path).map_err(InstanceError::Socket)?),
            None => None,
        };
        let bound = socket_path
            .as_deref()
            .and_then(entry_at)
            .map(|(identity, _)| identity);
        Ok(InstanceDisposition::Primary(Self {
            _lock: lock,
            socket_path,
            bound,
            listener,
        }))
    }

    /// Starts answering second starts. A start that came between the bind and here waits in the
    /// socket's backlog.
    pub fn start_listener(
        &mut self,
        on_wake: impl Fn() + Send + 'static,
    ) -> Result<(), InstanceError> {
        let Some(listener) = self.listener.take() else {
            return Ok(());
        };
        std::thread::Builder::new()
            .name("stickymd-instance".into())
            .spawn(move || {
                for connection in listener.incoming() {
                    if connection.is_ok() {
                        on_wake();
                    }
                }
            })
            .map_err(InstanceError::Socket)?;
        Ok(())
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        if let (Some(path), Some(bound)) = (&self.socket_path, self.bound)
            && entry_at(path) == Some((bound, true))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// The entry at `path` itself (a link is not followed): its identity and whether it is a socket.
fn entry_at(path: &Path) -> Option<((u64, u64), bool)> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let metadata = std::fs::symlink_metadata(path).ok()?;
    Some((
        (metadata.dev(), metadata.ino()),
        metadata.file_type().is_socket(),
    ))
}

fn identity(directory: &DirectoryHandle) -> Result<[u8; 8], InstanceError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = directory.metadata().map_err(InstanceError::Open)?;
    let mut digest = Sha256::new();
    digest.update(metadata.dev().to_le_bytes());
    digest.update(metadata.ino().to_le_bytes());
    let digest = digest.finalize();
    let mut identity = [0; 8];
    identity.copy_from_slice(&digest[..8]);
    Ok(identity)
}

fn socket_path_for(identity: &[u8; 8]) -> Option<PathBuf> {
    let hex: String = identity.iter().map(|byte| format!("{byte:02x}")).collect();
    let path = crate::sys::user_runtime_dir()?.join(format!("stickymd-{hex}.sock"));
    (path.as_os_str().len() <= SOCKET_PATH_LIMIT).then_some(path)
}

/// The first copy may hold the lock and not listen yet.
fn ask_first_copy(path: &Path) -> io::Result<()> {
    let mut outcome = UnixStream::connect(path).map(drop);
    for delay in WAKE_RETRIES_MS {
        if outcome.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(delay));
        outcome = UnixStream::connect(path).map(drop);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::sync::mpsc;

    fn directory(label: &str) -> PathBuf {
        let root = unique_temp_path(label);
        fs::create_dir(&root).unwrap();
        root
    }

    fn acquire(root: &Path) -> InstanceDisposition {
        InstanceGuard::acquire(&DirectoryHandle::open(root).unwrap()).unwrap()
    }

    fn primary(root: &Path) -> InstanceGuard {
        match acquire(root) {
            InstanceDisposition::Primary(guard) => guard,
            InstanceDisposition::SecondarySignaled => panic!("expected the first copy"),
        }
    }

    #[test]
    fn the_socket_path_fits_and_does_not_depend_on_tmpdir() {
        let path = socket_path_for(&[0xab; 8]).unwrap();
        assert!(path.as_os_str().len() <= SOCKET_PATH_LIMIT, "{path:?}");
        assert!(path.ends_with("stickymd-abababababababab.sock"));
        assert_eq!(path.parent(), crate::sys::user_runtime_dir().as_deref());
    }

    #[test]
    fn the_second_start_finds_the_first_and_creates_nothing_in_the_directory() {
        let root = directory("instance-second");
        let mut first = primary(&root);
        let (sender, receiver) = mpsc::channel();
        first
            .start_listener(move || {
                let _ = sender.send(());
            })
            .unwrap();
        assert!(matches!(
            acquire(&root),
            InstanceDisposition::SecondarySignaled
        ));
        receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        drop(first);
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn a_second_start_before_the_listener_is_answered_from_the_backlog() {
        let root = directory("instance-backlog");
        let mut first = primary(&root);
        assert!(matches!(
            acquire(&root),
            InstanceDisposition::SecondarySignaled
        ));
        let (sender, receiver) = mpsc::channel();
        first
            .start_listener(move || {
                let _ = sender.send(());
            })
            .unwrap();
        receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        drop(first);
        fs::remove_dir(root).unwrap();
    }

    /// The lock is on the directory the handle names: after a move it goes with the directory,
    /// and a new directory at the old path is another Note Root.
    #[test]
    fn the_lock_follows_the_handle_not_the_path() {
        let root = directory("instance-handle");
        let handle = DirectoryHandle::open(&root).unwrap();
        let moved = unique_temp_path("instance-handle-moved");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        let first = match InstanceGuard::acquire(&handle).unwrap() {
            InstanceDisposition::Primary(guard) => guard,
            InstanceDisposition::SecondarySignaled => panic!("expected the first copy"),
        };
        drop(handle);
        assert!(matches!(
            acquire(&moved),
            InstanceDisposition::SecondarySignaled
        ));
        let other = primary(&root);
        drop((first, other));
        fs::remove_dir(root).unwrap();
        fs::remove_dir(moved).unwrap();
    }

    #[test]
    fn another_directory_is_another_instance() {
        let first_root = directory("instance-a");
        let second_root = directory("instance-b");
        let first = primary(&first_root);
        let second = primary(&second_root);
        drop((first, second));
        fs::remove_dir(first_root).unwrap();
        fs::remove_dir(second_root).unwrap();
    }

    #[test]
    fn the_lock_ends_with_its_owner_and_a_stale_socket_is_replaced() {
        let root = directory("instance-stale");
        let first = primary(&root);
        let path = first.socket_path.clone().unwrap();
        drop(first);
        // A socket left by a copy that ended without cleaning up.
        drop(UnixListener::bind(&path).unwrap());
        let again = primary(&root);
        assert!(again.listener.is_some());
        drop(again);
        assert!(!path.exists());
        fs::remove_dir(root).unwrap();
    }

    /// Something that is not a socket at the socket's name is not this program's to delete.
    #[test]
    fn a_plain_file_at_the_socket_name_is_kept_and_the_copy_runs_without_wake() {
        let root = directory("instance-plain-file");
        let first = primary(&root);
        let path = first.socket_path.clone().unwrap();
        drop(first);
        fs::write(&path, b"not ours").unwrap();
        let again = primary(&root);
        assert!(again.listener.is_none());
        drop(again);
        assert_eq!(fs::read(&path).unwrap(), b"not ours");
        fs::remove_file(&path).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
