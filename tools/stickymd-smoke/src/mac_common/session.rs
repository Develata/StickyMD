//! Own only this run's directory and child, including setup failures and unwind.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) struct Session {
    path: PathBuf,
    device: u64,
    inode: u64,
    pub child: Option<Child>,
    cleaned: bool,
    opened_executable: Option<PathBuf>,
    retained: Option<String>,
}

impl Session {
    pub fn new() -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let parent = fs::canonicalize(std::env::temp_dir()).map_err(|e| e.to_string())?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = parent.join(format!(
            "stickymd-mac-session-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|e| format!("create {}: {e}", path.display()))?;
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
            child: None,
            cleaned: false,
            opened_executable: None,
            retained: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The created directory's identity, for code that must act inside it without holding the
    /// session (a guard that runs while the session is borrowed or unwinding).
    pub fn anchor(&self) -> Anchor {
        Anchor {
            path: self.path.clone(),
            device: self.device,
            inode: self.inode,
        }
    }

    pub fn spawn(&mut self, command: &mut Command) -> Result<(), String> {
        if self.child.is_some() {
            return Err("A session already owns a child".into());
        }
        self.child = Some(command.spawn().map_err(|e| format!("spawn: {e}"))?);
        Ok(())
    }

    pub fn poll(&mut self) -> Result<Option<ExitStatus>, String> {
        self.child
            .as_mut()
            .ok_or("Session has no child")?
            .try_wait()
            .map_err(|e| format!("try_wait: {e}"))
    }

    pub fn open_bundle(
        &mut self,
        open: impl FnOnce(&Path) -> Result<String, String>,
    ) -> Result<String, String> {
        if let Some(status) = self.poll()? {
            return Err(format!(
                "refused open: owned child already exited: {status}"
            ));
        }
        let pid = self.child.as_ref().ok_or("missing owned child")?.id();
        // Freeze the same canonical executable used at spawn, including an allowed
        // bundle-internal symlink; do not resolve a possibly changed path at cleanup.
        self.opened_executable = Some(super::fixture::contained_executable(
            &self.path.join("StickyMD.app"),
        )?);
        // Also retain on unwind between open and audit, when a new PID may exist.
        self.retained = Some("open process audit did not complete".into());
        let result = open(&self.path.join("StickyMD.app"));
        let foreign = self.audit_processes(pid)?;
        if !foreign.is_empty() {
            return Err(format!(
                "open created foreign fixture PIDs {foreign:?}; terminated and confirmed gone; open result={result:?}"
            ));
        }
        result
    }

    fn audit_processes(&mut self, owned: u32) -> Result<Vec<u32>, String> {
        let executable = self
            .opened_executable
            .as_ref()
            .ok_or("missing opened executable")?;
        let result = super::processes::drain(
            &mut super::processes::Native {
                executable,
                started: Instant::now(),
            },
            owned,
        );
        self.retained = result.as_ref().err().cloned();
        result
    }

    pub fn wait_until(&mut self, deadline: Instant) -> Result<Option<ExitStatus>, String> {
        loop {
            if let Some(status) = self.poll()? {
                return Ok(Some(status));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }
            std::thread::sleep(remaining.min(Duration::from_millis(100)));
        }
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            self.child = None;
            return Ok(());
        }
        let term = crate::mac_common::native::terminate(child.id());
        if self
            .wait_until(Instant::now() + Duration::from_secs(3))?
            .is_none()
        {
            self.child
                .as_mut()
                .ok_or("Missing child")?
                .kill()
                .map_err(|e| format!("SIGKILL: {e}; SIGTERM: {term:?}"))?;
            if self
                .wait_until(Instant::now() + Duration::from_secs(3))?
                .is_none()
            {
                return Err(format!(
                    "Child was not reaped after SIGKILL; retained {}",
                    self.path.display()
                ));
            }
        }
        self.child = None;
        Ok(())
    }

    pub fn cleanup(&mut self) -> Result<(), String> {
        if self.cleaned {
            return Ok(());
        }
        self.stop()?;
        if self.retained.is_none() && self.opened_executable.is_some() {
            // The child is reaped now, so every remaining matching PID is foreign.
            let _ = self.audit_processes(0);
        }
        if let Some(reason) = &self.retained {
            return Err(format!(
                "{reason}; retained fixture {}; stop the whole run",
                self.path.display()
            ));
        }
        validate_directory(&self.path, self.device, self.inode)?;
        fs::remove_dir_all(&self.path)
            .map_err(|e| format!("remove {}: {e}", self.path.display()))?;
        self.cleaned = true;
        Ok(())
    }
}

/// The session directory as created: its path and the device and inode it had then.
#[derive(Clone, Debug)]
pub(crate) struct Anchor {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl Anchor {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Refuses a directory that was replaced, moved or linked since the session created it.
    pub fn validate(&self) -> Result<(), String> {
        validate_directory(&self.path, self.device, self.inode)
    }
}

fn validate_directory(path: &Path, device: u64, inode: u64) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.dev() != device
        || metadata.ino() != inode
        || fs::canonicalize(path).map_err(|e| e.to_string())? != path
    {
        return Err(format!(
            "Refusing cleanup: created directory identity changed at {}",
            path.display()
        ));
    }
    Ok(())
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("macOS session cleanup: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_refuses_an_exited_child_without_invoking_launch_services() {
        let mut session = Session::new().unwrap();
        session.spawn(&mut Command::new("/usr/bin/true")).unwrap();
        assert!(
            session
                .wait_until(Instant::now() + Duration::from_secs(3))
                .unwrap()
                .is_some()
        );
        let error = session
            .open_bundle(|_| panic!("must not invoke open"))
            .unwrap_err();
        assert!(error.contains("already exited"));
        session.cleanup().unwrap();
    }

    #[test]
    fn uncertain_foreign_process_retains_fixture_even_on_drop() {
        let mut session = Session::new().unwrap();
        let path = session.path.clone();
        session.retained = Some("foreign PID not confirmed gone".into());
        let error = session.cleanup().unwrap_err();
        assert!(error.contains("retained fixture") && error.contains("stop the whole run"));
        drop(session);
        assert!(path.exists());
        fs::remove_dir(&path).unwrap(); // Reclaim only this test's empty fixture.
    }

    #[test]
    fn cleanup_refuses_a_replaced_root_and_does_not_follow_child_symlinks() {
        let mut session = Session::new().unwrap();
        let original = session.path.clone();
        let moved = original.with_extension("moved");
        fs::rename(&original, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &original).unwrap();
        assert!(session.cleanup().unwrap_err().contains("Refusing cleanup"));
        assert!(moved.is_dir());
        fs::remove_file(&original).unwrap();
        fs::rename(&moved, &original).unwrap();
        let mut other = Session::new().unwrap();
        crate::atomic_evidence::write(&other.path.join("keep"), b"owned by other session").unwrap();
        std::os::unix::fs::symlink(other.path(), original.join("outside")).unwrap();
        session.cleanup().unwrap();
        assert!(other.path.join("keep").is_file());
        other.cleanup().unwrap();
    }

    #[test]
    fn unwind_stops_only_the_owned_headless_child_and_removes_its_directory() {
        let session = Session::new().unwrap();
        let path = session.path.clone();
        let result = std::panic::catch_unwind(move || {
            let mut session = session;
            session.spawn(Command::new("/bin/sleep").arg("30")).unwrap();
            panic!("simulated post-spawn failure");
        });
        assert!(result.is_err());
        assert!(!path.exists());
    }
}
