//! Exact primary-monitor docking geometry and timing qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;
use std::thread;
use std::time::Duration;

use super::super::super::exact_desktop::{
    ChildGuard, run_cargo_test, seed_note, wait_for_config, wait_for_layout, wait_until,
};

pub(super) fn g4_02(repository: &Path, program: &Path) -> Result<(), String> {
    // Virtual-time reducer and pure geometry are the authority for exact
    // 24/25 DIP, 100/500/700 ms, IME/focus, and Pin invariants. The copied
    // candidate below proves those transitions reach the real HWND.
    run_cargo_test(repository, "flow::window")?;

    seed_note(program, "G4 docking integration\n")?;
    let mut child = ChildGuard::start(&program.join("StickyMD.exe"))?;
    wait_for_layout(program)?;
    let window = crate::window_control::visible_window(child.id())?;

    for (edge, value) in [
        (crate::window_control::PrimaryDockEdge::Left, "left"),
        (crate::window_control::PrimaryDockEdge::Top, "top"),
        (crate::window_control::PrimaryDockEdge::Right, "right"),
    ] {
        dock_timing_cycle(program, window, edge, value)?;
    }

    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Topmost)?;
    wait_for_config(program, "always_on_top = true")?;
    if !crate::window_control::is_topmost(window)? {
        return Err("Pin ON did not reach the native HWND".to_owned());
    }
    dock_timing_cycle(
        program,
        window,
        crate::window_control::PrimaryDockEdge::Right,
        "right",
    )?;
    if !crate::window_control::is_topmost(window)? {
        return Err("Pin ON was lost during right-edge auto-hide".to_owned());
    }
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Topmost)?;
    wait_for_config(program, "always_on_top = false")?;

    verify_snap_boundaries(program, window)?;
    child.kill_and_wait()
}

fn dock_timing_cycle(
    program: &Path,
    window: crate::window_control::WindowHandle,
    edge: crate::window_control::PrimaryDockEdge,
    config_value: &str,
) -> Result<(), String> {
    crate::window_control::move_to_primary_edge(window, edge)?;
    wait_for_config(program, &format!("dock_edge = \"{config_value}\""))?;
    wait_for_edge(window, edge, false)?;

    thread::sleep(Duration::from_millis(850));
    assert_edge(window, edge, false, "focused dock guard")?;

    crate::window_control::focus_shell_desktop(window)?;
    thread::sleep(Duration::from_millis(600));
    assert_edge(window, edge, false, "pre-700ms focus-loss boundary")?;
    wait_for_edge(window, edge, true)?;

    crate::window_control::reveal_primary_sensor(window, edge)?;
    thread::sleep(Duration::from_millis(60));
    assert_edge(window, edge, true, "pre-100ms sensor boundary")?;
    wait_for_edge(window, edge, false)?;

    crate::window_control::park_cursor_outside_window(window)?;
    thread::sleep(Duration::from_millis(400));
    assert_edge(window, edge, false, "pre-500ms hover-leave boundary")?;
    wait_for_edge(window, edge, true)?;
    crate::window_control::reveal_primary_sensor(window, edge)?;
    wait_for_edge(window, edge, false)
}

