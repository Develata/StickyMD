//! Isolated portable bootstrap and instance checks using smoke-owned child guards.
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::path::Path;

#[cfg(not(windows))]
pub(super) fn verify(_: &Path, _: &Path) -> Result<(), String> {
    Err("NOT_TESTED: packaged runtime verification requires Windows".to_owned())
}

#[cfg(windows)]
pub(super) fn verify(exe: &Path, runtime_root: &Path) -> Result<(), String> {
    use crate::{
        managed_process::ChildGuard,
        qualification_environment::{self, QualificationEnvironmentStatus},
    };
    use std::{
        fs, thread,
        time::{Duration, Instant},
    };

    let environment = qualification_environment::inspect();
    if environment.status != QualificationEnvironmentStatus::Valid {
        return Err(format!("NOT_TESTED: {}", environment.summary()));
    }
    let mut children = Vec::new();
    let mut directories = Vec::new();
    for name in ["ascii", "with space", "中文便签"] {
        let directory = runtime_root.join(name);
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let copied_exe = directory.join("StickyMD.exe");
        fs::copy(exe, &copied_exe).map_err(|error| error.to_string())?;
        let mut child = ChildGuard::start(&copied_exe)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if !child.is_running()? {
                return Err(format!(
                    "Packaged runtime exited early in {}",
                    directory.display()
                ));
            }
            if ["note/note.md", "note/config.toml"]
                .iter()
                .all(|name| directory.join(name).is_file())
            {
                break;
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "Packaged runtime did not bootstrap portable files in {}",
                    directory.display()
                ));
            }
            thread::sleep(Duration::from_millis(50));
        }
        directories.push(directory);
        children.push(child);
    }
    let primary = &directories[0];
    let before = snapshot(primary)?;
    let mut secondary = ChildGuard::start(&primary.join("StickyMD.exe"))?;
    secondary.wait_for_exit(Duration::from_secs(5))?;
    if before != snapshot(primary)? {
        return Err("Same-directory packaged secondary modified durable files".to_owned());
    }
    for child in &mut children {
        if !child.is_running()? {
            return Err(format!(
                "Different-directory packaged instance exited early: {}",
                child.id()
            ));
        }
    }
    for child in &mut children {
        child.kill_and_wait()?;
    }
    println!(
        "PACKAGE_RUNTIME=PASS (ASCII, space, Chinese, same-directory and different-directory)"
    );
    Ok(())
}

#[cfg(any(windows, test))]
#[derive(Debug, Eq, PartialEq)]
struct FileSnapshot {
    bytes: Vec<u8>,
    modified: std::time::SystemTime,
}

#[cfg(any(windows, test))]
fn snapshot(directory: &Path) -> Result<Vec<FileSnapshot>, String> {
    ["note/note.md", "note/config.toml"]
        .iter()
        .map(|name| {
            let path = directory.join(name);
            Ok(FileSnapshot {
                // Bootstrap notes may be empty. certutil refuses empty files on Windows;
                // exact bytes preserve the original unchanged-content assertion without it.
                bytes: std::fs::read(&path).map_err(|error| error.to_string())?,
                modified: std::fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_snapshot_accepts_empty_note_and_detects_changes_or_missing_files() {
        let temporary =
            crate::release::temporary::TemporaryDirectory::new("bootstrap-snapshot-test").unwrap();
        let root = temporary.path();
        std::fs::create_dir(root.join("note")).unwrap();
        std::fs::write(root.join("note/note.md"), []).unwrap();
        std::fs::write(root.join("note/config.toml"), b"config").unwrap();
        let before = snapshot(root).unwrap();
        assert_eq!(before, snapshot(root).unwrap());
        std::fs::write(root.join("note/note.md"), "中文 changed").unwrap();
        assert_ne!(before, snapshot(root).unwrap());
        std::fs::remove_file(root.join("note/config.toml")).unwrap();
        assert!(snapshot(root).is_err());
    }
}
