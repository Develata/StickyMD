//! P08-A11 copied-directory note, config and graceful-exit isolation.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;
use std::process::Child;

use crate::window_control::{self as input, WindowHandle};

fn state(program: &Path) -> Result<[(Vec<u8>, std::time::SystemTime); 2], String> {
    Ok([
        super::file_state(&program.join("note/note.md"))?,
        super::file_state(&program.join("note/config.toml"))?,
    ])
}

fn replace(program: &Path, window: WindowHandle, text: &str) -> Result<(), String> {
    input::focus_source_editor(window)?;
    input::press_select_all(window)?;
    input::set_clipboard_text(text)?;
    input::press_paste(window)?;
    input::press_save(window)?;
    super::wait_for_note(&program.join("note/note.md"), |bytes| {
        bytes == text.as_bytes()
    })?;
    Ok(())
}

fn unchanged(program: &Path, before: &[(Vec<u8>, std::time::SystemTime); 2]) -> Result<(), String> {
    if &state(program)? != before {
        return Err(format!(
            "other portable copy changed bytes or mtime: {}",
            program.display()
        ));
    }
    Ok(())
}

pub(crate) fn run(
    repository: &Path,
    program: &Path,
    executable: &Path,
    primary: &mut Child,
    window: WindowHandle,
) -> Result<(), String> {
    input::switch_to_source(primary.id())?;
    super::wait_for_config_field(program, "view_mode = \"source\"")?;
    let first_before = state(program)?;
    let root = program
        .parent()
        .ok_or("portable instance has no fixture root")?;
    let second_program = root.join("phase8-independent");
    let second_executable = super::copy_executable(executable, &second_program)?;
    super::prepare_isolation_layout(&second_program)?;
    let mut second = super::start(&second_executable)?;
    super::wait_for_layout(&second_program)?;
    let second_window = input::visible_window(second.id())?;
    if second_window == window {
        return Err("independent copy reused the first HWND".to_owned());
    }
    super::ensure_alive(primary, "first portable copy")?;
    super::ensure_alive(&mut second, "second portable copy")?;
    if state(&second_program)?[0].0 == first_before[0].0 {
        return Err("portable copy notes were not independently seeded".to_owned());
    }

    replace(
        &second_program,
        second_window,
        "Second portable copy edited independently",
    )?;
    input::click_toolbar(second_window, input::ToolbarControl::Topmost)?;
    super::wait_for_config_field(&second_program, "always_on_top = true")?;
    unchanged(program, &first_before)?;
    let second_before = state(&second_program)?;

    // Temporarily hide the other copy through its own titlebar so it cannot cover
    // the first fixture's input target; hiding must preserve its durable state.
    input::request_close(second_window)?;
    super::wait_for_window_visibility(second_window, false)?;
    replace(program, window, "First portable copy edited independently")?;
    input::click_toolbar(window, input::ToolbarControl::Topmost)?;
    super::wait_for_config_field(program, "always_on_top = true")?;
    unchanged(&second_program, &second_before)?;
    let first_after = state(program)?;

    crate::qualification::exit_owned_copy(repository, second.id(), &[primary.id(), second.id()])?;
    let status = super::wait_for_exit(&mut second, super::EXIT_TIMEOUT)?;
    if !status.success() {
        return Err(format!("second copy tray exit failed: {status}"));
    }
    super::ensure_alive(primary, "first copy after second copy's tray exit")?;
    if input::visible_window(primary.id())? != window {
        return Err("first copy HWND changed after second copy exit".to_owned());
    }
    unchanged(program, &first_after)?;
    // A final real edit proves the surviving copy remains usable.
    replace(
        program,
        window,
        "First portable copy still usable after peer exit",
    )?;
    unchanged(&second_program, &second_before)?;
    input::click_toolbar(window, input::ToolbarControl::Topmost)?;
    super::wait_for_config_field(program, "always_on_top = false")?;
    let original = std::str::from_utf8(&first_before[0].0)
        .map_err(|error| format!("first portable fixture was not UTF-8: {error}"))?;
    replace(program, window, original)?;
    Ok(())
}