fn verify_snap_boundaries(
    program: &Path,
    window: crate::window_control::WindowHandle,
) -> Result<(), String> {
    let work = crate::window_control::primary_work_area()?;
    let rect = crate::window_control::window_rect(window)?;
    let center_x = i32::try_from(work.width.saturating_sub(rect.width) / 2).unwrap_or(i32::MAX);
    let center_y = i32::try_from(work.height.saturating_sub(rect.height) / 2).unwrap_or(i32::MAX);
    let snap = crate::window_control::dip_pixels(window, 24.0)?;
    let outside = crate::window_control::dip_pixels(window, 25.0)?;
    let near = crate::window_control::dip_pixels(window, 10.0)?;
    let farther = crate::window_control::dip_pixels(window, 20.0)?;

    move_floating(program, window)?;
    crate::window_control::move_outer_to_primary_offset(window, snap, center_y)?;
    wait_for_config(program, "dock_edge = \"left\"")?;

    move_floating(program, window)?;
    crate::window_control::move_outer_to_primary_offset(window, outside, center_y)?;
    wait_for_config(program, "dock_edge = \"none\"")?;

    let bottom_y = i32::try_from(work.height.saturating_sub(rect.height)).unwrap_or(i32::MAX);
    crate::window_control::move_outer_to_primary_offset(window, center_x, bottom_y)?;
    wait_for_config(program, "dock_edge = \"none\"")?;

    crate::window_control::move_outer_to_primary_offset(window, farther, near)?;
    wait_for_config(program, "dock_edge = \"top\"")?;
    move_floating(program, window)?;
    crate::window_control::move_outer_to_primary_offset(window, near, farther)?;
    wait_for_config(program, "dock_edge = \"left\"")?;

    move_floating(program, window)?;
    crate::window_control::move_outer_to_primary_offset(window, near, near)?;
    wait_for_config(program, "dock_edge = \"top\"")?;
    move_floating(program, window)?;
    let right_tie = i32::try_from(work.width.saturating_sub(rect.width))
        .unwrap_or(i32::MAX)
        .saturating_sub(near);
    crate::window_control::move_outer_to_primary_offset(window, right_tie, near)?;
    wait_for_config(program, "dock_edge = \"top\"")?;
    move_floating(program, window)
}

fn move_floating(
    program: &Path,
    window: crate::window_control::WindowHandle,
) -> Result<(), String> {
    crate::window_control::move_to_primary_floating(window)?;
    wait_for_config(program, "dock_edge = \"none\"")
}

fn wait_for_edge(
    window: crate::window_control::WindowHandle,
    edge: crate::window_control::PrimaryDockEdge,
    collapsed: bool,
) -> Result<(), String> {
    wait_until("stable primary-edge geometry", || {
        match edge_state(window, edge) {
            // The animated transition intentionally has no stable classification.
            Ok(observed) => Ok(observed == collapsed),
            Err(error) if error == "dock geometry is still animating" => Ok(false),
            Err(error) => Err(error),
        }
    })
}

fn assert_edge(
    window: crate::window_control::WindowHandle,
    edge: crate::window_control::PrimaryDockEdge,
    collapsed: bool,
    stage: &str,
) -> Result<(), String> {
    let observed = edge_state(window, edge)?;
    if observed == collapsed {
        Ok(())
    } else {
        Err(format!(
            "{stage} observed collapsed={observed}, expected {collapsed}"
        ))
    }
}

fn edge_state(
    window: crate::window_control::WindowHandle,
    edge: crate::window_control::PrimaryDockEdge,
) -> Result<bool, String> {
    let first = crate::window_control::window_rect(window)?;
    thread::sleep(Duration::from_millis(20));
    let rect = crate::window_control::window_rect(window)?;
    if rect != first {
        return Err("dock geometry is still animating".to_owned());
    }
    let work = crate::window_control::primary_work_area()?;
    let rect_right = i64::from(rect.x) + i64::from(rect.width);
    let rect_bottom = i64::from(rect.y) + i64::from(rect.height);
    let work_right = i64::from(work.x) + i64::from(work.width);
    let collapsed = match edge {
        crate::window_control::PrimaryDockEdge::Left => {
            rect.x < work.x && (1..=16).contains(&(rect_right - i64::from(work.x)))
        }
        crate::window_control::PrimaryDockEdge::Right => {
            rect_right > work_right && (1..=16).contains(&(work_right - i64::from(rect.x)))
        }
        crate::window_control::PrimaryDockEdge::Top => {
            rect.y < work.y && (1..=16).contains(&(rect_bottom - i64::from(work.y)))
        }
    };
    if collapsed {
        return Ok(true);
    }
    let expanded = match edge {
        crate::window_control::PrimaryDockEdge::Left => (rect.x - work.x).abs() <= 1,
        crate::window_control::PrimaryDockEdge::Right => (rect_right - work_right).abs() <= 1,
        crate::window_control::PrimaryDockEdge::Top => (rect.y - work.y).abs() <= 1,
    };
    if expanded {
        Ok(false)
    } else {
        Err(format!(
            "window is neither expanded nor collapsed on {edge:?}: rect={rect:?} work={work:?}"
        ))
    }
}
