//! G5 shell-surface eligibility and focus-recovery probe.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tool-window-identity

use std::path::Path;

use super::super::super::exact_desktop::{CaseEvidence, seed_note, wait_note};
use super::support;

pub(super) fn run(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    seed_note(program, "G5 shell focus baseline\n")?;
    let (mut child, window) = support::start_ready(program)?;
    let facts = crate::window_control::style_facts(window)?;
    if !facts.tool_window || facts.app_window || facts.no_activate {
        return Err(format!(
            "StickyMD shell identity is not taskbar/Alt+Tab-ineligible: {facts:?}"
        ));
    }
    let mut artifacts = Vec::new();
    support::capture(
        repository,
        child.id(),
        "G5-01",
        "tool-window",
        &mut artifacts,
    )?;

    crate::window_control::focus_source_editor(window)?;
    crate::window_control::focus_shell_desktop(window)?;
    let away = crate::window_control::activation_facts(window)?;
    // GetGUIThreadInfo can retain the last active/focused HWND for the
    // StickyMD GUI thread after another thread owns the foreground window.
    // Foreground ownership is the cross-thread shell fact that controls real
    // keyboard delivery; requiring stale per-thread active/focus fields would
    // reject a valid Windows focus transition.
    if away.foreground {
        return Err(format!(
            "StickyMD retained shell foreground ownership: {away:?}"
        ));
    }
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_document_end(window)?;
    crate::window_control::set_clipboard_text("G5_FOCUS_RESTORED\n")?;
    crate::window_control::press_paste(window)?;
    wait_note(program, |text| text.ends_with("G5_FOCUS_RESTORED\n"))?;
    child.kill_and_wait()?;
    Ok(CaseEvidence { artifacts })
}
