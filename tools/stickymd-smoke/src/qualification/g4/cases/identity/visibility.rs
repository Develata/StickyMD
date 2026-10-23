//! Native wake and per-directory tray visibility, independent of human visual judgement.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;

use super::{file_stamp, wait_for_visibility};
use crate::qualification::exact_desktop::{
    Checks, ChildGuard, TIMEOUT, assert_sole_stickymd_process, seed_note, start_until_editor_ready,
    tray, wait_note, wait_until,
};
use crate::window_control::{self as window, WindowHandle};

pub(super) fn wake(
    program: &Path,
    process_id: u32,
    hwnd: WindowHandle,
    checks: &mut Checks,
) -> Result<(), String> {
    let note = program.join("note/note.md");
    let config = program.join("note/config.toml");
    let original = fs::read_to_string(&note).map_err(|e| e.to_string())?;
    let monitors = window::manual_probe::monitors()?;
    for state in ["hidden", "minimized"] {
        if state == "hidden" {
            window::request_close(hwnd)?;
            wait_for_visibility(hwnd, false)?;
        } else {
            window::qualification_input::minimize(hwnd)?;
            wait_until("native minimized state before second launch", || {
                Ok(window::manual_probe::state(hwnd, &monitors)?.minimized)
            })?;
        }
        let before = (file_stamp(&note)?, file_stamp(&config)?);
        let mut secondary = ChildGuard::start(&program.join("StickyMD.exe"))?;
        secondary.wait_for_exit(TIMEOUT)?;
        checks.record("second-launch-exits", &[("state", state)]);
        // No click, activate or focus helper is allowed until IPC has restored focus.
        wait_until(
            "second launch restores original visible focused window",
            || {
                let observed = window::manual_probe::state(hwnd, &monitors)?;
                Ok(observed.visible && !observed.minimized && observed.activated)
            },
        )?;
        if window::visible_window(process_id)? != hwnd {
            return Err(format!("{state} wake replaced the original HWND"));
        }
        assert_sole_stickymd_process(process_id)?;
        checks.record("second-launch-restores-focused-window", &[("state", state)]);
        if before != (file_stamp(&note)?, file_stamp(&config)?) {
            return Err(format!(
                "{state} second launch changed note/config bytes or mtime"
            ));
        }
        checks.record("second-launch-keeps-durable-files", &[("state", state)]);
        let marker = format!("{state} wake accepts immediate input\n");
        window::set_clipboard_text(&marker)?;
        window::press_document_end(hwnd)?;
        window::press_paste(hwnd)?;
        wait_note(program, |text| text == format!("{original}{marker}"))?;
        checks.record("second-launch-immediate-input", &[("state", state)]);
        window::press_undo(hwnd)?;
        wait_note(program, |text| text == original)?;
    }
    Ok(())
}

pub(super) fn copies(repository: &Path, parent: &Path, checks: &mut Checks) -> Result<(), String> {
    let mut programs = Vec::new();
    let mut children = Vec::new();
    let mut windows = Vec::new();
    for name in ["visibility-first", "visibility-second"] {
        let program = parent.join(name);
        fs::create_dir(&program).map_err(|e| e.to_string())?;
        fs::copy(parent.join("StickyMD.exe"), program.join("StickyMD.exe"))
            .map_err(|e| e.to_string())?;
        seed_note(&program, name)?;
        let child = start_until_editor_ready(&program.join("StickyMD.exe"))?;
        windows.push(window::visible_window(child.id())?);
        children.push(child);
        programs.push(program);
    }
    let known = [children[0].id(), children[1].id()];
    let before = programs
        .iter()
        .map(|p| {
            Ok((
                file_stamp(&p.join("note/note.md"))?,
                file_stamp(&p.join("note/config.toml"))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    // Each menu is resolved by its native process owner, never icon order or caption alone.
    for (copy, visible) in [(0, false), (1, false), (0, true), (1, true)] {
        let peer = 1 - copy;
        let peer_visible = window::is_visible(windows[peer])?;
        tray::set_owned_visibility(repository, known[copy], &known, visible)?;
        wait_for_visibility(windows[copy], visible)?;
        if window::is_visible(windows[peer])? != peer_visible {
            return Err("tray visibility operation changed the other portable window".to_owned());
        }
        for (index, program) in programs.iter().enumerate() {
            if before[index]
                != (
                    file_stamp(&program.join("note/note.md"))?,
                    file_stamp(&program.join("note/config.toml"))?,
                )
            {
                return Err("tray visibility changed a portable copy's durable files".to_owned());
            }
        }
        checks.record(
            "portable-tray-visibility-isolated",
            &[
                ("copy", if copy == 0 { "first" } else { "second" }),
                ("visible", if visible { "true" } else { "false" }),
            ],
        );
    }
    for child in &mut children {
        child.kill_and_wait()?;
    }
    Ok(())
}
