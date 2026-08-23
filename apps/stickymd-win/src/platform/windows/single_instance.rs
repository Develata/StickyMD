//! Same-program-directory named mutex and wake event.
//!
//! plan_ref: docs/plan/09_windows_shell.md#single-instance

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use thiserror::Error;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForMultipleObjects,
};
use windows::core::HSTRING;

use super::program_dir::ProgramDirectory;

pub enum InstanceDisposition {
    Primary(SingleInstanceGuard),
    SecondarySignaled,
}

pub struct SingleInstanceGuard {
    mutex: HANDLE,
    show_event: HANDLE,
    stop_event: HANDLE,
    listener: Option<JoinHandle<()>>,
}

impl SingleInstanceGuard {
    pub fn acquire(
        directory: &ProgramDirectory,
    ) -> Result<InstanceDisposition, SingleInstanceError> {
        let show_name = HSTRING::from(directory.show_event_name());
        let mutex_name = HSTRING::from(directory.mutex_name());

        // SAFETY: names are immutable NUL-terminated HSTRING values for the call;
        // default security attributes are requested and returned handles are owned.
        let show_event = unsafe { CreateEventW(None, false, false, &show_name) }
            .map_err(SingleInstanceError::CreateShowEvent)?;
        // SAFETY: same lifetime and ownership conditions as above. Initial ownership
        // is unnecessary; existence of the named object is the instance boundary.
        let mutex = match unsafe { CreateMutexW(None, false, &mutex_name) } {
            Ok(handle) => handle,
            Err(error) => {
                close_handle(show_event);
                return Err(SingleInstanceError::CreateMutex(error));
            }
        };
        // SAFETY: immediately queries this thread's last error from CreateMutexW.
        let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if already_exists {
            // The event was created before the mutex lookup, so simultaneous starts
            // cannot observe the mutex without a matching wake object.
            // SAFETY: `show_event` is a live event handle returned above.
            let signal = unsafe { SetEvent(show_event) };
            close_handle(mutex);
            close_handle(show_event);
            signal.map_err(SingleInstanceError::SignalExisting)?;
            return Ok(InstanceDisposition::SecondarySignaled);
        }

        // SAFETY: unnamed auto-reset event with process-local ownership.
        let stop_event = match unsafe { CreateEventW(None, false, false, None) } {
            Ok(handle) => handle,
            Err(error) => {
                close_handle(mutex);
                close_handle(show_event);
                return Err(SingleInstanceError::CreateStopEvent(error));
            }
        };
        Ok(InstanceDisposition::Primary(Self {
            mutex,
            show_event,
            stop_event,
            listener: None,
        }))
    }

    pub fn start_listener<F>(&mut self, on_show: F) -> Result<(), SingleInstanceError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        if self.listener.is_some() {
            return Ok(());
        }
        // HANDLE intentionally does not promise Send. The underlying kernel
        // handle value is process-wide; transfer only its integer value and keep
        // ownership in this guard until the listener has joined.
        let show_raw = self.show_event.0 as usize;
        let stop_raw = self.stop_event.0 as usize;
        let on_show = Arc::new(on_show);
        self.listener = Some(
            thread::Builder::new()
                .name("stickymd-instance-wake".into())
                .stack_size(256 * 1024)
                .spawn(move || {
                    let show_event = HANDLE(show_raw as *mut core::ffi::c_void);
                    let stop_event = HANDLE(stop_raw as *mut core::ffi::c_void);
                    loop {
                        // SAFETY: both event handles remain owned by the guard until
                        // it signals stop and joins this listener.
                        let result = unsafe {
                            WaitForMultipleObjects(&[show_event, stop_event], false, INFINITE)
                        };
                        if result == WAIT_OBJECT_0 {
                            on_show();
                        } else {
                            break;
                        }
                    }
                })
                .map_err(SingleInstanceError::SpawnListener)?,
        );
        Ok(())
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        // SAFETY: stop_event remains live until after the listener joins.
        let _ = unsafe { SetEvent(self.stop_event) };
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        close_handle(self.stop_event);
        close_handle(self.show_event);
        close_handle(self.mutex);
    }
}

fn close_handle(handle: HANDLE) {
    // SAFETY: each handle is owned by this module and closed exactly once after
    // dependent threads have stopped using it.
    let _ = unsafe { CloseHandle(handle) };
}

#[derive(Debug, Error)]
pub enum SingleInstanceError {
    #[error("cannot create the named show event: {0}")]
    CreateShowEvent(windows::core::Error),
    #[error("cannot create the named mutex: {0}")]
    CreateMutex(windows::core::Error),
    #[error("cannot create the listener stop event: {0}")]
    CreateStopEvent(windows::core::Error),
    #[error("cannot signal the existing instance: {0}")]
    SignalExisting(windows::core::Error),
    #[error("cannot start the existing-instance listener: {0}")]
    SpawnListener(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    fn unique_program_directory() -> (PathBuf, ProgramDirectory) {
        let root = unique_temp_path("instance");
        fs::create_dir(&root).unwrap();
        let executable = root.join("StickyMD.exe");
        fs::write(&executable, b"").unwrap();
        let directory = ProgramDirectory::from_executable(&executable).unwrap();
        (root, directory)
    }

    #[test]
    fn same_directory_second_instance_signals_without_creating_files() {
        let (root, directory) = unique_program_directory();
        let InstanceDisposition::Primary(mut first) =
            SingleInstanceGuard::acquire(&directory).unwrap()
        else {
            panic!("first instance must be primary")
        };
        let (sender, receiver) = mpsc::channel();
        first
            .start_listener(move || {
                let _ = sender.send(());
            })
            .unwrap();
        assert!(matches!(
            SingleInstanceGuard::acquire(&directory).unwrap(),
            InstanceDisposition::SecondarySignaled
        ));
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!root.join("note").exists());
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn different_directories_are_independent_instances() {
        let (root_a, directory_a) = unique_program_directory();
        let (root_b, directory_b) = unique_program_directory();
        let first = SingleInstanceGuard::acquire(&directory_a).unwrap();
        let second = SingleInstanceGuard::acquire(&directory_b).unwrap();
        assert!(matches!(first, InstanceDisposition::Primary(_)));
        assert!(matches!(second, InstanceDisposition::Primary(_)));
        drop(first);
        drop(second);
        fs::remove_dir_all(root_a).unwrap();
        fs::remove_dir_all(root_b).unwrap();
    }
}
