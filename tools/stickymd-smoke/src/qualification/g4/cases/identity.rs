//! Exact canonical-program-directory and second-instance qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::super::exact_desktop::{
    ChildGuard, assert_sole_stickymd_process, io_error, seed_note, wait_for_layout, wait_until,
};

pub(super) fn g4_05(_repository: &Path, program: &Path) -> Result<(), String> {
    const SOURCE: &str = "junction identity must remain singular\n";
    seed_note(program, SOURCE)?;
    let mut primary = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(primary.id())?;
    crate::window_control::request_close(window)?;
    wait_for_visibility(window, false)?;
    thread::sleep(Duration::from_millis(300));

    let alias = program
        .parent()
        .ok_or_else(|| "G4-05 program directory has no parent".to_owned())?
        .join("g4-05-junction");
    create_junction(&alias, program)?;
    let note = program.join("note/note.md");
    let config = program.join("note/config.toml");
    let before = (file_stamp(&note)?, file_stamp(&config)?);
    let mut secondary = ChildGuard::start(&alias.join("StickyMD.exe"))?;
    secondary.wait_for_exit()?;
    wait_for_visibility(window, true)?;
    let after = (file_stamp(&note)?, file_stamp(&config)?);
    if before != after {
        return Err("junction second instance modified durable note/config".to_owned());
    }
    assert_sole_stickymd_process(primary.id())?;
    fs::remove_dir(&alias)
        .map_err(|error| format!("cannot remove G4-05 junction after qualification: {error}"))?;
    primary.kill_and_wait()
}

fn wait_for_visibility(
    window: crate::window_control::WindowHandle,
    expected: bool,
) -> Result<(), String> {
    wait_until("StickyMD window visibility", || {
        Ok(crate::window_control::is_visible(window)? == expected)
    })
}

fn create_junction(alias: &Path, target: &Path) -> Result<(), String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:STICKYMD_JUNCTION_ALIAS -Target $env:STICKYMD_JUNCTION_TARGET | Out-Null",
        ])
        .env("STICKYMD_JUNCTION_ALIAS", alias)
        .env("STICKYMD_JUNCTION_TARGET", target)
        .output()
        .map_err(|error| format!("cannot start junction creator: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "cannot create real Windows junction: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileStamp {
    bytes: Vec<u8>,
    modified: Option<SystemTime>,
}

fn file_stamp(path: &Path) -> Result<FileStamp, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    Ok(FileStamp {
        bytes: fs::read(path).map_err(io_error)?,
        modified: metadata.modified().ok(),
    })
}
