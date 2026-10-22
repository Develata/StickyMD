//! Physical controls used by the explicit G4 desktop cases.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use super::*;

pub(crate) fn shift_escape(window: WindowHandle) -> Result<(), String> {
    send_shift_chord(window, 0x1B, 0x01, false)
}

pub(crate) fn repeated_escape(window: WindowHandle) -> Result<(), String> {
    require_keyboard_target(window)?;
    input_injection::live()?.repeat_key(input_injection::ESCAPE, 2)
}

pub(crate) fn opacity_field(window: WindowHandle) -> Result<(), String> {
    let (x, scale) = opacity_popup_x(window)?;
    let button = begin_press(
        window,
        x + 194.5 * scale,
        62.0 * scale,
        PhysicalCursorKind::Text,
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
