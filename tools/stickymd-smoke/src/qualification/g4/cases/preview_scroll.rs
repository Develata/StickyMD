//! Native Preview scrollbar capture, outside motion, release and focus loss.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    Checks, io_error, seed_note, start_until_editor_ready, wait_for_config, wait_until,
};
use crate::window_control::{self as window, ToolbarControl, qualification_input as input};

pub(super) fn run(repository: &Path, program: &Path, checks: &mut Checks) -> Result<(), String> {
    for mode in ["preview", "split"] {
        let instance = program.join(format!("scroll-{mode}"));
        fs::create_dir(&instance).map_err(io_error)?;
        let executable = instance.join("StickyMD.exe");
        fs::copy(program.join("StickyMD.exe"), &executable).map_err(io_error)?;
        let text: String = (0..2000)
            .map(|index| format!("ROW{index:04}  \n"))
            .collect();
        seed_note(&instance, &text)?;
        let mut child = start_until_editor_ready(&executable)?;
        let hwnd = window::visible_window(child.id())?;
        window::move_to_primary_floating(hwnd)?;
        window::click_toolbar_physically(hwnd, ToolbarControl::Opacity)?;
        let held = input::opacity_drag(hwnd, 100)?;
        held.release()?;
        wait_for_config(&instance, "opacity = 100")?;
        window::press_escape(hwnd)?;
        if mode == "preview" {
            window::press_preview_shortcut(hwnd)?;
        } else {
            window::press_split_shortcut(hwnd)?;
        }
        wait_for_config(&instance, &format!("view_mode = \"{mode}\""))?;
        let mut top = (0, 0, 0);
        wait_until("initial Preview thumb", || {
            if let Ok(thumb) = input::right_scroll_thumb(hwnd) {
                top = thumb;
                Ok(thumb.0 < 20)
            } else {
                Ok(false)
            }
        })?;
        let note = instance.join("note/note.md");
        let before = fs::metadata(&note)
            .and_then(|meta| meta.modified())
            .map_err(io_error)?;
        let held = input::right_scroll_drag(hwnd, false)?;
        wait_until("Preview native mouse capture", || {
            Ok(window::activation_facts(hwnd)?.captured)
        })?;
        // No posted mouse messages, focus requests or clicks during this gesture.
        input::scroll_pointer(hwnd, 1.2, true)?;
        let bottom = input::right_scroll_thumb(hwnd)?;
        if bottom.0 <= top.1 + 100 || bottom.2 - bottom.1 > bottom.1 - bottom.0 {
            return Err(format!(
                "{mode} outside drag did not move to the bottom: {top:?} -> {bottom:?}"
            ));
        }
        if !window::activation_facts(hwnd)?.captured {
            return Err(format!("{mode} lost native capture outside the window"));
        }
        checks.record("preview-scroll-native-capture", &[("mode", mode)]);
        held.release()?;
        wait_until("Preview capture released", || {
            Ok(!window::activation_facts(hwnd)?.captured)
        })?;
        input::scroll_pointer(hwnd, 0.0, false)?;
        if input::right_scroll_thumb(hwnd)? != bottom {
            return Err(format!("{mode} kept scrolling after native release"));
        }
        checks.record("preview-scroll-stops-on-release", &[("mode", mode)]);
        super::diagnostics::step(repository, &instance, child.id(), "preview-bottom-released")?;

        let held = input::right_scroll_drag(hwnd, true)?;
        input::scroll_pointer(hwnd, -0.2, true)?;
        let returned = input::right_scroll_thumb(hwnd)?;
        if returned != top {
            return Err(format!(
                "{mode} reverse outside drag did not reach the initial top: {top:?} -> {returned:?}"
            ));
        }
        input::scroll_pointer(hwnd, 1.2, true)?;
        if input::right_scroll_thumb(hwnd)? != bottom {
            return Err(format!("{mode} bottom endpoint is not repeatable"));
        }
        input::scroll_pointer(hwnd, 0.5, true)?;
        let middle = input::right_scroll_thumb(hwnd)?;
        if middle.0 <= top.1 || middle.1 >= bottom.0 {
            return Err(format!(
                "{mode} outside motion missed the middle: {middle:?}"
            ));
        }
        checks.record("preview-scroll-outside-both-ends", &[("mode", mode)]);
        window::focus_shell_desktop(hwnd)?;
        thread::sleep(Duration::from_millis(150));
        let blurred = input::right_scroll_thumb(hwnd)?;
        input::scroll_pointer(hwnd, 0.0, true)?;
        if input::right_scroll_thumb(hwnd)? != blurred {
            return Err(format!(
                "{mode} continued the held drag after native focus loss"
            ));
        }
        held.release()?;
        checks.record("preview-scroll-stops-on-blur", &[("mode", mode)]);
        if fs::read_to_string(&note).map_err(io_error)? != text
            || fs::metadata(&note)
                .and_then(|meta| meta.modified())
                .map_err(io_error)?
                != before
            || window::title(hwnd)?.starts_with("StickyMD *")
        {
            return Err(format!(
                "{mode} scroll changed note bytes, mtime or clean state"
            ));
        }
        checks.record("preview-scroll-keeps-note", &[("mode", mode)]);
        child.kill_and_wait()?;
    }
    Ok(())
}
