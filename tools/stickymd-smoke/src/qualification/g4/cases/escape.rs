//! Esc priority through real input, including a held key's second key-down.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{Checks, wait_for_config, wait_note, wait_until};
use crate::window_control::{
    self as window, PrimaryDockEdge, ToolbarControl, WindowHandle, qualification_input as input,
};

pub(super) fn run(program: &Path, hwnd: WindowHandle, checks: &mut Checks) -> Result<(), String> {
    window::focus_source_editor(hwnd)?;
    let floating = window::window_rect(hwnd)?;
    window::press_escape(hwnd)?;
    thread::sleep(Duration::from_millis(250));
    if window::window_rect(hwnd)? != floating || !window::is_visible(hwnd)? {
        return Err("ordinary Esc changed a floating window".to_owned());
    }
    checks.record("escape-floating-keeps-window", &[]);
    let edge = PrimaryDockEdge::Left;
    window::move_to_primary_edge(hwnd, edge)?;
    wait_for_config(program, "dock_edge = \"left\"")?;
    super::dock::wait_for_edge(hwnd, edge, false)?;
    window::focus_source_editor(hwnd)?;
    window::press_document_end(hwnd)?;
    window::press_find(hwnd)?;
    window::set_clipboard_text("absent-search-query")?;
    window::press_paste(hwnd)?;
    window::press_escape(hwnd)?;
    // The window is expanded already: hold it there so a wrong collapse shows.
    super::hold("Search consumes its Esc", || {
        super::dock::assert_edge(hwnd, edge, false, "Search consumes its Esc")
    })?;
    window::set_clipboard_text("ESC-RETURNED-TO-SOURCE")?;
    window::press_paste(hwnd)?;
    wait_note(program, |text| text.ends_with("ESC-RETURNED-TO-SOURCE"))?;
    window::press_undo(hwnd)?;
    wait_note(program, |text| text == "G4 docking integration\n")?;
    checks.record("escape-search-closes-before-dock", &[]);

    window::switch_to_preview(hwnd)?;
    wait_for_config(program, "view_mode = \"preview\"")?;
    window::focus_split_preview(hwnd)?;
    // Config publication does not acknowledge the asynchronous Preview layout.
    // Retry the read-only selection until the actual frame can be copied.
    wait_until("Preview selection positive copy", || {
        window::clear_clipboard()?;
        window::press_select_all(hwnd)?;
        window::press_copy(hwnd)?;
        Ok(window::clipboard_text()?.is_some_and(|text| text.contains("G4 docking integration")))
    })?;
    window::press_escape(hwnd)?;
    super::hold("Preview selection consumes Esc", || {
        super::dock::assert_edge(hwnd, edge, false, "Preview selection consumes Esc")
    })?;
    window::set_clipboard_text("NO-SELECTION-COPY")?;
    window::press_copy(hwnd)?;
    thread::sleep(Duration::from_millis(250));
    if window::clipboard_text()?.as_deref() != Some("NO-SELECTION-COPY") {
        return Err("Preview Esc did not clear its selection".to_owned());
    }
    checks.record("escape-preview-clears-selection", &[]);
    window::press_escape(hwnd)?;
    super::dock::wait_for_edge(hwnd, edge, true)
        .map_err(|error| format!("empty Preview Esc: {error}"))?;
    checks.record("escape-preview-empty-collapses", &[]);
    reveal(hwnd, edge)?;
    window::click_toolbar_physically(hwnd, ToolbarControl::Collapse)?;
    super::dock::wait_for_edge(hwnd, edge, true)
        .map_err(|error| format!("manual Collapse button: {error}"))?;
    checks.record("manual-collapse-button", &[]);
    // A dedicated adapter owns the covering window; this path adds no mouse messages.
    window::covered_sensor::hover(hwnd, || super::dock::wait_for_edge(hwnd, edge, false))
        .map_err(|error| format!("covered sensor: {error}"))?;
    checks.record("covered-sensor-real-hover", &[]);
    window::focus_window(hwnd)?;
    input::shift_escape(hwnd)?;
    super::dock::wait_for_edge(hwnd, edge, true).map_err(|error| format!("Shift+Esc: {error}"))?;
    checks.record("escape-shift-collapses", &[]);
    reveal(hwnd, edge)?;
    window::press_find(hwnd)?; // Preview switches to Source and opens Search.
    wait_for_config(program, "view_mode = \"source\"")?;
    input::repeated_escape(hwnd)?;
    super::dock::wait_for_edge(hwnd, edge, true)
        .map_err(|error| format!("repeated Esc: {error}"))?;
    checks.record("escape-repeat-keydowns-independent", &[]);
    reveal(hwnd, edge)?;
    window::move_to_primary_floating(hwnd)?;
    wait_for_config(program, "dock_edge = \"none\"")
}

fn reveal(hwnd: WindowHandle, edge: PrimaryDockEdge) -> Result<(), String> {
    window::reveal_primary_sensor(hwnd, edge)?;
    super::dock::wait_for_edge(hwnd, edge, false)?;
    window::focus_window(hwnd)?;
    Ok(())
}
