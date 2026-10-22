//! Physical opacity commits and Esc rollback without canonical text mutation.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    Checks, io_error, wait_for_config, wait_note, wait_until,
};
use crate::window_control::{
    self as window, ToolbarControl, WindowHandle, qualification_input as input,
};

pub(super) fn run(program: &Path, hwnd: WindowHandle, checks: &mut Checks) -> Result<(), String> {
    window::focus_source_editor(hwnd)?;
    let original = fs::read(program.join("note/note.md")).map_err(io_error)?;
    open(hwnd)?;
    let held = input::opacity_drag(hwnd, 50)?;
    alpha(hwnd, 50)?;
    wait_for_config(program, "opacity = 96")?;
    held.release()?;
    committed(program, hwnd, 50)?;
    checks.record("opacity-slider-release-commits", &[]);

    input::opacity_field(hwnd)?;
    digits(hwnd, 82)?;
    window::press_enter(hwnd)?;
    committed(program, hwnd, 82)?;
    checks.record("opacity-numeric-enter-commits", &[]);
    window::press_escape(hwnd)?;

    open(hwnd)?;
    input::opacity_field(hwnd)?;
    digits(hwnd, 64)?;
    window::focus_shell_desktop(hwnd)?;
    committed(program, hwnd, 64)?;
    checks.record("opacity-numeric-blur-commits", &[]);
    window::focus_source_editor(hwnd)?;

    open(hwnd)?;
    input::opacity_field(hwnd)?;
    digits(hwnd, 73)?;
    window::press_escape(hwnd)?;
    committed(program, hwnd, 64)?;
    checks.record("opacity-escape-cancels-field", &[]);

    open(hwnd)?;
    let held = input::opacity_drag(hwnd, 90)?;
    alpha(hwnd, 90)?;
    wait_for_config(program, "opacity = 64")?;
    window::press_escape(hwnd)?;
    alpha(hwnd, 64)?;
    held.release()?;
    thread::sleep(Duration::from_millis(250));
    committed(program, hwnd, 64)?;
    checks.record("opacity-escape-cancels-drag", &[]);

    open(hwnd)?;
    let held = input::opacity_drag(hwnd, 96)?;
    held.release()?;
    committed(program, hwnd, 96)?;
    window::press_escape(hwnd)?;
    window::press_save(hwnd)?;
    wait_note(program, |text| text.as_bytes() == original)?;
    Ok(())
}

pub(super) fn open(hwnd: WindowHandle) -> Result<(), String> {
    window::click_toolbar_physically(hwnd, ToolbarControl::Opacity)
}

pub(super) fn digits(hwnd: WindowHandle, value: u8) -> Result<(), String> {
    for digit in value.to_string().bytes() {
        window::press_digit(hwnd, digit - b'0')?;
    }
    Ok(())
}

pub(super) fn committed(program: &Path, hwnd: WindowHandle, opacity: u8) -> Result<(), String> {
    wait_for_config(program, &format!("opacity = {opacity}"))?;
    alpha(hwnd, opacity)
}

fn alpha(hwnd: WindowHandle, opacity: u8) -> Result<(), String> {
    wait_until(&format!("native opacity {opacity}"), || {
        let observed = window::layered_alpha(hwnd)?;
        let expected = (f64::from(opacity) * 255.0 / 100.0).round() as u8;
        Ok(if opacity == 100 {
            !observed.layered && observed.alpha.is_none()
        } else {
            observed.layered && observed.alpha == Some(expected)
        })
    })
}
