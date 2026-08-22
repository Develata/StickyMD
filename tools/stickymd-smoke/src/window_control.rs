//! Minimal Windows window-message bridge for opt-in runtime smoke transitions.

use std::thread;
use std::time::Duration;

const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_CLOSE: u32 = 0x0010;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_ENTERSIZEMOVE: u32 = 0x0231;
const WM_EXITSIZEMOVE: u32 = 0x0232;
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
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

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

#[repr(C)]
#[derive(Default)]
struct NativeRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        parameter: isize,
    ) -> i32;
    fn GetWindowThreadProcessId(window: isize, process_id: *mut u32) -> u32;
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
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
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

pub(crate) fn press_zoom_in(window: WindowHandle) -> Result<(), String> {
    post_control_chord(window, 0x6B, 0x4E)
}

pub(crate) fn press_zoom_out(window: WindowHandle) -> Result<(), String> {
    post_control_chord(window, 0x6D, 0x4A)
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
    move_to_primary_inset(window, 0)
}

pub(crate) fn move_to_primary_inset(window: WindowHandle, inset_px: i32) -> Result<(), String> {
    let current = window_rect(window)?;
    let work = primary_work_area()?;
    // winit's message hook observes queued thread messages, so move-loop facts
    // must use PostMessageW. The delay gives the hook and its EventLoopProxy
    // hand-off a complete scheduling turn before the synthetic move.
    post_message(window, WM_ENTERSIZEMOVE, 0, 0, "enter move-size")?;
    thread::sleep(Duration::from_millis(100));
    // SAFETY: `window` is the live paper HWND. The call retains no pointers,
    // preserves z-order/activation, and keeps the existing dimensions.
    if unsafe {
        SetWindowPos(
            window.0,
            0,
            work.x.saturating_add(inset_px.max(0)),
            current
                .y
                .clamp(work.y, work.y.saturating_add(work.height as i32)),
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err(format!(
            "cannot move StickyMD to primary left edge: {}",
            std::io::Error::last_os_error()
        ));
    }
    post_message(window, WM_EXITSIZEMOVE, 0, 0, "exit move-size")?;
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

pub(crate) fn reveal_primary_left_sensor(window: WindowHandle) -> Result<(), String> {
    let rect = window_rect(window)?;
    let work = primary_work_area()?;
    let outside_x = work.x.saturating_add(work.width.saturating_sub(2) as i32);
    let sensor_x = work.x.saturating_add(1);
    let sensor_y = rect
        .y
        .saturating_add((rect.height / 2).min(i32::MAX as u32) as i32);
    // SAFETY: SetCursorPos consumes only copied screen coordinates and retains
    // no pointer. This opt-in runtime smoke intentionally drives the real
    // desktop cursor; it is never part of CI.
    if unsafe { SetCursorPos(outside_x, sensor_y) } == 0 {
        return Err(format!(
            "cannot move cursor away from StickyMD sensor: {}",
            std::io::Error::last_os_error()
        ));
    }
    // The paper can animate away from a cursor that winit still marks as
    // inside until Windows delivers its tracked leave message. Mirror the
    // actual outside position to the HWND first so the following sensor move
    // always crosses a well-defined outside -> inside boundary.
    let outside_client_x = rect.width.saturating_add(16).min(u16::MAX as u32) as u16;
    let outside_client_y = (sensor_y.saturating_sub(rect.y)).clamp(0, u16::MAX as i32) as u16;
    let outside_lparam =
        isize::try_from((u32::from(outside_client_y) << 16) | u32::from(outside_client_x))
            .map_err(|_| "outside mouse coordinates do not fit LPARAM".to_owned())?;
    send_mouse_move(window, outside_lparam)?;
    // Windows may coalesce rapid pointer moves. Give the winit event loop a
    // full scheduling slice to observe CursorLeft before returning to the
    // 3-DIP sensor; otherwise a long stress run can miss a synthetic enter.
    thread::sleep(Duration::from_millis(50));
    // SAFETY: same contract as above; the destination is the visible primary
    // left-edge sensor strip owned by the paper window.
    if unsafe { SetCursorPos(sensor_x, sensor_y) } == 0 {
        return Err(format!(
            "cannot hover StickyMD sensor: {}",
            std::io::Error::last_os_error()
        ));
    }
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
    // SAFETY: coordinates are copied by SetCursorPos. This is an explicit,
    // opt-in desktop runtime smoke and never executes in CI.
    if unsafe { SetCursorPos(x, y) } == 0 {
        Err(format!(
            "cannot park cursor outside StickyMD: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
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
    // SAFETY: coordinates are copied by SetCursorPos. The selected point is
    // inset from every work-area edge and outside the measured StickyMD rect,
    // avoiding both paper interaction and taskbar/edge-sensor activation.
    if unsafe { SetCursorPos(x, y) } == 0 {
        Err(format!(
            "cannot park cursor outside StickyMD: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
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
    let scale = f64::from(window_dpi(window)) / 96.0;
    let width = (f64::from(width_dip) * scale).round() as i32;
    let height = (f64::from(height_dip) * scale).round() as i32;
    post_message(window, WM_ENTERSIZEMOVE, 0, 0, "enter move-size")?;
    thread::sleep(Duration::from_millis(50));
    // SAFETY: `window` is live, dimensions are bounded smoke inputs, and the
    // call preserves position, z-order, and activation without retaining data.
    if unsafe {
        SetWindowPos(
            window.0,
            0,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err(format!(
            "cannot resize StickyMD compact window: {}",
            std::io::Error::last_os_error()
        ));
    }
    post_message(window, WM_EXITSIZEMOVE, 0, 0, "exit move-size")?;
    thread::sleep(Duration::from_millis(100));
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
}
