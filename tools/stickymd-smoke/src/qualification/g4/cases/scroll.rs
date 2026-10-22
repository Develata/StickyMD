//! Observe real Source scrollbar wiring by reading a fixed viewport line.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{
    Checks, io_error, seed_note, start_until_editor_ready, wait_until,
};
use crate::window_control::{self as window, WindowHandle, qualification_input as input};

pub(super) fn run(program: &Path, checks: &mut Checks) -> Result<(), String> {
    let instance = program.join("scroll");
    fs::create_dir(&instance).map_err(io_error)?;
    let executable = instance.join("StickyMD.exe");
    fs::copy(program.join("StickyMD.exe"), &executable).map_err(io_error)?;
    let text: String = (0..2000).map(|index| format!("ROW{index:04}\n")).collect();
    seed_note(&instance, &text)?;
    let mut child = start_until_editor_ready(&executable)?;
    let hwnd = window::visible_window(child.id())?;
    window::move_to_primary_floating(hwnd)?;
    window::focus_source_editor(hwnd)?;
    input::document_home(hwnd)?;
    let initial = selected_line(hwnd)?;
    if initial > 20 {
        return Err(format!(
            "scroll fixture did not start at the top: {initial}"
        ));
    }
    let before = fs::metadata(instance.join("note/note.md"))
        .and_then(|meta| meta.modified())
        .map_err(io_error)?;
    let held = input::source_scroll_drag(hwnd, false)?;
    input::scroll_pointer(hwnd, 1.2, true)?;
    held.release()?;
    // Copy without an editor click: the selected source bytes must survive the drag.
    window::clear_clipboard()?;
    window::press_copy(hwnd)?;
    wait_until("selection survives physical scroll", || {
        Ok(window::clipboard_text()?.as_deref() == Some(format!("ROW{initial:04}").as_str()))
    })?;
    let bottom = selected_line(hwnd)?;
    if bottom < 1900 {
        return Err(format!(
            "outside drag did not reach document bottom: {bottom}"
        ));
    }
    checks.record("scroll-drag-outside-finishes", &[]);
    input::scroll_pointer(hwnd, 0.0, false)?;
    if selected_line(hwnd)? != bottom {
        return Err("pointer movement after release continued scrolling".to_owned());
    }
    checks.record("scroll-drag-stops-on-release", &[]);

    let held = input::source_scroll_drag(hwnd, true)?;
    input::scroll_pointer(hwnd, 0.5, false)?;
    window::focus_shell_desktop(hwnd)?;
    // Move back to the top while the physical button is still owned and down.
    // If Focused(false) did not cancel the drag this would scroll to the start.
    input::scroll_pointer(hwnd, 0.0, false)?;
    held.release()?;
    window::request_window_foreground(hwnd)?;
    let middle = selected_line(hwnd)?;
    if !(500..1500).contains(&middle) {
        return Err(format!(
            "blur failed to retain the middle viewport: {middle}"
        ));
    }
    checks.record("scroll-drag-stops-on-blur", &[]);
    let note = instance.join("note/note.md");
    if fs::read_to_string(&note).map_err(io_error)? != text
        || fs::metadata(&note)
            .and_then(|meta| meta.modified())
            .map_err(io_error)?
            != before
        || window::title(hwnd)?.starts_with("StickyMD *")
    {
        return Err("scrolling changed canonical text, save state or durable mtime".to_owned());
    }
    checks.record("scroll-keeps-note-and-selection", &[]);
    child.kill_and_wait()
}

fn selected_line(hwnd: WindowHandle) -> Result<usize, String> {
    input::select_viewport_line(hwnd)?;
    window::clear_clipboard()?;
    window::press_copy(hwnd)?;
    let mut line = None;
    wait_until("fixed viewport line copy", || {
        line = window::clipboard_text()?.and_then(|text| text.strip_prefix("ROW")?.parse().ok());
        Ok(line.is_some())
    })?;
    line.ok_or_else(|| "no viewport line observed".to_owned())
}
