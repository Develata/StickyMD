//! Exact tray-menu and close/show/quit lifecycle qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{
    Checks, assert_sole_stickymd_process, io_error, seed_note, start_until_editor_ready, tray,
    wait_note, wait_until,
};

pub(super) fn g4_01(repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
    const RETAINED: &str = "tray retained 中文🙂\n";
    const LATEST: &str = "tray quit latest generation 中文🙂\n";

    seed_note(program, "tray initial\n")?;
    let mut child = start_until_editor_ready(&program.join("StickyMD.exe"))?;
    let window = crate::window_control::visible_window(child.id())?;
    assert_sole_stickymd_process(child.id())?;
    checks.record("sole-process", &[("stage", "startup")]);

    // The menu opens, from closed, on a right-click on the `NotifyItemIcon` the helper resolves
    // in Explorer's tray, so an opened menu proves the icon.
    let items = tray::menu_items(repository, child.id())?;
    checks.record("tray-icon-present", &[]);
    let expected = ["9690-85CF", "7F6E-9876", "9000-51FA"];
    if items != expected {
        return Err(format!(
            "tray menu items are {items:?}, expected only {expected:?} in order"
        ));
    }
    checks.record("tray-menu-items", &[]);
    super::tray_topmost::run(repository, program, child.id(), window, checks)?;

    replace_source(window, RETAINED)?;
    crate::window_control::request_close(window)?;
    wait_for_visibility(window, false)?;
    checks.record("close-hides-window", &[]);
    if !child.is_running()? {
        return Err("paper Close terminated the StickyMD process".to_owned());
    }
    checks.record("close-keeps-process", &[]);

    tray::show(repository, child.id())?;
    wait_for_visibility(window, true)?;
    checks.record("tray-show-restores-window", &[]);
    if source_projection(window)? != RETAINED {
        return Err("tray Show did not restore the same retained source text".to_owned());
    }
    checks.record("tray-show-retains-source", &[]);

    tray::hide(repository, child.id())?;
    wait_for_visibility(window, false)?;
    if !child.is_running()? {
        return Err("tray Hide terminated the process".to_owned());
    }
    checks.record("tray-hide-keeps-process", &[]);
    tray::show(repository, child.id())?;
    wait_for_visibility(window, true)?;
    if crate::window_control::visible_window(child.id())? != window
        || source_projection(window)? != RETAINED
    {
        return Err("tray Hide/Show did not retain the same HWND and text".to_owned());
    }
    checks.record("tray-hide-show-retains-window-and-source", &[]);

    crate::window_control::press_alt_f4(window)?;
    wait_for_visibility(window, false)?;
    if !child.is_running()? {
        return Err("physical Alt+F4 terminated the process".to_owned());
    }
    checks.record("alt-f4-hides-without-exit", &[]);
    tray::show(repository, child.id())?;
    wait_for_visibility(window, true)?;
    wait_note(program, |text| text == RETAINED)?;
    quit_save_failure(repository, program, child.id(), window, RETAINED, checks)?;

    replace_source(window, LATEST)?;
    assert_sole_stickymd_process(child.id())?;
    checks.record("sole-process", &[("stage", "pre-exit")]);
    tray::exit(repository, child.id())?;
    child.wait_for_exit(super::super::super::exact_desktop::TIMEOUT)?;
    checks.record("tray-quit-exits", &[]);
    let durable = fs::read_to_string(program.join("note/note.md")).map_err(io_error)?;
    if durable != LATEST {
        return Err("tray Quit exited without durably saving the latest generation".to_owned());
    }
    checks.record("tray-quit-saves-latest", &[]);
    Ok(())
}

fn quit_save_failure(
    repository: &Path,
    program: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    original: &str,
    checks: &mut Checks,
) -> Result<(), String> {
    const UNSAVED: &str = "read-only quit must retain this unsaved generation 中文🙂\n";
    const FAILURE_TITLE: &str = "StickyMD * — 退出已取消：保存失败";
    let note = program.join("note/note.md");
    super::super::super::fixture_readonly::with_readonly(&note, || {
        replace_source(window, UNSAVED)?;
        if fs::read_to_string(&note).map_err(io_error)? != original {
            return Err("read-only fixture changed before tray Quit".to_owned());
        }
        tray::exit(repository, process_id)?;
        wait_until("quit cancelled by canonical note save failure", || {
            Ok(crate::window_control::title(window)? == FAILURE_TITLE)
        })?;
        assert_sole_stickymd_process(process_id)?;
        if !crate::window_control::is_visible(window)? {
            return Err("failed tray Quit left the paper hidden".to_owned());
        }
        checks.record("tray-quit-save-failure-keeps-visible-process", &[]);
        checks.record("tray-quit-save-failure-title", &[]);
        if source_projection(window)? != UNSAVED
            || fs::read_to_string(&note).map_err(io_error)? != original
        {
            return Err("failed tray Quit lost unsaved text or changed durable note".to_owned());
        }
        checks.record("tray-quit-save-failure-retains-text", &[]);
        Ok(())
    })?;
    crate::window_control::press_save(window)?;
    wait_note(program, |text| text == UNSAVED)?;
    wait_until("retry clears quit-save-failure state", || {
        Ok(crate::window_control::title(window)? == "StickyMD")
    })?;
    checks.record("tray-quit-save-failure-retry", &[]);
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
