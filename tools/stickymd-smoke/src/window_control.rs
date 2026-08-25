//! Minimal Windows window-message bridge for opt-in runtime smoke transitions.

use std::ffi::c_void;
use std::thread;
use std::time::Duration;

mod physical_input;

use physical_input::{
    PhysicalCursorKind, PhysicalLeftButtonGuard, current_cursor_handle, current_cursor_position,
    cursor_matches, move_physical_cursor, move_physical_cursor_with_tolerance, release_left_button,
};

const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const MK_LBUTTON: usize = 0x0001;
const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOPMOST: isize = 0x0000_0008;
const WS_EX_LAYERED: isize = 0x0008_0000;
const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
const WS_EX_APPWINDOW: isize = 0x0004_0000;
const WS_EX_NOACTIVATE: isize = 0x0800_0000;
const WS_EX_TRANSPARENT: isize = 0x0000_0020;
const LWA_ALPHA: u32 = 0x0000_0002;
const SPI_GETWORKAREA: u32 = 0x0030;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_SHOWWINDOW: u32 = 0x0040;
const GA_ROOT: u32 = 2;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;
const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
const CURSOR_MOVE_ATTEMPTS: usize = 3;
const CURSOR_MOVE_RETRY: Duration = Duration::from_millis(25);
const CF_UNICODETEXT: u32 = 13;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;

