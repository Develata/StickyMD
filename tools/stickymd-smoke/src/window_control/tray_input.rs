//! Input for the StickyMD tray menu: placing the cursor on its icon and right-clicking it, and
//! Esc to close its popup, each injected only after its target is verified (plan 11 physical
//! input injection).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::thread;
use std::time::Duration;

use super::{GetGUIThreadInfo, NativeGuiThreadInfo, NativePoint};
use crate::input_injection::{self, ESCAPE};

const GA_ROOT: u32 = 2;
const GUI_POPUPMENUMODE: u32 = 0x0000_0010;
/// Explorer's taskbar and its overflow flyout host the notification icons.
const TRAY_CLASSES: [&str; 2] = ["Shell_TrayWnd", "TopLevelWindowForOverflowXamlIsland"];
const PLACEMENT_ATTEMPTS: usize = 3;
const PLACEMENT_RETRY: Duration = Duration::from_millis(50);

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut NativePoint) -> i32;
    fn WindowFromPoint(point: NativePoint) -> isize;
    fn GetAncestor(window: isize, flags: u32) -> isize;
    fn GetWindowThreadProcessId(window: isize, process_id: *mut u32) -> u32;
    fn GetClassNameW(window: isize, name: *mut u16, capacity: i32) -> i32;
}

fn class_name(window: isize) -> String {
    let mut buffer = [0_u16; 128];
    // SAFETY: `buffer` is writable storage of the stated capacity; the API writes at most that
    // many UTF-16 units and returns how many, retaining no pointer.
    let length = unsafe { GetClassNameW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..usize::try_from(length).unwrap_or(0)])
}

fn root_at(x: i32, y: i32) -> isize {
    // SAFETY: the point is copied by value; the returned HWNDs are borrowed scalars.
    unsafe {
        let hit = WindowFromPoint(NativePoint { x, y });
        if hit == 0 {
            0
        } else {
            GetAncestor(hit, GA_ROOT)
        }
    }
}

fn cursor() -> Result<NativePoint, String> {
    let mut point = NativePoint::default();
    // SAFETY: `point` is writable POINT storage; the API copies the position and keeps no
    // pointer.
    if unsafe { GetCursorPos(&raw mut point) } == 0 {
        return Err(format!(
            "cannot read the cursor: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(point)
}

/// Places the cursor on the tray icon at `(x, y)`, retrying a bounded number of times until it
/// reads back there; `read_back` holds where it last read back, also when a later placement or
/// read fails, for the right-click to check and for diagnostics.
pub(crate) fn place_cursor_on_tray_icon(
    x: i32,
    y: i32,
    read_back: &mut Option<(i32, i32)>,
) -> Result<(), String> {
    let injection = input_injection::live()?;
    for attempt in 0..PLACEMENT_ATTEMPTS {
        injection.place_cursor(x, y)?;
        let point = cursor()?;
        *read_back = Some((point.x, point.y));
        if (point.x, point.y) == (x, y) {
            break;
        }
        if attempt + 1 < PLACEMENT_ATTEMPTS {
            thread::sleep(PLACEMENT_RETRY);
        }
    }
    Ok(())
}

/// Where the cursor is now, so a tray step can put it back after it fails.
pub(crate) fn tray_cursor_position() -> Result<(i32, i32), String> {
    cursor().map(|point| (point.x, point.y))
}

/// Puts the cursor back at `(x, y)` through the injection module and reads it back; an error
/// when it does not read back there.
pub(crate) fn restore_tray_cursor(x: i32, y: i32) -> Result<(), String> {
    let mut read_back = None;
    place_cursor_on_tray_icon(x, y, &mut read_back)?;
    match read_back {
        Some(point) if point == (x, y) => Ok(()),
        other => Err(format!(
            "the cursor was not restored to ({x},{y}); it reads {other:?}"
        )),
    }
}

/// Right-clicks the tray icon at `(x, y)`, only while the cursor reads back exactly there and
/// the window under it is Explorer's tray; both are read right before the click. The cursor can
/// still move between that check and the click.
pub(crate) fn right_click_tray_icon(x: i32, y: i32) -> Result<(), String> {
    let point = cursor()?;
    if (point.x, point.y) != (x, y) {
        return Err(format!(
            "refusing the tray right-click: the cursor reads ({},{}), not the icon at ({x},{y})",
            point.x, point.y
        ));
    }
    let class = class_name(root_at(x, y));
    if !TRAY_CLASSES.contains(&class.as_str()) {
        return Err(format!(
            "refusing the tray right-click: the window under ({x},{y}) is `{class}`, not the tray"
        ));
    }
    input_injection::live()?.click_right()
}

/// Presses Esc to close a tray popup menu, only while a popup menu is active on the foreground
/// thread and its owner window belongs to `process_id`. Returns whether it was pressed; not
/// pressing shows only that no StickyMD popup was active at the query, not that the menu is
/// closed. A query that fails, or a popup whose owner cannot be read, is an error.
pub(crate) fn escape_tray_popup(process_id: u32) -> Result<bool, String> {
    if active_tray_popup_owner()? != Some(process_id) {
        return Ok(false);
    }
    input_injection::live()?.tap(ESCAPE)?;
    Ok(true)
}

/// Observes the active native popup owner without injecting input. A caller may
/// close only a popup whose PID belongs to its explicitly owned fixture set.
pub(crate) fn active_tray_popup_owner() -> Result<Option<u32>, String> {
    let mut info = NativeGuiThreadInfo {
        size: std::mem::size_of::<NativeGuiThreadInfo>() as u32,
        ..NativeGuiThreadInfo::default()
    };
    // SAFETY: `info` is correctly sized writable GUITHREADINFO storage; thread 0 names the
    // foreground thread; the API copies borrowed facts and keeps no pointer.
    if unsafe { GetGUIThreadInfo(0, &raw mut info) } == 0 {
        // A failed query says nothing about the menu; it is not "no menu".
        return Err(format!(
            "cannot read the foreground thread's GUI state for a tray popup: {}",
            std::io::Error::last_os_error()
        ));
    }
    if info.flags & GUI_POPUPMENUMODE == 0 {
        return Ok(None);
    }
    let mut owner = 0_u32;
    let menu_owner = info.menu_owner as isize;
    // SAFETY: `owner` is writable storage for the process id; the HWND is borrowed.
    if menu_owner == 0 || unsafe { GetWindowThreadProcessId(menu_owner, &raw mut owner) } == 0 {
        return Err(format!(
            "a popup menu is active but its owner window ({menu_owner:#x}) cannot be read"
        ));
    }
    Ok(Some(owner))
}
