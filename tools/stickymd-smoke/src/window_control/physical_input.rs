//! Physical mouse input for opt-in runtime qualification: cursor placement, cursor shape and
//! the left button, injected through `input_injection`.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::thread;
use std::time::Duration;

use super::NativePoint;
use crate::input_injection;

const CURSOR_MOVE_ATTEMPTS: usize = 3;
const CURSOR_MOVE_RETRY: Duration = Duration::from_millis(25);
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const CURSOR_SHOWING: u32 = 0x0001;
const IDC_IBEAM: usize = 32_513;
const IDC_SIZENWSE: usize = 32_642;
const IDC_HAND: usize = 32_649;

#[repr(C)]
#[derive(Default)]
struct NativeCursorInfo {
    size: u32,
    flags: u32,
    cursor: isize,
    screen_position: NativePoint,
}

#[derive(Clone, Copy)]
pub(super) enum PhysicalCursorKind {
    Text,
    Pointer,
    DragRegion,
    SouthEastResize,
}

impl PhysicalCursorKind {
    fn resource(self) -> usize {
        match self {
            Self::Text => IDC_IBEAM,
            Self::Pointer => IDC_HAND,
            Self::DragRegion => IDC_HAND,
            Self::SouthEastResize => IDC_SIZENWSE,
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Pointer => "pointer",
            Self::DragRegion => "drag-region",
            Self::SouthEastResize => "south-east-resize",
        }
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut NativePoint) -> i32;
    fn GetCursorInfo(info: *mut NativeCursorInfo) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn LoadCursorW(instance: isize, cursor_name: *const u16) -> isize;
}

/// Presses the left button; release it with `release` (the guard releases it on drop).
pub(super) fn press_left_button() -> Result<input_injection::LiveLeftButton, String> {
    input_injection::live()?.press_left()
}

/// A left-button release for input an interrupted earlier run may have left down.
pub(super) fn release_left_button() -> Result<(), String> {
    input_injection::live()?.release_left()
}

pub(super) fn scroll_wheel(notches: i32) -> Result<(), String> {
    if notches == 0 {
        return Ok(());
    }
    let delta = notches
        .checked_mul(120)
        .ok_or_else(|| format!("mouse-wheel notch count {notches} overflowed"))?;
    input_injection::live()?
        .wheel(delta)
        .map_err(|error| format!("cannot turn the physical mouse wheel: {error}"))
}

pub(super) fn current_cursor_handle() -> Result<isize, String> {
    let mut info = NativeCursorInfo {
        size: std::mem::size_of::<NativeCursorInfo>() as u32,
        ..NativeCursorInfo::default()
    };
    // SAFETY: `info` is correctly sized writable CURSORINFO storage. The API
    // copies borrowed cursor facts and retains no pointer or ownership.
    if unsafe { GetCursorInfo(&raw mut info) } == 0 {
        return Err(format!(
            "cannot inspect current Windows cursor: {}",
            std::io::Error::last_os_error()
        ));
    }
    if info.flags & CURSOR_SHOWING == 0 {
        return Err("Windows cursor is not visible during physical smoke input".to_owned());
    }
    Ok(info.cursor)
}

pub(super) fn cursor_matches(
    observed: isize,
    expected: PhysicalCursorKind,
) -> Result<bool, String> {
    // SAFETY: null instance plus MAKEINTRESOURCE-style cursor identifier asks
    // Windows for a shared system cursor. The returned handle is borrowed and
    // must not be destroyed by the smoke driver.
    let expected_handle = unsafe { LoadCursorW(0, expected.resource() as *const u16) };
    if expected_handle == 0 {
        return Err(format!(
            "cannot load expected {} system cursor: {}",
            expected.description(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(observed == expected_handle)
}

pub(super) fn move_physical_cursor(x: i32, y: i32, operation: &str) -> Result<(), String> {
    move_physical_cursor_with_tolerance(x, y, 1, operation)
}

pub(super) fn move_physical_cursor_with_tolerance(
    x: i32,
    y: i32,
    tolerance: u32,
    operation: &str,
) -> Result<(), String> {
    let mut observed = NativePoint::default();
    for attempt in 0..CURSOR_MOVE_ATTEMPTS {
        send_absolute_mouse_move(x, y)?;
        thread::sleep(Duration::from_millis(20));
        // SAFETY: `observed` is writable POINT storage and the API retains no
        // pointer after copying the current cursor coordinates.
        if unsafe { GetCursorPos(&raw mut observed) } != 0
            && observed.x.abs_diff(x) <= tolerance
            && observed.y.abs_diff(y) <= tolerance
        {
            return Ok(());
        }
        if attempt + 1 < CURSOR_MOVE_ATTEMPTS {
            thread::sleep(CURSOR_MOVE_RETRY);
        }
    }
    Err(format!(
        "cannot {operation} through physical mouse input: requested=({x},{y}) observed=({},{})",
        observed.x, observed.y
    ))
}

pub(super) fn current_cursor_position() -> Result<NativePoint, String> {
    let mut observed = NativePoint::default();
    // SAFETY: `observed` is writable POINT storage and GetCursorPos copies one
    // coordinate pair without retaining the pointer.
    if unsafe { GetCursorPos(&raw mut observed) } == 0 {
        return Err(format!(
            "cannot read current physical cursor position: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(observed)
}

fn send_absolute_mouse_move(x: i32, y: i32) -> Result<(), String> {
    // SAFETY: GetSystemMetrics consumes copied metric identifiers and returns
    // the current virtual-desktop bounds without retaining any resource.
    let (left, top, width, height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    if width <= 1 || height <= 1 {
        return Err(format!(
            "Windows reported invalid virtual desktop bounds: left={left} top={top} width={width} height={height}"
        ));
    }
    let dx = normalize_absolute_coordinate(x, left, width)?;
    let dy = normalize_absolute_coordinate(y, top, height)?;
    input_injection::live()?
        .move_absolute(dx, dy)
        .map_err(|error| format!("cannot move the physical cursor: {error}"))
}

fn normalize_absolute_coordinate(value: i32, origin: i32, extent: i32) -> Result<i32, String> {
    if extent <= 1 {
        return Err(format!(
            "virtual desktop extent must exceed one pixel, got {extent}"
        ));
    }
    let offset = i64::from(value) - i64::from(origin);
    let normalized = offset
        .saturating_mul(65_535)
        .checked_div(i64::from(extent - 1))
        .ok_or_else(|| "virtual desktop coordinate normalization failed".to_owned())?;
    i32::try_from(normalized.clamp(0, 65_535))
        .map_err(|_| "normalized mouse coordinate does not fit i32".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase14_absolute_mouse_mapping_supports_negative_virtual_desktop_origins() {
        assert_eq!(
            normalize_absolute_coordinate(-1920, -1920, 4480).expect("left edge"),
            0
        );
        assert_eq!(
            normalize_absolute_coordinate(2559, -1920, 4480).expect("right edge"),
            65_535
        );
        let primary_origin = normalize_absolute_coordinate(0, -1920, 4480).expect("primary origin");
        assert!(primary_origin > 0 && primary_origin < 65_535);
    }
}
