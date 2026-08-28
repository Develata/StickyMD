//! Exact tray-menu and close/show/quit lifecycle qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{
    ChildGuard, assert_sole_stickymd_process, invoke_uia, io_error, seed_note, wait_for_layout,
    wait_until,
};

pub(super) fn g4_01(repository: &Path, program: &Path) -> Result<(), String> {
    const RETAINED: &str = "tray retained 中文🙂\n";
    const LATEST: &str = "tray quit latest generation 中文🙂\n";

    seed_note(program, "tray initial\n")?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;
    assert_sole_stickymd_process(child.id())?;

    let menu = invoke_uia(repository, "tray-menu", child.id(), None)?;
    let items = menu
        .lines()
        .filter_map(|line| line.strip_prefix("UIA_TRAY_ITEM_HEX="))
        .collect::<Vec<_>>();
    let expected = ["9690-85CF", "7F6E-9876", "9000-51FA"];
    if items != expected {
        return Err(format!(
            "tray menu items are {items:?}, expected only {expected:?} in order"
        ));
    }

    replace_source(window, RETAINED)?;
    crate::window_control::request_close(window)?;
    wait_for_visibility(window, false)?;
    if !child.is_running()? {
        return Err("paper Close terminated the StickyMD process".to_owned());
    }

    invoke_uia(repository, "tray-show", child.id(), None)?;
    wait_for_visibility(window, true)?;
    if source_projection(window)? != RETAINED {
        return Err("tray Show did not restore the same retained source text".to_owned());
    }

    replace_source(window, LATEST)?;
    assert_sole_stickymd_process(child.id())?;
    invoke_uia(repository, "tray-exit", child.id(), None)?;
    child.wait_for_exit(super::super::super::exact_desktop::TIMEOUT)?;
    let durable = fs::read_to_string(program.join("note/note.md")).map_err(io_error)?;
    if durable != LATEST {
        return Err("tray Quit exited without durably saving the latest generation".to_owned());
    }
    Ok(())
}

fn replace_source(window: crate::window_control::WindowHandle, text: &str) -> Result<(), String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::set_clipboard_text(text)?;
    crate::window_control::press_paste(window)?;
    wait_until("source projection replacement", || {
        Ok(source_projection(window).is_ok_and(|observed| observed == text))
    })
}

fn source_projection(window: crate::window_control::WindowHandle) -> Result<String, String> {
    crate::window_control::focus_source_editor(window)?;
    crate::window_control::clear_clipboard()?;
    crate::window_control::press_select_all(window)?;
    crate::window_control::press_copy(window)?;
    let text = crate::window_control::clipboard_text()?.unwrap_or_default();
    crate::window_control::press_document_end(window)?;
    Ok(super::editor::normalize_newlines(&text).into_owned())
}

fn wait_for_visibility(
    window: crate::window_control::WindowHandle,
    expected: bool,
) -> Result<(), String> {
    wait_until("StickyMD window visibility", || {
        Ok(crate::window_control::is_visible(window)? == expected)
    })
}
