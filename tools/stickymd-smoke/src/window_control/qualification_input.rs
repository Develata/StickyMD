//! Physical controls used by the explicit G4 desktop cases.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use super::*;

pub(crate) fn shift_escape(window: WindowHandle) -> Result<(), String> {
    send_shift_chord(window, 0x1B, 0x01, false)
}

pub(crate) fn document_home(window: WindowHandle) -> Result<(), String> {
    send_control_chord(window, 0x24, 0x47, true)
}

pub(crate) fn repeated_escape(window: WindowHandle) -> Result<(), String> {
    require_keyboard_target(window)?;
    input_injection::live()?.repeat_key(input_injection::ESCAPE, 2)
}

pub(crate) fn repeated_enter(window: WindowHandle) -> Result<(), String> {
    require_keyboard_target(window)?;
    input_injection::live()?.repeat_key(input_injection::Key::new(0x0D, 0x1C), 4)
}

pub(crate) fn opacity_field(window: WindowHandle) -> Result<(), String> {
    let (x, scale) = opacity_popup_x(window)?;
    // The field spans y=47..77 DIP. Search's native IME candidate window can
    // cover its middle, including transparent margins. Click the visible upper
    // interior without dismissing the candidate or changing composition state;
    // begin_press still proves that the actual target belongs to StickyMD.
    let button = begin_press(
        window,
        x + 194.5 * scale,
        50.0 * scale,
        PhysicalCursorKind::Pointer,
    )?;
    button.release()?;
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

pub(crate) fn opacity_drag(
    window: WindowHandle,
    opacity: u8,
) -> Result<input_injection::LiveLeftButton, String> {
    if !(40..=100).contains(&opacity) {
        return Err("opacity outside 40..=100".to_owned());
    }
    let (x, scale) = opacity_popup_x(window)?;
    let ratio = f64::from(opacity - 40) / 60.0;
    begin_press(
        window,
        x + (12.0 + 150.0 * ratio).min(161.0) * scale,
        63.0 * scale,
        PhysicalCursorKind::Pointer,
    )
}

fn begin_press(
    window: WindowHandle,
    x: f64,
    y: f64,
    cursor: PhysicalCursorKind,
) -> Result<input_injection::LiveLeftButton, String> {
    let rect = window_rect(window)?;
    let mut route = prepare_physical_input_target(
        window,
        rect.x + i32::from(pixel_u16(x)?),
        rect.y + i32::from(pixel_u16(y)?),
        cursor,
    )?;
    route.restore()?;
    press_left_on(window)
}

pub(crate) fn right_scroll_drag(
    window: WindowHandle,
    bottom: bool,
) -> Result<input_injection::LiveLeftButton, String> {
    let rect = client_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let y = if bottom {
        f64::from(rect.height) - 20.0 * scale
    } else {
        54.0 * scale
    };
    begin_press(
        window,
        f64::from(rect.width) - 13.0 * scale,
        y,
        PhysicalCursorKind::Arrow,
    )
}

/// Move only: while a drag is owned, no synthetic pointer message assists it.
pub(crate) fn scroll_pointer(
    window: WindowHandle,
    fraction: f64,
    outside: bool,
) -> Result<(), String> {
    let rect = window_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let x = if outside {
        rect.x + rect.width as i32 + 20
    } else {
        rect.x + rect.width as i32 - (13.0 * scale).round() as i32
    };
    let y =
        rect.y + (54.0 * scale + fraction * (f64::from(rect.height) - 74.0 * scale)).round() as i32;
    move_physical_cursor(x, y, "move owned scrollbar gesture")?;
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

pub(crate) fn select_viewport_line(window: WindowHandle) -> Result<(), String> {
    let scale = f64::from(window_dpi(window)) / 96.0;
    let button = begin_press(
        window,
        24.0 * scale,
        100.0 * scale,
        PhysicalCursorKind::Text,
    )?;
    button.release()?;
    send_physical_key(window, 0x24, 0x47, true)?;
    send_shift_chord(window, 0x23, 0x4F, true)
}

/// Read only the visible scrollbar's center column; never captures other apps.
/// This observes an unfocused live drag without sending a click or focus request.
pub(crate) fn right_scroll_thumb(window: WindowHandle) -> Result<(usize, usize, usize), String> {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetPixel(dc: isize, x: i32, y: i32) -> u32;
    }
    let rect = window_rect(window)?;
    let scale = f64::from(window_dpi(window)) / 96.0;
    let x = rect.x + rect.width as i32 - (13.0 * scale).round() as i32;
    let first = (54.0 * scale).round() as i32;
    let last = rect.height as i32 - (8.0 * scale).round() as i32;
    // SAFETY: GetDC(NULL) borrows a screen DC, released below on every result path.
    let dc = unsafe { GetDC(0) };
    if dc == 0 {
        return Err("cannot read the visible scrollbar".to_owned());
    }
    let result = (|| {
        let mut colors = Vec::new();
        for y in first..last {
            let point = NativePoint { x, y: rect.y + y };
            // SAFETY: scalar handles and by-value point only. Check the pixel is
            // owned by this fixture before reading; no other window is captured.
            if unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) } != window.0 {
                return Err("scrollbar was covered during observation".to_owned());
            }
            // SAFETY: the borrowed DC is live; no pointer is passed or retained.
            let color = unsafe { GetPixel(dc, x, rect.y + y) };
            if color == u32::MAX {
                return Err("scrollbar pixel read failed".to_owned());
            }
            colors.push(color);
        }
        let mut counts = std::collections::BTreeMap::new();
        for color in &colors {
            *counts.entry(*color).or_insert(0_usize) += 1;
        }
        let background = counts
            .into_iter()
            .max_by_key(|(_, n)| *n)
            .map(|(color, _)| color)
            .ok_or("empty scrollbar column")?;
        let mut best = (0, 0);
        let mut start = None;
        for (index, color) in colors
            .iter()
            .chain(std::iter::once(&background))
            .enumerate()
        {
            if *color != background {
                start.get_or_insert(index);
            } else if let Some(begin) = start.take()
                && index - begin > best.1 - best.0
            {
                best = (begin, index);
            }
        }
        if best.1 - best.0 < (12.0 * scale) as usize || best.1 - best.0 > colors.len() / 3 {
            return Err(format!("no bounded scrollbar thumb in column: {best:?}"));
        }
        Ok((best.0, best.1, colors.len()))
    })();
    // SAFETY: exactly balances the GetDC(NULL) above, including error paths.
    unsafe {
        ReleaseDC(0, dc);
    }
    result
}
