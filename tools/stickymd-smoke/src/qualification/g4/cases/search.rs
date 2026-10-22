//! Physical Search shortcuts and their Source/Preview routing.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use super::super::super::exact_desktop::{
    Checks, io_error, seed_note, start_until_editor_ready, wait_for_config, wait_note,
};
use crate::window_control::{self as input, WindowHandle};

const ORIGINAL: &str = "alpha one\nbeta\nalpha two\n";
const FIRST: &str = "alphaX one\nbeta\nalpha two\n";
const BOTH: &str = "alphaX one\nbeta\nalphaX two\n";
const OBSERVATION: Duration = Duration::from_millis(800);

type Scenario = fn(&Path, WindowHandle, &mut Checks) -> Result<(), String>;

pub(super) fn run(template: &Path, checks: &mut Checks) -> Result<(), String> {
    // Fresh processes avoid retained query/replacement/cursor state between scenarios.
    let scenarios: &[(&str, Scenario)] = &[
        ("find", find_only),
        ("park", parked_search),
        ("replace", select_before_replace),
    ];
    for (name, scenario) in scenarios {
        let program = template.join(format!("search-{name}"));
        fs::create_dir(&program).map_err(io_error)?;
        fs::copy(template.join("StickyMD.exe"), program.join("StickyMD.exe")).map_err(io_error)?;
        seed_note(&program, ORIGINAL)?;
        let mut child = start_until_editor_ready(&program.join("StickyMD.exe"))?;
        let window = input::visible_window(child.id())?;
        input::focus_source_editor(window)?;
        input::press_document_end(window)?;
        scenario(&program, window, checks).map_err(|error| format!("Search {name}: {error}"))?;
        child.kill_and_wait()?;
    }
    Ok(())
}

fn paste(window: WindowHandle, text: &str) -> Result<(), String> {
    input::set_clipboard_text(text)?;
    input::press_paste(window)
}

fn open_find(window: WindowHandle) -> Result<(), String> {
    input::press_find(window)?;
    paste(window, "alpha")
}

fn open_replace(window: WindowHandle) -> Result<(), String> {
    open_find(window)?;
    input::press_replace(window)?;
    paste(window, "alphaX")
}

fn find_only(program: &Path, window: WindowHandle, checks: &mut Checks) -> Result<(), String> {
    open_find(window)?;
    input::press_control_enter(window)?;
    unchanged(program, window, ORIGINAL)?;
    input::press_control_shift_enter(window)?;
    unchanged(program, window, ORIGINAL)?;
    checks.record("search-find-only-guards", &[]);

    input::press_escape(window)?;
    // A positive edit proves the Search closed and the selected match reached Source.
    paste(window, "returned")?;
    wait_note(program, |text| text == "returned one\nbeta\nalpha two\n")?;
    input::press_undo(window)?;
    wait_note(program, |text| text == ORIGINAL)?;
    checks.record("search-escape-returns-to-source", &[]);
    Ok(())
}

fn parked_search(program: &Path, window: WindowHandle, checks: &mut Checks) -> Result<(), String> {
    open_replace(window)?;
    input::press_preview_shortcut(window)?;
    wait_for_config(program, "view_mode = \"preview\"")?;
    paste(window, "must-not-edit-preview")?;
    unchanged(program, window, ORIGINAL)?;

    // Ctrl+H must restore the existing query/replacement, not create an empty session.
    input::press_replace(window)?;
    wait_for_config(program, "view_mode = \"source\"")?;
    input::press_control_enter(window)?;
    wait_note(program, |text| text == FIRST)?;
    checks.record("search-preview-restores-on-replace", &[]);

    input::press_preview_shortcut(window)?;
    wait_for_config(program, "view_mode = \"preview\"")?;
    input::press_split_shortcut(window)?;
    wait_for_config(program, "view_mode = \"split\"")?;
    input::press_source_shortcut(window)?;
    wait_for_config(program, "view_mode = \"source\"")?;
    input::press_control_enter(window)?;
    wait_note(program, |text| text == BOTH)?;
    checks.record("search-view-shortcuts-retain-session", &[]);
    undo_replacements(program, window, checks)
}

fn select_before_replace(
    program: &Path,
    window: WindowHandle,
    checks: &mut Checks,
) -> Result<(), String> {
    open_replace(window)?;
    // The normal Source click collapses its selection while the Search remains open.
    input::focus_source_editor(window)?;
    input::press_control_enter(window)?;
    unchanged(program, window, ORIGINAL)?;
    checks.record("search-select-before-replace", &[]);
    input::press_control_enter(window)?;
    wait_note(program, |text| text == FIRST)?;
    input::press_control_enter(window)?;
    wait_note(program, |text| text == BOTH)?;
    checks.record("search-replacement-advances-past-insert", &[]);
    undo_replacements(program, window, checks)
}

fn undo_replacements(
    program: &Path,
    window: WindowHandle,
    checks: &mut Checks,
) -> Result<(), String> {
    input::press_find(window)?;
    input::press_undo(window)?;
    wait_note(program, |text| text == FIRST)?;
    input::press_undo(window)?;
    wait_note(program, |text| text == ORIGINAL)?;
    checks.record("search-replacement-one-undo", &[]);
    Ok(())
}

/// Observe for longer than the save debounce: a transient dirty edit also fails,
/// even if some later event restores the durable bytes before the final read.
fn unchanged(program: &Path, window: WindowHandle, expected: &str) -> Result<(), String> {
    let deadline = Instant::now() + OBSERVATION;
    loop {
        let text = fs::read_to_string(program.join("note/note.md")).map_err(io_error)?;
        let title = input::title(window)?;
        if text != expected || title != "StickyMD" {
            return Err(format!(
                "expected unchanged clean note; title={title:?}, note={text:?}"
            ));
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
}
