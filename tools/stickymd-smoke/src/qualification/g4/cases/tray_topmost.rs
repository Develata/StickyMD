//! Native tray/window Pin synchronization and the temporary sensor state.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use std::fs;
use std::path::Path;

use super::super::super::exact_desktop::{Checks, io_error, tray, wait_for_config, wait_until};
use crate::window_control::{self as window, ToolbarControl, WindowHandle};

pub(super) fn run(
    repository: &Path,
    program: &Path,
    process_id: u32,
    hwnd: WindowHandle,
    checks: &mut Checks,
) -> Result<(), String> {
    assert_pin(repository, program, process_id, hwnd, false)?;
    tray::toggle_topmost(repository, process_id)?;
    assert_pin(repository, program, process_id, hwnd, true)?;
    window::click_toolbar_physically(hwnd, ToolbarControl::Topmost)?;
    assert_pin(repository, program, process_id, hwnd, false)?;
    window::click_toolbar_physically(hwnd, ToolbarControl::Topmost)?;
    assert_pin(repository, program, process_id, hwnd, true)?;
    tray::toggle_topmost(repository, process_id)?;
    assert_pin(repository, program, process_id, hwnd, false)?;
    checks.record("tray-topmost-controls-sync", &[]);

    let edge = window::PrimaryDockEdge::Left;
    window::move_to_primary_edge(hwnd, edge)?;
    wait_for_config(program, "dock_edge = \"left\"")?;
    super::dock::wait_for_edge(hwnd, edge, false)?;
    let config = fs::read(program.join("note/config.toml")).map_err(io_error)?;
    window::press_escape(hwnd)?;
    super::dock::wait_for_edge(hwnd, edge, true)?;
    window::park_cursor_outside_window(hwnd)?;
    if tray::topmost_checked(repository, process_id)?
        || fs::read(program.join("note/config.toml")).map_err(io_error)? != config
    {
        return Err("temporary dock sensor state changed the user's Pin or config".to_owned());
    }
    // Revealing and focusing the original HWND must remove the temporary state.
    window::reveal_primary_sensor(hwnd, edge)?;
    super::dock::wait_for_edge(hwnd, edge, false)?;
    window::focus_source_editor(hwnd)?;
    assert_pin(repository, program, process_id, hwnd, false)?;
    checks.record("tray-topmost-sensor-keeps-config", &[]);
    window::move_to_primary_floating(hwnd)?;
    wait_for_config(program, "dock_edge = \"none\"")
}

fn assert_pin(
    repository: &Path,
    program: &Path,
    process_id: u32,
    hwnd: WindowHandle,
    expected: bool,
) -> Result<(), String> {
    wait_for_config(program, &format!("always_on_top = {expected}"))?;
    wait_until("Pin reaches the HWND", || {
        Ok(window::is_topmost(hwnd)? == expected)
    })?;
    if tray::topmost_checked(repository, process_id)? != expected {
        return Err(format!("tray Pin check state differs from {expected}"));
    }
    Ok(())
}
