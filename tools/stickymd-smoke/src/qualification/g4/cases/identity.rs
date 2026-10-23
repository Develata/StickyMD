//! Exact canonical-program-directory and second-instance qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};

use super::super::super::exact_desktop::{
    Checks, ChildGuard, assert_sole_stickymd_process, io_error, seed_note,
    start_until_editor_ready, wait_for_config, wait_until,
};

mod visibility;

pub(super) fn g4_05(repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
    const SOURCE: &str = "junction identity must remain singular\n";
    seed_note(program, SOURCE)?;
    let mut primary = start_until_editor_ready(&program.join("StickyMD.exe"))?;
    let window = crate::window_control::visible_window(primary.id())?;
    visibility::wake(program, primary.id(), window, checks)?;
    crate::window_control::request_close(window)?;
    wait_for_visibility(window, false)?;
    checks.record("close-hides-window", &[]);
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
    secondary.wait_for_exit(super::super::super::exact_desktop::TIMEOUT)?;
    checks.record("junction-second-instance-exits", &[]);
    wait_for_visibility(window, true)?;
    checks.record("junction-wakes-primary", &[]);
    let after = (file_stamp(&note)?, file_stamp(&config)?);
    if before != after {
        return Err("junction second instance modified durable note/config".to_owned());
    }
    // Only note.md and config.toml are observed; other durable state is not claimed.
    checks.record("junction-keeps-note-and-config", &[]);
    assert_sole_stickymd_process(primary.id())?;
    checks.record("sole-process-after-junction", &[]);
    collapsed_wake(program, &alias, primary.id(), window, checks)?;
    fs::remove_dir(&alias)
        .map_err(|error| format!("cannot remove G4-05 junction after qualification: {error}"))?;
    primary.kill_and_wait()?;
    portable_copies(repository, program, checks)
}

fn portable_copies(repository: &Path, parent: &Path, checks: &mut Checks) -> Result<(), String> {
    // A fresh portable directory keeps the preceding dock state out of this scenario.
    let program = parent.join("copies");
    fs::create_dir(&program).map_err(io_error)?;
    let executable = program.join("StickyMD.exe");
    fs::copy(parent.join("StickyMD.exe"), &executable).map_err(io_error)?;
    seed_note(&program, "First portable copy original")?;
    let mut child = start_until_editor_ready(&executable)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::runtime::portable_isolation::run(repository, &program, &executable, &mut child, window)?;
    child.kill_and_wait()?;
    checks.record("portable-copies-note-config-and-exit-isolated", &[]);
    visibility::copies(repository, parent, checks)?;
    Ok(())
}

fn collapsed_wake(
    program: &Path,
    alias: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    checks: &mut Checks,
) -> Result<(), String> {
    let edge = crate::window_control::PrimaryDockEdge::Left;
    crate::window_control::move_to_primary_edge(window, edge)?;
    wait_for_config(program, "dock_edge = \"left\"")?;
    super::dock::wait_for_edge(window, edge, false)?;
    crate::window_control::press_escape(window)?;
    super::dock::wait_for_edge(window, edge, true)?;
    crate::window_control::park_cursor_outside_window(window)?;
    let note = program.join("note/note.md");
    let config = program.join("note/config.toml");
    let before = (file_stamp(&note)?, file_stamp(&config)?);
    let mut secondary = ChildGuard::start(&alias.join("StickyMD.exe"))?;
    secondary.wait_for_exit(super::super::super::exact_desktop::TIMEOUT)?;
    checks.record("collapsed-second-instance-exits", &[]);
    super::dock::wait_for_edge(window, edge, false)?;
    if crate::window_control::visible_window(process_id)? != window {
        return Err("collapsed-instance wake replaced the primary HWND".to_owned());
    }
    checks.record("collapsed-wakes-primary", &[]);
    // Do not activate or click the window here: activation must come from IPC.
    wait_until(
        "collapsed primary is activated by the second launch",
        || {
            let facts = crate::window_control::activation_facts(window)?;
            Ok(facts.foreground && facts.active && facts.focused)
        },
    )?;
    checks.record("collapsed-wake-activates-primary", &[]);
    if before != (file_stamp(&note)?, file_stamp(&config)?) {
        return Err("collapsed second launch modified durable note/config".to_owned());
    }
    checks.record("collapsed-keeps-note-and-config", &[]);
    assert_sole_stickymd_process(process_id)?;
    checks.record("sole-process-after-collapsed-wake", &[]);
    Ok(())
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
    modified: SystemTime,
}

fn file_stamp(path: &Path) -> Result<FileStamp, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    Ok(FileStamp {
        bytes: fs::read(path).map_err(io_error)?,
        modified: metadata.modified().map_err(io_error)?,
    })
}
