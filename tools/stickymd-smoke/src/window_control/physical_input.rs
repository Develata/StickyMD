//! Physical Windows mouse injection for opt-in runtime qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::thread;
use std::time::Duration;

use super::NativePoint;

const CURSOR_MOVE_ATTEMPTS: usize = 3;
const CURSOR_MOVE_RETRY: Duration = Duration::from_millis(25);
const INPUT_MOUSE: u32 = 0;
const MOUSEEVENTF_MOVE: u32 = 0x0001;
const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;
const CURSOR_SHOWING: u32 = 0x0001;
const IDC_IBEAM: usize = 32_513;
const IDC_SIZENWSE: usize = 32_642;
const IDC_HAND: usize = 32_649;
const VK_LBUTTON: i32 = 0x01;

#[repr(C)]
#[derive(Clone, Copy)]
struct NativeMouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
union NativeInputData {
    mouse: NativeMouseInput,
}

#[repr(C)]
struct NativeInput {
    input_type: u32,
    data: NativeInputData,
}

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
    DragRegion,
    SouthEastResize,
}

impl PhysicalCursorKind {
    fn resource(self) -> usize {
        match self {
            Self::Text => IDC_IBEAM,
            Self::DragRegion => IDC_HAND,
            Self::SouthEastResize => IDC_SIZENWSE,
        }
    }

    pub(super) fn description(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::DragRegion => "drag-region",
            Self::SouthEastResize => "south-east-resize",
        }
    }
}

#[link(name = "user32")]
unsafe extern "system" {
    fn GetCursorPos(point: *mut NativePoint) -> i32;
    fn GetCursorInfo(info: *mut NativeCursorInfo) -> i32;
    fn GetAsyncKeyState(virtual_key: i32) -> i16;
    fn GetSystemMetrics(index: i32) -> i32;
    fn LoadCursorW(instance: isize, cursor_name: *const u16) -> isize;
    fn SendInput(count: u32, inputs: *const NativeInput, size: i32) -> u32;
}

pub(super) struct PhysicalLeftButtonGuard;

impl PhysicalLeftButtonGuard {
    pub(super) fn press() -> Result<Self, String> {
        send_mouse_input(MOUSEEVENTF_LEFTDOWN, 0, 0)?;
        wait_for_left_button_state(true)?;
        Ok(Self)
    }
}

impl Drop for PhysicalLeftButtonGuard {
    fn drop(&mut self) {
        let _ = send_mouse_input(MOUSEEVENTF_LEFTUP, 0, 0);
        let _ = wait_for_left_button_state(false);
    }
}

pub(super) fn release_left_button() -> Result<(), String> {
    send_mouse_input(MOUSEEVENTF_LEFTUP, 0, 0)?;
    wait_for_left_button_state(false)
}

fn wait_for_left_button_state(down: bool) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    let mut observed = false;
    while std::time::Instant::now() < deadline {
        // SAFETY: VK_LBUTTON is a copied virtual-key identifier. The query
        // returns process-independent input state and retains no resource.
        observed = unsafe { GetAsyncKeyState(VK_LBUTTON) } < 0;
        if observed == down {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    Err(format!(
        "Windows left-button state did not become {} after injected input; observed_down={observed}",
        if down { "pressed" } else { "released" }
    ))
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
    send_mouse_input(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        dx,
        dy,
    )
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

fn send_mouse_input(flags: u32, dx: i32, dy: i32) -> Result<(), String> {
    let input = NativeInput {
        input_type: INPUT_MOUSE,
        data: NativeInputData {
            mouse: NativeMouseInput {
                dx,
                dy,
                mouse_data: 0,
                flags,
                time: 0,
                extra_info: 0,
            },
        },
    };
    let size = i32::try_from(std::mem::size_of::<NativeInput>())
        .map_err(|_| "native INPUT size does not fit i32".to_owned())?;
    // SAFETY: `input` is a correctly sized, initialized INPUT_MOUSE value and
    // remains alive for the synchronous call. SendInput copies it and retains
    // no pointer or ownership.
    if unsafe { SendInput(1, &raw const input, size) } != 1 {
        return Err(format!(
            "cannot inject physical mouse input flags=0x{flags:04x}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
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
