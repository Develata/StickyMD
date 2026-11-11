//! One running copy per program directory: a lock on the directory itself, and a socket through
//! which a second start asks the first to come forward.
//!
//! plan_ref: docs/plan/05_document_persistence.md#program-directory-identity

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use thiserror::Error;

/// `sockaddr_un.sun_path` is 104 bytes on macOS, the terminator included.
const SOCKET_PATH_LIMIT: usize = 103;
const WAKE_RETRIES_MS: [u64; 3] = [50, 150, 300];

#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("cannot open the program directory: {0}")]
    Open(io::Error),
    #[error("cannot lock the program directory: {0}")]
    Lock(io::Error),
    #[error("cannot create the wake socket: {0}")]
    Socket(io::Error),
}

pub enum InstanceDisposition {
    Primary(InstanceGuard),
    /// Another copy runs from this directory and was asked to come forward, or could not be
    /// reached; either way this copy must not start a second session (plan 05).
    SecondarySignaled,
}

/// Held for the life of the process. The kernel releases the lock when the process ends, however
/// it ends.
pub struct InstanceGuard {
    _lock: File,
    socket_path: Option<PathBuf>,
    listener: Option<UnixListener>,
}

impl InstanceGuard {
    pub fn acquire(program_dir: &Path) -> Result<InstanceDisposition, InstanceError> {
        // No file is created, so a second start leaves nothing behind (plan 05 startup order).
        let lock = File::open(program_dir).map_err(InstanceError::Open)?;
        let socket_path = socket_path_for(&identity(&lock)?);
        // SAFETY: the descriptor is valid for the lifetime of `lock`.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::WouldBlock {
                if let Some(path) = &socket_path {
                    let _ = ask_first_copy(path);
                }
                return Ok(InstanceDisposition::SecondarySignaled);
            }
            return Err(InstanceError::Lock(error));
        }
        let listener = match &socket_path {
            Some(path) => {
                // Holding the lock proves no other copy owns this path: what is there is stale.
                let _ = std::fs::remove_file(path);
                Some(UnixListener::bind(path).map_err(InstanceError::Socket)?)
            }
            None => None,
        };
        Ok(InstanceDisposition::Primary(Self {
            _lock: lock,
            socket_path,
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
        if let Some(path) = &self.socket_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn identity(directory: &File) -> Result<[u8; 8], InstanceError> {
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

    fn primary(root: &Path) -> InstanceGuard {
        match InstanceGuard::acquire(root).unwrap() {
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
            InstanceGuard::acquire(&root).unwrap(),
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
            InstanceGuard::acquire(&root).unwrap(),
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
        fs::write(&path, b"stale").unwrap();
        let again = primary(&root);
        assert!(again.listener.is_some());
        drop(again);
        fs::remove_dir(root).unwrap();
    }
}