#[derive(Default)]
struct WindowSearch {
    process_id: u32,
    window: isize,
    fallback: isize,
    fallback_area: i64,
    require_visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowHandle(isize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolbarControl {
    Topmost,
    Theme,
    Opacity,
    Collapse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimaryDockEdge {
    Left,
    Right,
    Top,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LayeredAlpha {
    pub(crate) layered: bool,
    pub(crate) alpha: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowStyleFacts {
    pub(crate) tool_window: bool,
    pub(crate) app_window: bool,
    pub(crate) no_activate: bool,
    pub(crate) transparent: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowActivationFacts {
    pub(crate) foreground: bool,
    pub(crate) active: bool,
    pub(crate) focused: bool,
    pub(crate) captured: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CursorFacts {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) inside_window: bool,
}

#[repr(C)]
#[derive(Default)]
struct NativeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct NativePoint {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Default)]
struct NativeGuiThreadInfo {
    size: u32,
    flags: u32,
    active: *mut c_void,
    focus: *mut c_void,
    capture: *mut c_void,
    menu_owner: *mut c_void,
    move_size: *mut c_void,
    caret: *mut c_void,
    caret_rect: NativeRect,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
    fn GetWindowThreadProcessId(window: isize, process_id: *mut u32) -> u32;
    fn GetForegroundWindow() -> *mut c_void;
    fn GetShellWindow() -> isize;
    fn GetGUIThreadInfo(thread_id: u32, info: *mut NativeGuiThreadInfo) -> i32;
    fn IsWindowVisible(window: isize) -> i32;
    fn IsWindow(window: isize) -> i32;
    fn GetWindowTextLengthW(window: isize) -> i32;
    fn GetWindowTextW(window: isize, text: *mut u16, maximum: i32) -> i32;
    fn GetWindowRect(window: isize, rect: *mut NativeRect) -> i32;
    fn GetClientRect(window: isize, rect: *mut NativeRect) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
    fn GetWindowLongPtrW(window: isize, index: i32) -> isize;
    fn GetLayeredWindowAttributes(
        window: isize,
        color_key: *mut u32,
        alpha: *mut u8,
        flags: *mut u32,
    ) -> i32;
    fn SetWindowPos(
        window: isize,
        insert_after: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn SystemParametersInfoW(
        action: u32,
        parameter: u32,
        value: *mut NativeRect,
        flags: u32,
    ) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn SetForegroundWindow(window: isize) -> i32;
    fn GetCursorPos(point: *mut NativePoint) -> i32;
    fn ScreenToClient(window: isize, point: *mut NativePoint) -> i32;
    fn WindowFromPoint(point: NativePoint) -> isize;
    fn GetAncestor(window: isize, flags: u32) -> isize;
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    #[cfg(test)]
    fn GetThreadDpiAwarenessContext() -> isize;
    #[cfg(test)]
    fn AreDpiAwarenessContextsEqual(first: isize, second: isize) -> i32;
    fn OpenClipboard(owner: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(format: u32) -> isize;
    fn keybd_event(virtual_key: u8, scan_code: u8, flags: u32, extra_info: usize);
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalLock(memory: isize) -> *const c_void;
    fn GlobalUnlock(memory: isize) -> i32;
    fn GlobalSize(memory: isize) -> usize;
}

struct ClipboardGuard;

struct PhysicalInputRouteGuard {
    window: WindowHandle,
    restore_not_topmost: bool,
}

impl PhysicalInputRouteGuard {
    fn restore(&mut self) -> Result<(), String> {
        if !self.restore_not_topmost {
            return Ok(());
        }
        // SAFETY: this reverses the temporary HWND_TOPMOST projection made
        // solely by the local smoke driver. The HWND is borrowed and no
        // pointer or ownership obligation crosses the call.
        if unsafe {
            SetWindowPos(
                self.window.0,
                HWND_NOTOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } == 0
        {
            return Err(format!(
                "cannot restore StickyMD non-topmost state after physical drag: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.restore_not_topmost = false;
        Ok(())
    }
}

impl Drop for PhysicalInputRouteGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

impl ClipboardGuard {
    fn open() -> Result<Self, String> {
        const TIMEOUT: Duration = Duration::from_secs(1);
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            // SAFETY: a null owner is valid for this short-lived smoke query;
            // successful ownership is released by `Drop` on every path.
            if unsafe { OpenClipboard(0) } != 0 {
                return Ok(Self);
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "cannot open clipboard within {:.3} seconds: {}",
                    TIMEOUT.as_secs_f64(),
                    std::io::Error::last_os_error()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: this guard exists only after OpenClipboard succeeded and is
        // the sole close obligation for that acquisition.
        unsafe {
            CloseClipboard();
        }
    }
}

/// Keep native window queries and posted client coordinates in physical
/// pixels. Without this context Windows virtualizes cross-process coordinates
/// on scaled monitors, causing the smoke driver to apply DPI twice.
pub(crate) fn enable_per_monitor_v2_dpi_awareness() -> Result<(), String> {
    // SAFETY: the pseudo-handle is the documented constant for Per-Monitor V2.
    // The call changes only the current smoke CLI thread and retains no pointer.
    let previous =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if previous == 0 {
        return Err(format!(
            "cannot enable Per-Monitor V2 DPI awareness for runtime smoke: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Click StickyMD's Source toolbar control in the visible top-level window
/// owned by `process_id`. This bridge exists only in the development smoke
/// CLI; product runtime code does not expose a test command channel.
pub(crate) fn switch_to_source(process_id: u32) -> Result<(), String> {
    let window = visible_window(process_id)?;
    click_view_control(window, 0)?;
    // Let the application's message pump consume the ordered sequence before
    // the caller starts polling the durable config acknowledgement.
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub(crate) fn switch_to_preview(window: WindowHandle) -> Result<(), String> {
    click_view_control(window, 2)?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub(crate) fn switch_to_split(window: WindowHandle) -> Result<(), String> {
    click_view_control(window, 1)?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub(crate) fn focus_split_preview(window: WindowHandle) -> Result<(), String> {
    let client = client_rect(window)?;
    let x = ((client.width as u64 * 3) / 4).min(u64::from(u16::MAX)) as u16;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let y = pixel_u16(34.0 * scale + 32.0 * scale)?;
    click_client(window, x, y)?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

/// Click the semantic math-delimiter conversion action in the native toolbar.
pub(crate) fn click_math_conversion(window: WindowHandle) -> Result<(), String> {
    click_view_control(window, 3)?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub(crate) fn press_enter(window: WindowHandle) -> Result<(), String> {
    post_virtual_key(window, 0x0D, 0x1C)
}

pub(crate) fn press_f6(window: WindowHandle) -> Result<(), String> {
    post_virtual_key(window, 0x75, 0x40)
}

pub(crate) fn press_select_all(window: WindowHandle) -> Result<(), String> {
    send_control_chord(window, 0x41, 0x1E, false)
}

pub(crate) fn press_copy(window: WindowHandle) -> Result<(), String> {
    send_control_chord(window, 0x43, 0x2E, false)
}

pub(crate) fn press_document_end(window: WindowHandle) -> Result<(), String> {
    send_control_chord(window, 0x23, 0x4F, true)
}

pub(crate) fn clear_clipboard() -> Result<(), String> {
    let _guard = ClipboardGuard::open()?;
    // SAFETY: ClipboardGuard owns the open clipboard and EmptyClipboard
    // retains no caller-owned pointer.
    if unsafe { EmptyClipboard() } == 0 {
        return Err(format!(
            "cannot clear clipboard: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub(crate) fn clipboard_text() -> Result<Option<String>, String> {
    let _guard = ClipboardGuard::open()?;
    // SAFETY: the clipboard is open for this guard and the query only returns
    // whether the standard Unicode text format is currently available.
    if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } == 0 {
        return Ok(None);
    }
    // SAFETY: the clipboard is open and the returned HGLOBAL remains owned by
    // the clipboard for the duration of this guard.
    let memory = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if memory == 0 {
        return Err(format!(
            "cannot obtain Unicode clipboard text: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: GlobalSize only queries the clipboard-owned allocation and
    // retains no pointer.
    let bytes = unsafe { GlobalSize(memory) };
    if bytes < std::mem::size_of::<u16>() || bytes % std::mem::size_of::<u16>() != 0 {
        return Err(format!(
            "Unicode clipboard allocation has invalid byte length {bytes}"
        ));
    }
    // SAFETY: CF_UNICODETEXT is a NUL-terminated UTF-16 HGLOBAL. The pointer
    // remains valid while locked and ClipboardGuard keeps the clipboard open.
    let pointer = unsafe { GlobalLock(memory) }.cast::<u16>();
    if pointer.is_null() {
        return Err(format!(
            "cannot lock Unicode clipboard text: {}",
            std::io::Error::last_os_error()
        ));
    }
    let units = bytes / std::mem::size_of::<u16>();
    // SAFETY: GlobalSize bounds the locked allocation to `units` UTF-16 code
    // units; this read ends before GlobalUnlock below.
    let text = unsafe { std::slice::from_raw_parts(pointer, units) };
    let length = text.iter().position(|unit| *unit == 0).unwrap_or(units);
    let decoded = String::from_utf16(&text[..length])
        .map_err(|error| format!("clipboard text is invalid UTF-16: {error}"));
    // SAFETY: `memory` is exactly the HGLOBAL locked above; no borrowed slice
    // is used after this call.
    unsafe {
        GlobalUnlock(memory);
    }
    decoded.map(Some)
}

pub(crate) fn press_zoom_in(window: WindowHandle) -> Result<(), String> {
    post_control_chord(window, 0x6B, 0x4E)
}

pub(crate) fn press_zoom_out(window: WindowHandle) -> Result<(), String> {
    post_control_chord(window, 0x6D, 0x4A)
}

pub(crate) fn press_zoom_reset(window: WindowHandle) -> Result<(), String> {
    post_control_chord(window, 0x30, 0x0B)
}

pub(crate) fn title(window: WindowHandle) -> Result<String, String> {
    ensure_window(window)?;
    Ok(raw_window_title(window.0))
}

/// Locate the visible StickyMD top-level window owned by `process_id`.
pub(crate) fn visible_window(process_id: u32) -> Result<WindowHandle, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(window) = find_window(process_id, true)
            && window_area(window.0) >= 10_000
        {
            return Ok(window);
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!(
                "cannot find a visible StickyMD paper window for process {process_id}"
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// Request normal window close through the real native message path.
pub(crate) fn request_close(window: WindowHandle) -> Result<(), String> {
    post_message(window, WM_CLOSE, 0, 0, "close")
}

pub(crate) fn click_toolbar(window: WindowHandle, control: ToolbarControl) -> Result<(), String> {
    let client = client_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let (control_size, gap, edge) = toolbar_metrics(client.width, scale);
    let right_width = 5.0 * control_size + 4.0 * gap;
    let right_origin =
        (f64::from(client.width) - edge - right_width).max(edge + 4.0 * (control_size + gap) + gap);
    let offset = match control {
        ToolbarControl::Topmost => 0.0,
        ToolbarControl::Theme => 1.0,
        ToolbarControl::Opacity => 2.0,
        ToolbarControl::Collapse => 3.0,
    };
    let x = right_origin + offset * (control_size + gap) + control_size / 2.0;
    let y = 17.0 * scale;
    click_client(window, pixel_u16(x)?, pixel_u16(y)?)?;
    thread::sleep(Duration::from_millis(25));
    Ok(())
}

pub(crate) fn commit_opacity_slider(window: WindowHandle, opacity: u8) -> Result<(), String> {
    if !(40..=100).contains(&opacity) {
        return Err(format!("opacity {opacity} is outside 40..=100"));
    }
    let client = client_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let (control_size, gap, edge) = toolbar_metrics(client.width, scale);
    let right_width = 5.0 * control_size + 4.0 * gap;
    let right_origin =
        (f64::from(client.width) - edge - right_width).max(edge + 4.0 * (control_size + gap) + gap);
    let opacity_control_right = right_origin + 3.0 * (control_size + gap) - gap;
    let popup_width = (230.0 * scale).min(f64::from(client.width));
    let popup_x = (opacity_control_right - popup_width)
        .clamp(0.0, (f64::from(client.width) - popup_width).max(0.0));
    let slider_x = popup_x + 12.0 * scale;
    let slider_width = 150.0 * scale;
    let ratio = f64::from(opacity - 40) / 60.0;
    let x = (slider_x + slider_width * ratio).min(slider_x + slider_width - scale.max(1.0));
    let y = 34.0 * scale + 29.0 * scale;
    click_client(window, pixel_u16(x)?, pixel_u16(y)?)?;
    thread::sleep(Duration::from_millis(25));
    Ok(())
}

pub(crate) fn move_to_primary_left_edge(window: WindowHandle) -> Result<(), String> {
    move_to_primary_edge(window, PrimaryDockEdge::Left)
}

pub(crate) fn move_to_primary_edge(
    window: WindowHandle,
    edge: PrimaryDockEdge,
) -> Result<(), String> {
    let current = window_rect(window)?;
    let work = primary_work_area()?;
    let maximum_x = work
        .x
        .saturating_add(work.width.saturating_sub(current.width) as i32);
    let maximum_y = work
        .y
        .saturating_add(work.height.saturating_sub(current.height) as i32);
    let centered_x = work.x.saturating_add(
        i32::try_from(work.width.saturating_sub(current.width) / 2).unwrap_or(i32::MAX),
    );
    let centered_y = work.y.saturating_add(
        i32::try_from(work.height.saturating_sub(current.height) / 2).unwrap_or(i32::MAX),
    );
    let (x, y) = match edge {
        PrimaryDockEdge::Left => (work.x, centered_y.clamp(work.y, maximum_y)),
        PrimaryDockEdge::Right => (maximum_x, centered_y.clamp(work.y, maximum_y)),
        PrimaryDockEdge::Top => (centered_x.clamp(work.x, maximum_x), work.y),
    };
    move_window(window, x, y)
}

pub(crate) fn move_to_primary_corner(window: WindowHandle, right: bool) -> Result<(), String> {
    let current = window_rect(window)?;
    let work = primary_work_area()?;
    let x = if right {
        work.x
            .saturating_add(work.width.saturating_sub(current.width) as i32)
    } else {
        work.x
    };
    move_window(window, x, work.y)
}

pub(crate) fn move_to_primary_floating(window: WindowHandle) -> Result<(), String> {
    let current = window_rect(window)?;
    let work = primary_work_area()?;
    let x = work
        .x
        .saturating_add(work.width.saturating_sub(current.width) as i32 / 2);
    let y = work
        .y
        .saturating_add(work.height.saturating_sub(current.height) as i32 / 2);
    move_window(window, x, y)
}

pub(crate) fn focus_shell_desktop(window: WindowHandle) -> Result<(), String> {
    ensure_window(window)?;
    // SAFETY: GetShellWindow returns the borrowed desktop-shell HWND scalar;
    // no ownership is transferred and no pointer is retained.
    let shell = unsafe { GetShellWindow() };
    if shell == 0 {
        return Err("Windows did not expose a desktop shell window".to_owned());
    }
    // SAFETY: both HWND values are live borrowed scalars. This opt-in runtime
    // smoke merely asks Windows to move foreground focus to the desktop; it
    // does not send input or mutate any durable user state.
    if unsafe { SetForegroundWindow(shell) } == 0 {
        return Err(format!(
            "cannot focus the Windows desktop shell: {}",
            std::io::Error::last_os_error()
        ));
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let expected = window.0 as *mut c_void;
    while std::time::Instant::now() < deadline {
        // SAFETY: GetForegroundWindow returns a borrowed HWND scalar and
        // retains no caller-owned data.
        if unsafe { GetForegroundWindow() } != expected {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err("StickyMD retained foreground focus after focusing the desktop shell".to_owned())
}

pub(crate) fn focus_source_editor(window: WindowHandle) -> Result<(), String> {
    let (screen_x, screen_y) = content_activation_point(window)?;
    let mut input_route =
        prepare_physical_input_target(window, screen_x, screen_y, PhysicalCursorKind::Text)?;
    let click = PhysicalLeftButtonGuard::press()?;
    thread::sleep(Duration::from_millis(25));
    drop(click);
    wait_for_window_activation(window)?;
    input_route.restore()
}

fn wait_for_window_activation(window: WindowHandle) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let mut observed = activation_facts(window)?;
    while std::time::Instant::now() < deadline {
        observed = activation_facts(window)?;
        if observed.foreground && observed.active && observed.focused && !observed.captured {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "StickyMD source editor did not become an uncaptured foreground input target: {observed:?}"
    ))
}

fn content_activation_point(window: WindowHandle) -> Result<(i32, i32), String> {
    let rect = window_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let client_x = (24.0 * scale).round().clamp(1.0, f64::from(u16::MAX)) as i32;
    let client_y = (58.0 * scale).round().clamp(1.0, f64::from(u16::MAX)) as i32;
    Ok((
        rect.x.saturating_add(client_x),
        rect.y.saturating_add(client_y),
    ))
}

fn move_window(window: WindowHandle, x: i32, y: i32) -> Result<(), String> {
    let rect = window_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let drag_x = (rect.width / 2).min(i32::MAX as u32) as i32;
    let drag_y = (17.0 * scale).round().clamp(1.0, f64::from(i32::MAX)) as i32;
    let start_x = rect.x.saturating_add(drag_x);
    let start_y = rect.y.saturating_add(drag_y);
    let mut input_route =
        prepare_physical_input_target(window, start_x, start_y, PhysicalCursorKind::DragRegion)?;
    // WindowFromPoint proves only native routing. Give winit's event loop one
    // bounded turn to project the matching CursorMoved fact before the press;
    // the shell intentionally decides whether this point is a drag region
    // from its most recent cursor projection.
    thread::sleep(Duration::from_millis(75));
    let button = PhysicalLeftButtonGuard::press()?;
    let (engaged_cursor, engaged_rect) =
        engage_native_move_size(window, 8, 0, "start StickyMD window drag")?;
    let target_x = apply_coordinate_delta(engaged_cursor.x, x, engaged_rect.x, "drag x")?;
    let target_y = apply_coordinate_delta(engaged_cursor.y, y, engaged_rect.y, "drag y")?;
    move_physical_cursor_with_tolerance(
        target_x,
        target_y,
        32,
        "drag StickyMD to requested position",
    )?;
    thread::sleep(Duration::from_millis(25));
    drop(button);
    thread::sleep(Duration::from_millis(150));
    let completed_rect = window_rect(window)?;
    if completed_rect.x.abs_diff(x) > 24 || completed_rect.y.abs_diff(y) > 24 {
        return Err(format!(
            "physical StickyMD drag did not reach its requested outer position: requested=({x},{y}) engaged_cursor={engaged_cursor:?} engaged_rect={engaged_rect:?} completed_rect={completed_rect:?}"
        ));
    }
    input_route.restore()?;
    Ok(())
}

fn engage_native_move_size(
    window: WindowHandle,
    nudge_step_x: i32,
    nudge_step_y: i32,
    operation: &str,
) -> Result<(NativePoint, WindowRect), String> {
    let started = std::time::Instant::now();
    let deadline = started + Duration::from_secs(1);
    let mut next_nudge = started + Duration::from_millis(250);
    let expected = window.0 as *mut c_void;
    let initial_cursor = current_cursor_position()?;
    let initial_rect = window_rect(window)?;
    let mut observed_capture = 0_isize;
    let mut observed_move_size = 0_isize;
    let mut observed_flags = 0_u32;
    let mut observed_rect = initial_rect;
    let mut nudge_count = 0_i32;
    while std::time::Instant::now() < deadline {
        let info = native_gui_thread_info(window, operation)?;
        observed_capture = info.capture as isize;
        observed_move_size = info.move_size as isize;
        observed_flags = info.flags;
        observed_rect = window_rect(window)?;
        if info.move_size == expected || (info.capture == expected && observed_rect != initial_rect)
        {
            return Ok((current_cursor_position()?, observed_rect));
        }
        if nudge_count < 3 && std::time::Instant::now() >= next_nudge {
            // A generic HWND capture proves that the press reached winit, but
            // Windows may not enter the native move-size loop until the cursor
            // crosses its drag threshold. Advance in three bounded steps, then recompute the
            // remaining movement from the live geometry after hwndMoveSize is
            // established; the nudge can therefore never skew the final edge
            // or compact-size assertion.
            nudge_count += 1;
            move_physical_cursor_with_tolerance(
                initial_cursor
                    .x
                    .saturating_add(nudge_step_x.saturating_mul(nudge_count)),
                initial_cursor
                    .y
                    .saturating_add(nudge_step_y.saturating_mul(nudge_count)),
                16,
                operation,
            )?;
            next_nudge = std::time::Instant::now() + Duration::from_millis(75);
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "StickyMD did not enter native move-size after physical press: expected HWND={} capture HWND={observed_capture} move-size HWND={observed_move_size} gui_flags=0x{observed_flags:08x} nudges={nudge_count} initial_rect={initial_rect:?} observed_rect={observed_rect:?}",
        window.0,
    ))
}

fn apply_coordinate_delta(
    cursor: i32,
    desired: i32,
    observed: i32,
    operation: &str,
) -> Result<i32, String> {
    let value = i64::from(cursor) + i64::from(desired) - i64::from(observed);
    i32::try_from(value).map_err(|_| format!("{operation} target overflowed"))
}

fn native_gui_thread_info(
    window: WindowHandle,
    operation: &str,
) -> Result<NativeGuiThreadInfo, String> {
    let mut process_id = 0_u32;
    // SAFETY: the live HWND is borrowed and `process_id` is writable stack
    // storage. The API copies identifiers and retains no pointer.
    let thread_id = unsafe { GetWindowThreadProcessId(window.0, &raw mut process_id) };
    if thread_id == 0 {
        return Err(format!(
            "cannot read StickyMD GUI thread while {operation}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut info = NativeGuiThreadInfo {
        size: std::mem::size_of::<NativeGuiThreadInfo>() as u32,
        ..NativeGuiThreadInfo::default()
    };
    // SAFETY: `info` has the documented size and remains writable for the
    // synchronous query. The API retains no pointer.
    if unsafe { GetGUIThreadInfo(thread_id, &raw mut info) } == 0 {
        return Err(format!(
            "cannot inspect StickyMD GUI thread while {operation}: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(info)
}

fn prepare_physical_input_target(
    window: WindowHandle,
    x: i32,
    y: i32,
    expected_cursor: PhysicalCursorKind,
) -> Result<PhysicalInputRouteGuard, String> {
    ensure_window(window)?;
    // Recover from any interrupted prior smoke process before starting a new
    // physical route. A redundant button-up is harmless, while a stale
    // synthetic press can leave Windows in a move-size loop and warp every
    // subsequent cursor placement.
    release_left_button()?;
    let restore_not_topmost = !is_topmost(window)?;
    if restore_not_topmost {
        // SAFETY: HWND_TOPMOST is the documented Z-order sentinel. This
        // temporary smoke-only projection makes the physical input target
        // deterministic even while another normal app covers the paper.
        if unsafe {
            SetWindowPos(
                window.0,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        } == 0
        {
            return Err(format!(
                "cannot temporarily raise StickyMD for physical drag: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    let guard = PhysicalInputRouteGuard {
        window,
        restore_not_topmost,
    };
    activate_window_for_physical_input(window)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let mut observed = 0;
    let mut observed_cursor = 0_isize;
    let mut observed_activation = activation_facts(window)?;
    let mut last_cursor_error = None;
    while std::time::Instant::now() < deadline {
        // SAFETY: the live borrowed HWND and HWND_TOP sentinel contain no
        // caller-owned pointers. This changes only Z-order/activation of the
        // copied Release window used by the explicit local runtime smoke.
        if unsafe {
            SetWindowPos(
                window.0,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        } == 0
        {
            return Err(format!(
                "cannot raise StickyMD before physical drag: {}",
                std::io::Error::last_os_error()
            ));
        }
        // SAFETY: the scalar HWND is live. Windows may lawfully reject the
        // foreground request, so WindowFromPoint below remains the decisive
        // input-routing fact rather than this return value.
        unsafe {
            SetForegroundWindow(window.0);
        }
        // Generate a real coordinate transition before every physical press.
        // An unchanged absolute move can be coalesced without a winit
        // CursorMoved projection, leaving the product's hit test at an older
        // resize border. The inward waypoint keeps both drag-region and border
        // targets inside the paper before returning to the intended point.
        if let Err(error) = move_physical_cursor_with_tolerance(
            x.saturating_sub(8),
            y.saturating_sub(8),
            16,
            "prime StickyMD physical cursor projection",
        ) {
            last_cursor_error = Some(error);
            thread::sleep(Duration::from_millis(25));
            continue;
        }
        thread::sleep(Duration::from_millis(25));
        if let Err(error) = move_physical_cursor(x, y, "position StickyMD physical cursor") {
            last_cursor_error = Some(error);
            thread::sleep(Duration::from_millis(25));
            continue;
        }
        project_physical_cursor_to_window(window, x, y)?;
        thread::sleep(Duration::from_millis(25));
        // SAFETY: WindowFromPoint and GetAncestor copy/return borrowed HWND
        // scalars only; neither retains the by-value POINT or transfers a
        // window ownership obligation.
        observed = unsafe {
            let hit = WindowFromPoint(NativePoint { x, y });
            if hit == 0 {
                0
            } else {
                GetAncestor(hit, GA_ROOT)
            }
        };
        observed_cursor = current_cursor_handle()?;
        observed_activation = activation_facts(window)?;
        if observed == window.0
            && cursor_matches(observed_cursor, expected_cursor)?
            && observed_activation.foreground
            && observed_activation.active
            && observed_activation.focused
            && !observed_activation.captured
        {
            return Ok(guard);
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "physical input target is not ready: expected HWND={} observed root HWND={observed} expected_cursor={} observed_cursor=0x{observed_cursor:x} activation={observed_activation:?} last_cursor_error={}",
        window.0,
        expected_cursor.description(),
        last_cursor_error.as_deref().unwrap_or("none"),
    ))
}

fn activate_window_for_physical_input(window: WindowHandle) -> Result<(), String> {
    let current = activation_facts(window)?;
    if current.foreground && current.active && current.focused && !current.captured {
        return Ok(());
    }
    let (x, y) = content_activation_point(window)?;
    move_physical_cursor(x, y, "activate StickyMD before physical input")?;
    project_physical_cursor_to_window(window, x, y)?;
    thread::sleep(Duration::from_millis(25));
    let click = PhysicalLeftButtonGuard::press()?;
    thread::sleep(Duration::from_millis(25));
    drop(click);
    wait_for_window_activation(window)
}

fn project_physical_cursor_to_window(window: WindowHandle, x: i32, y: i32) -> Result<(), String> {
    let mut client = NativePoint { x, y };
    // SAFETY: `client` is writable stack storage and the live HWND is
    // borrowed. The API converts the copied point in place and retains no
    // pointer.
    if unsafe { ScreenToClient(window.0, &raw mut client) } == 0 {
        return Err(format!(
            "cannot convert physical drag point to StickyMD client coordinates: {}",
            std::io::Error::last_os_error()
        ));
    }
    let client_x = u16::try_from(client.x).map_err(|_| {
        format!(
            "physical drag client x is outside WM_MOUSEMOVE range: {}",
            client.x
        )
    })?;
    let client_y = u16::try_from(client.y).map_err(|_| {
        format!(
            "physical drag client y is outside WM_MOUSEMOVE range: {}",
            client.y
        )
    })?;
    send_mouse_move(window, mouse_lparam(client_x, client_y))
}

pub(crate) fn reveal_primary_sensor(
    window: WindowHandle,
    edge: PrimaryDockEdge,
) -> Result<(), String> {
    let rect = window_rect(window)?;
    let work = primary_work_area()?;
    let center_x = rect
        .x
        .saturating_add((rect.width / 2).min(i32::MAX as u32) as i32);
    let center_y = rect
        .y
        .saturating_add((rect.height / 2).min(i32::MAX as u32) as i32);
    let (outside_x, outside_y, sensor_x, sensor_y) = match edge {
        PrimaryDockEdge::Left => (
            work.x.saturating_add(work.width.saturating_sub(2) as i32),
            center_y,
            work.x.saturating_add(1),
            center_y,
        ),
        PrimaryDockEdge::Right => (
            work.x.saturating_add(1),
            center_y,
            work.x.saturating_add(work.width.saturating_sub(2) as i32),
            center_y,
        ),
        PrimaryDockEdge::Top => (
            center_x,
            work.y.saturating_add(work.height.saturating_sub(2) as i32),
            center_x,
            work.y.saturating_add(1),
        ),
    };
    set_cursor_position(
        outside_x,
        outside_y,
        "move cursor away from StickyMD sensor",
    )?;
    // The paper can animate away from a cursor that winit still marks as
    // inside until Windows delivers its tracked leave message. Mirror the
    // actual outside position to the HWND first so the following sensor move
    // always crosses a well-defined outside -> inside boundary.
    let outside_client_x = rect.width.saturating_add(16).min(u16::MAX as u32) as u16;
    let outside_client_y = (outside_y.saturating_sub(rect.y)).clamp(0, u16::MAX as i32) as u16;
    let outside_lparam =
        isize::try_from((u32::from(outside_client_y) << 16) | u32::from(outside_client_x))
            .map_err(|_| "outside mouse coordinates do not fit LPARAM".to_owned())?;
    send_mouse_move(window, outside_lparam)?;
    // Windows may coalesce rapid pointer moves. Give the winit event loop a
    // full scheduling slice to observe CursorLeft before returning to the
    // 3-DIP sensor; otherwise a long stress run can miss a synthetic enter.
    thread::sleep(Duration::from_millis(50));
    set_cursor_position(sensor_x, sensor_y, "hover StickyMD sensor")?;
    // A long stress loop can still have its final physical move coalesced by
    // Windows. Reinforce the real cursor position with the corresponding
    // client WM_MOUSEMOVE so winit's normal enter/track path observes every
    // requested cycle. This is smoke-only input; the product has no test IPC.
    let client_x = sensor_x.saturating_sub(rect.x).clamp(0, u16::MAX as i32) as u16;
    let client_y = sensor_y.saturating_sub(rect.y).clamp(0, u16::MAX as i32) as u16;
    let lparam = isize::try_from((u32::from(client_y) << 16) | u32::from(client_x))
        .map_err(|_| "sensor mouse coordinates do not fit LPARAM".to_owned())?;
    send_mouse_move(window, lparam)?;
    Ok(())
}

fn send_mouse_move(window: WindowHandle, lparam: isize) -> Result<(), String> {
    send_window_message(window, WM_MOUSEMOVE, 0, lparam)
}

fn send_window_message(
    window: WindowHandle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> Result<(), String> {
    ensure_window(window)?;
    // SAFETY: the target is the live paper HWND and the message carries only
    // copied integer values. SendMessageW returns after winit's native window
    // procedure has observed the transition; it retains no pointer.
    unsafe { SendMessageW(window.0, message, wparam, lparam) };
    Ok(())
}

pub(crate) fn park_cursor_at_primary_right(window: WindowHandle) -> Result<(), String> {
    let rect = window_rect(window)?;
    let work = primary_work_area()?;
    let x = work.x.saturating_add(work.width.saturating_sub(2) as i32);
    let y = rect
        .y
        .saturating_add((rect.height / 2).min(i32::MAX as u32) as i32);
    set_cursor_position(x, y, "park cursor outside StickyMD")
}

pub(crate) fn park_cursor_outside_window(window: WindowHandle) -> Result<(), String> {
    let rect = window_rect(window)?;
    let work = primary_work_area()?;
    let inset = 64_i32;
    let work_right = work
        .x
        .saturating_add(i32::try_from(work.width).unwrap_or(i32::MAX));
    let work_bottom = work
        .y
        .saturating_add(i32::try_from(work.height).unwrap_or(i32::MAX));
    let candidates = [
        (work.x.saturating_add(inset), work.y.saturating_add(inset)),
        (
            work_right.saturating_sub(inset),
            work.y.saturating_add(inset),
        ),
        (
            work.x.saturating_add(inset),
            work_bottom.saturating_sub(inset),
        ),
        (
            work_right.saturating_sub(inset),
            work_bottom.saturating_sub(inset),
        ),
    ];
    let rect_right = i64::from(rect.x) + i64::from(rect.width);
    let rect_bottom = i64::from(rect.y) + i64::from(rect.height);
    let point = candidates.into_iter().find(|(x, y)| {
        i64::from(*x) < i64::from(rect.x)
            || i64::from(*x) >= rect_right
            || i64::from(*y) < i64::from(rect.y)
            || i64::from(*y) >= rect_bottom
    });
    let Some((x, y)) = point else {
        return Err("cannot park the cursor inside the work area but outside StickyMD".to_owned());
    };
    set_cursor_position(x, y, "park cursor outside StickyMD")
}

fn set_cursor_position(x: i32, y: i32, operation: &str) -> Result<(), String> {
    let mut last_error = None;
    for attempt in 0..CURSOR_MOVE_ATTEMPTS {
        // SAFETY: SetCursorPos consumes copied screen coordinates and retains
        // no pointer. This opt-in runtime smoke drives the real desktop cursor
        // and never runs in CI.
        if unsafe { SetCursorPos(x, y) } != 0 {
            return Ok(());
        }
        last_error = Some(std::io::Error::last_os_error());
        let mut actual = NativePoint::default();
        // SAFETY: `actual` is writable for one POINT; GetCursorPos copies the
        // current desktop cursor location and retains no pointer.
        if unsafe { GetCursorPos(&raw mut actual) } != 0 && actual == (NativePoint { x, y }) {
            return Ok(());
        }
        if attempt + 1 < CURSOR_MOVE_ATTEMPTS {
            thread::sleep(CURSOR_MOVE_RETRY);
        }
    }
    Err(format!(
        "cannot {operation} after {CURSOR_MOVE_ATTEMPTS} attempts: {}",
        last_error.unwrap_or_else(std::io::Error::last_os_error)
    ))
}

pub(crate) fn window_rect(window: WindowHandle) -> Result<WindowRect, String> {
    native_rect(window, false)
}

pub(crate) fn resize_to_dip(
    window: WindowHandle,
    width_dip: u32,
    height_dip: u32,
) -> Result<(), String> {
    ensure_window(window)?;
    let rect = window_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let width = (f64::from(width_dip) * scale).round() as i32;
    let height = (f64::from(height_dip) * scale).round() as i32;
    let inset = scale.ceil().clamp(1.0, f64::from(i32::MAX)) as i32;
    let current_width = i32::try_from(rect.width).map_err(|_| {
        format!(
            "StickyMD width is outside physical resize range: {}",
            rect.width
        )
    })?;
    let current_height = i32::try_from(rect.height).map_err(|_| {
        format!(
            "StickyMD height is outside physical resize range: {}",
            rect.height
        )
    })?;
    let start_x = rect.x.saturating_add(current_width).saturating_sub(inset);
    let start_y = rect.y.saturating_add(current_height).saturating_sub(inset);
    let mut input_route = prepare_physical_input_target(
        window,
        start_x,
        start_y,
        PhysicalCursorKind::SouthEastResize,
    )?;
    // Route the same real pointer sequence as a USER resize. The product
    // intentionally commits durable placement only after its winit resize
    // intent observes the paired button release; synthetic WM_*SIZE messages
    // would resize the HWND without exercising that contract.
    thread::sleep(Duration::from_millis(75));
    let button = PhysicalLeftButtonGuard::press()?;
    let (engaged_cursor, engaged_rect) =
        engage_native_move_size(window, 8, 8, "start StickyMD compact resize")?;
    let engaged_width = i32::try_from(engaged_rect.width)
        .map_err(|_| "engaged StickyMD width does not fit i32".to_owned())?;
    let engaged_height = i32::try_from(engaged_rect.height)
        .map_err(|_| "engaged StickyMD height does not fit i32".to_owned())?;
    let target_x =
        apply_coordinate_delta(engaged_cursor.x, width, engaged_width, "compact resize x")?;
    let target_y =
        apply_coordinate_delta(engaged_cursor.y, height, engaged_height, "compact resize y")?;
    move_physical_cursor_with_tolerance(target_x, target_y, 32, "resize StickyMD compact window")?;
    thread::sleep(Duration::from_millis(25));
    drop(button);
    thread::sleep(Duration::from_millis(150));
    let completed_rect = window_rect(window)?;
    if completed_rect.width.abs_diff(width as u32) > 24
        || completed_rect.height.abs_diff(height as u32) > 24
    {
        return Err(format!(
            "physical StickyMD resize did not reach its requested extent: requested=({width},{height}) engaged_cursor={engaged_cursor:?} engaged_rect={engaged_rect:?} completed_rect={completed_rect:?}"
        ));
    }
    input_route.restore()?;
    Ok(())
}

pub(crate) fn style_facts(window: WindowHandle) -> Result<WindowStyleFacts, String> {
    ensure_window(window)?;
    // SAFETY: immutable pointer-free query of a live HWND's style bits.
    let style = unsafe { GetWindowLongPtrW(window.0, GWL_EXSTYLE) };
    Ok(WindowStyleFacts {
        tool_window: style & WS_EX_TOOLWINDOW != 0,
        app_window: style & WS_EX_APPWINDOW != 0,
        no_activate: style & WS_EX_NOACTIVATE != 0,
        transparent: style & WS_EX_TRANSPARENT != 0,
    })
}

pub(crate) fn activation_facts(window: WindowHandle) -> Result<WindowActivationFacts, String> {
    ensure_window(window)?;
    let mut process_id = 0_u32;
    // SAFETY: `process_id` is writable stack storage and `window` is a live
    // HWND. The API copies the owning thread/process identifiers.
    let thread_id = unsafe { GetWindowThreadProcessId(window.0, &raw mut process_id) };
    if thread_id == 0 {
        return Err(format!(
            "cannot read StickyMD GUI thread: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut info = NativeGuiThreadInfo {
        size: std::mem::size_of::<NativeGuiThreadInfo>() as u32,
        ..NativeGuiThreadInfo::default()
    };
    // SAFETY: `info` has the documented size and remains writable for the
    // synchronous query. The API retains no pointer.
    if unsafe { GetGUIThreadInfo(thread_id, &raw mut info) } == 0 {
        return Err(format!(
            "cannot read StickyMD GUI-thread state: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: GetForegroundWindow returns a borrowed HWND scalar and retains
    // no caller-owned data.
    let foreground = unsafe { GetForegroundWindow() };
    let expected = window.0 as *mut c_void;
    Ok(WindowActivationFacts {
        foreground: foreground == expected,
        active: info.active == expected,
        focused: info.focus == expected,
        captured: info.capture == expected,
    })
}

pub(crate) fn cursor_facts(window: WindowHandle) -> Result<CursorFacts, String> {
    let rect = window_rect(window)?;
    let mut point = NativePoint::default();
    // SAFETY: `point` is writable stack storage and the API copies the cursor
    // position without retaining a pointer.
    if unsafe { GetCursorPos(&raw mut point) } == 0 {
        return Err(format!(
            "cannot read desktop cursor position: {}",
            std::io::Error::last_os_error()
        ));
    }
    let right = i64::from(rect.x) + i64::from(rect.width);
    let bottom = i64::from(rect.y) + i64::from(rect.height);
    Ok(CursorFacts {
        x: point.x,
        y: point.y,
        inside_window: i64::from(point.x) >= i64::from(rect.x)
            && i64::from(point.x) < right
            && i64::from(point.y) >= i64::from(rect.y)
            && i64::from(point.y) < bottom,
    })
}

pub(crate) fn is_topmost(window: WindowHandle) -> Result<bool, String> {
    ensure_window(window)?;
    // SAFETY: this reads immutable style bits from the live HWND and retains
    // no pointer. A zero style is valid for this smoke assertion.
    Ok(unsafe { GetWindowLongPtrW(window.0, GWL_EXSTYLE) } & WS_EX_TOPMOST != 0)
}

pub(crate) fn layered_alpha(window: WindowHandle) -> Result<LayeredAlpha, String> {
    ensure_window(window)?;
    // SAFETY: see `is_topmost`; the style read is immutable and pointer-free.
    let layered = unsafe { GetWindowLongPtrW(window.0, GWL_EXSTYLE) } & WS_EX_LAYERED != 0;
    if !layered {
        return Ok(LayeredAlpha {
            layered: false,
            alpha: None,
        });
    }
    let mut color_key = 0_u32;
    let mut alpha = 0_u8;
    let mut flags = 0_u32;
    // SAFETY: all output pointers are valid stack storage and `window` is a
    // live HWND. The API only copies current layered attributes.
    if unsafe {
        GetLayeredWindowAttributes(window.0, &raw mut color_key, &raw mut alpha, &raw mut flags)
    } == 0
    {
        return Err(format!(
            "cannot read layered-window alpha: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(LayeredAlpha {
        layered: true,
        alpha: (flags & LWA_ALPHA != 0).then_some(alpha),
    })
}

/// Query whether the previously discovered top-level window still exists and
/// is visible. Keeping the same handle prevents a tray helper window from
/// being mistaken for the StickyMD paper after close-to-tray.
pub(crate) fn is_visible(window: WindowHandle) -> Result<bool, String> {
    // SAFETY: the integer handle came from EnumWindows. IsWindow and
    // IsWindowVisible only inspect it and retain no pointers.
    if unsafe { IsWindow(window.0) } == 0 {
        return Err("StickyMD top-level window was destroyed".to_owned());
    }
    // SAFETY: IsWindow above established that the handle still denotes a live
    // window at the point of this best-effort smoke observation.
    Ok(unsafe { IsWindowVisible(window.0) } != 0)
}

fn find_window(process_id: u32, require_visible: bool) -> Result<WindowHandle, String> {
    let mut search = WindowSearch {
        process_id,
        window: 0,
        fallback: 0,
        fallback_area: -1,
        require_visible,
    };
    // SAFETY: `search` remains alive and exclusively borrowed until EnumWindows
    // returns; the callback only writes that value during this synchronous call.
    unsafe {
        EnumWindows(
            Some(find_process_window),
            (&raw mut search).cast::<()>() as isize,
        );
    }
    if search.window == 0 {
        search.window = search.fallback;
    }
    if search.window == 0 {
        return Err(format!(
            "cannot find a StickyMD top-level window for process {process_id}"
        ));
    }
    Ok(WindowHandle(search.window))
}

fn ensure_window(window: WindowHandle) -> Result<(), String> {
    // SAFETY: the opaque integer came from EnumWindows; IsWindow only checks
    // whether it still identifies a live HWND.
    if unsafe { IsWindow(window.0) } == 0 {
        Err("StickyMD paper window was destroyed".to_owned())
    } else {
        Ok(())
    }
}

fn client_rect(window: WindowHandle) -> Result<WindowRect, String> {
    native_rect(window, true)
}

fn native_rect(window: WindowHandle, client: bool) -> Result<WindowRect, String> {
    ensure_window(window)?;
    let mut rect = NativeRect::default();
    // SAFETY: `rect` is writable stack storage, `window` is a live HWND, and
    // each selected API only copies geometry into the provided RECT.
    let succeeded = unsafe {
        if client {
            GetClientRect(window.0, &raw mut rect)
        } else {
            GetWindowRect(window.0, &raw mut rect)
        }
    };
    if succeeded == 0 {
        return Err(format!(
            "cannot read StickyMD {} rect: {}",
            if client { "client" } else { "window" },
            std::io::Error::last_os_error()
        ));
    }
    Ok(WindowRect {
        x: rect.left,
        y: rect.top,
        width: rect.right.saturating_sub(rect.left).max(0) as u32,
        height: rect.bottom.saturating_sub(rect.top).max(0) as u32,
    })
}

pub(crate) fn primary_work_area() -> Result<WindowRect, String> {
    let mut rect = NativeRect::default();
    // SAFETY: `rect` is valid writable storage for SPI_GETWORKAREA, which
    // copies the primary monitor work area and retains no pointer.
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &raw mut rect, 0) } == 0 {
        return Err(format!(
            "cannot read primary monitor work area: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(WindowRect {
        x: rect.left,
        y: rect.top,
        width: rect.right.saturating_sub(rect.left).max(0) as u32,
        height: rect.bottom.saturating_sub(rect.top).max(0) as u32,
    })
}

fn window_dpi(window: WindowHandle) -> u32 {
    // SAFETY: the HWND is live for this synchronous query; the API returns a
    // copied scalar and retains no resources.
    unsafe { GetDpiForWindow(window.0) }.max(96)
}

fn toolbar_metrics(client_width: u32, scale: f64) -> (f64, f64, f64) {
    let regular_required = 2.0 * 5.0 + 9.0 * 28.0 + 8.0 * 4.0;
    let compact = f64::from(client_width) / scale < regular_required;
    let edge = if compact { 6.0 } else { 5.0 } * scale;
    let gap = if compact { 0.0 } else { 4.0 } * scale;
    let available = (f64::from(client_width) - 2.0 * edge - 8.0 * gap).max(9.0 * scale);
    ((available / 9.0).min(28.0 * scale), gap, edge)
}

fn pixel_u16(value: f64) -> Result<u16, String> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u16::MAX) {
        return Err(format!("client coordinate {value} is outside u16 range"));
    }
    Ok(value.round() as u16)
}

fn click_client(window: WindowHandle, x: u16, y: u16) -> Result<(), String> {
    // (16, 10) physical pixels lies inside the first toolbar control for all
    // supported scale factors: the hit test derives the same 34-DIP toolbar
    // scale and maps the first 38-DIP slot to Source mode.
    let position = mouse_lparam(x, y);
    post_message(window, WM_MOUSEMOVE, 0, position, "mouse move")?;
    post_message(window, WM_LBUTTONDOWN, MK_LBUTTON, position, "mouse down")?;
    post_message(window, WM_LBUTTONUP, 0, position, "mouse up")?;
    Ok(())
}

fn click_view_control(window: WindowHandle, index: u8) -> Result<(), String> {
    if index > 3 {
        return Err(format!(
            "left toolbar control index {index} is outside 0..=3"
        ));
    }
    let scale = f64::from(window_dpi(window)) / 96.0;
    let x = (5.0 + f64::from(index) * 32.0 + 14.0) * scale;
    let y = 17.0 * scale;
    click_client(window, pixel_u16(x)?, pixel_u16(y)?)
}

fn post_virtual_key(
    window: WindowHandle,
    virtual_key: usize,
    scan_code: u16,
) -> Result<(), String> {
    let pressed = 1_isize | ((scan_code as isize) << 16);
    let released = pressed | (1_isize << 30) | (1_isize << 31);
    post_message(window, WM_KEYDOWN, virtual_key, pressed, "key down")?;
    post_message(window, WM_KEYUP, virtual_key, released, "key up")?;
    thread::sleep(Duration::from_millis(10));
    Ok(())
}

fn post_control_chord(
    window: WindowHandle,
    virtual_key: usize,
    scan_code: u16,
) -> Result<(), String> {
    const VK_CONTROL: usize = 0x11;
    const CONTROL_SCAN_CODE: u16 = 0x1D;
    let ctrl_pressed = 1_isize | ((CONTROL_SCAN_CODE as isize) << 16);
    let ctrl_released = ctrl_pressed | (1_isize << 30) | (1_isize << 31);
    let key_pressed = 1_isize | ((scan_code as isize) << 16);
    let key_released = key_pressed | (1_isize << 30) | (1_isize << 31);
    post_message(
        window,
        WM_KEYDOWN,
        VK_CONTROL,
        ctrl_pressed,
        "control key down",
    )?;
    post_message(
        window,
        WM_KEYDOWN,
        virtual_key,
        key_pressed,
        "zoom key down",
    )?;
    post_message(window, WM_KEYUP, virtual_key, key_released, "zoom key up")?;
    post_message(
        window,
        WM_KEYUP,
        VK_CONTROL,
        ctrl_released,
        "control key up",
    )?;
    thread::sleep(Duration::from_millis(10));
    Ok(())
}

fn send_control_chord(
    window: WindowHandle,
    virtual_key: u8,
    scan_code: u8,
    extended: bool,
) -> Result<(), String> {
    let activation = activation_facts(window)?;
    if !(activation.foreground && activation.active && activation.focused) {
        return Err(format!(
            "refusing keyboard injection without focused foreground StickyMD window: {activation:?}"
        ));
    }
    let key_flags = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    // SAFETY: keybd_event consumes copied key scalars. The target HWND is the
    // verified focused foreground window, and the balanced down/up sequence
    // leaves no modifier pressed after this synchronous smoke action.
    unsafe {
        keybd_event(0x11, 0x1D, 0, 0);
        keybd_event(virtual_key, scan_code, key_flags, 0);
        keybd_event(virtual_key, scan_code, key_flags | KEYEVENTF_KEYUP, 0);
        keybd_event(0x11, 0x1D, KEYEVENTF_KEYUP, 0);
    }
    thread::sleep(Duration::from_millis(10));
    Ok(())
}

unsafe extern "system" fn find_process_window(window: isize, parameter: isize) -> i32 {
    // SAFETY: EnumWindows invokes this callback synchronously with the pointer
    // to the live `WindowSearch` supplied by `switch_to_source`.
    let search = unsafe { &mut *(parameter as *mut WindowSearch) };
    let mut process_id = 0_u32;
    // SAFETY: `process_id` is a valid writable u32 and `window` is supplied by
    // EnumWindows for the duration of the callback.
    unsafe {
        GetWindowThreadProcessId(window, &raw mut process_id);
    }
    // SAFETY: `window` is the current EnumWindows handle and may be queried
    // during the callback.
    let visible = unsafe { IsWindowVisible(window) } != 0;
    if process_id == search.process_id && (!search.require_visible || visible) {
        if raw_window_title(window).starts_with("StickyMD") {
            search.window = window;
            0
        } else {
            let area = window_area(window);
            if area > search.fallback_area {
                search.fallback = window;
                search.fallback_area = area;
            }
            1
        }
    } else {
        1
    }
}

fn window_area(window: isize) -> i64 {
    let mut rect = NativeRect::default();
    // SAFETY: `rect` is a valid writable RECT and `window` is the current
    // EnumWindows handle. The API copies geometry and retains no pointer.
    if unsafe { GetWindowRect(window, &raw mut rect) } == 0 {
        return 0;
    }
    i64::from(rect.right.saturating_sub(rect.left).max(0))
        * i64::from(rect.bottom.saturating_sub(rect.top).max(0))
}

fn raw_window_title(window: isize) -> String {
    // SAFETY: `window` is a live EnumWindows handle for the duration of the
    // callback and the function only queries the copied title length.
    let length = unsafe { GetWindowTextLengthW(window) };
    if length <= 0 {
        return String::new();
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    // SAFETY: `buffer` is writable for `length + 1` UTF-16 units and remains
    // alive until the copied title is decoded below.
    let copied = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), length + 1) };
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
}

fn post_message(
    window: WindowHandle,
    message: u32,
    wparam: usize,
    lparam: isize,
    operation: &str,
) -> Result<(), String> {
    // SAFETY: `window` was returned by EnumWindows for the live child process;
    // PostMessageW copies these integer mouse-message values and retains no
    // pointers. Down/up are paired before the function returns.
    if unsafe { PostMessageW(window.0, message, wparam, lparam) } == 0 {
        Err(format!("cannot post {operation} message {message:#x}"))
    } else {
        Ok(())
    }
}

fn mouse_lparam(x: u16, y: u16) -> isize {
    let x = isize::try_from(x).unwrap_or_default();
    let y = isize::try_from(y).unwrap_or_default();
    x | (y << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_lparam_packs_client_coordinates() {
        let packed = mouse_lparam(16, 10);
        assert_eq!(packed & 0xffff, 16);
        assert_eq!((packed >> 16) & 0xffff, 10);
    }

    #[test]
    fn window_handle_is_an_opaque_copyable_value() {
        let handle = WindowHandle(42);
        assert_eq!(handle, handle);
    }

    #[test]
    fn phase10_compact_toolbar_driver_matches_the_minimum_hit_target_contract() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let (control, gap, edge) = toolbar_metrics((220.0 * scale) as u32, scale);
            assert!(control / scale >= 23.0);
            assert!(2.0 * edge + 9.0 * control + 8.0 * gap <= 220.0 * scale + 0.5);
        }
    }

    #[test]
    fn phase12_runtime_driver_uses_per_monitor_v2_coordinates() {
        enable_per_monitor_v2_dpi_awareness().expect("enable Per-Monitor V2 DPI awareness");
        // SAFETY: both APIs return or compare copied DPI-context pseudo-handles
        // for the current test thread and retain no resources.
        let current = unsafe { GetThreadDpiAwarenessContext() };
        assert_ne!(current, 0);
        assert_ne!(
            unsafe {
                AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            },
            0
        );
    }
}
