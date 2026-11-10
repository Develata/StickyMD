//! CoreGraphics window facts and optional normalized PPM screenshot, never product state.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::super::model::{Rect, Window};
use super::cf::{Cf, Owned};
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> Cf;
    fn CGRectMakeWithDictionaryRepresentation(dict: Cf, rect: *mut Rect) -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> Rect;
    fn CGWindowListCreateImage(bounds: Rect, options: u32, id: u32, image_options: u32) -> Cf;
    fn CGImageGetWidth(image: Cf) -> usize;
    fn CGImageGetHeight(image: Cf) -> usize;
    fn CGColorSpaceCreateDeviceRGB() -> Cf;
    fn CGBitmapContextCreate(
        data: *mut u8,
        width: usize,
        height: usize,
        bits: usize,
        stride: usize,
        space: Cf,
        info: u32,
    ) -> Cf;
    fn CGContextDrawImage(context: Cf, rect: Rect, image: Cf);
}
pub(in crate::mac_gui) fn screen() -> Rect {
    // SAFETY: value-returning getters for the main display, no borrowed pointers escape.
    unsafe { CGDisplayBounds(CGMainDisplayID()) }
}
pub(in crate::mac_gui) fn window(pid: u32, id: Option<u32>) -> Result<Window, String> {
    // SAFETY: all windows excluding desktop, owned Create-rule array; no image capture.
    let array = Owned::take(unsafe { CGWindowListCopyWindowInfo(16, 0) })?;
    let mut windows = Vec::new();
    for dict in array.array()? {
        if dict.get("kCGWindowOwnerPID")?.number()? != f64::from(pid) {
            continue;
        }
        let number = dict.get("kCGWindowNumber")?.number()? as u32;
        if id.is_some_and(|id| id != number) {
            continue;
        }
        let b = dict.get("kCGWindowBounds")?;
        let mut bounds = Rect::default();
        // SAFETY: SDK output storage matches CGRect; b is a live bounds dictionary.
        if !unsafe { CGRectMakeWithDictionaryRepresentation(b.0, &mut bounds) } {
            continue;
        }
        if bounds.size.width < 220.0 || bounds.size.height < 120.0 {
            continue;
        }
        windows.push(Window {
            id: number,
            bounds,
            alpha: dict.get("kCGWindowAlpha")?.number()?,
            layer: dict.get("kCGWindowLayer")?.number()? as i32,
            onscreen: dict
                .get("kCGWindowIsOnscreen")
                .and_then(|v| v.boolean())
                .unwrap_or(false),
        });
    }
    if windows.len() != 1 {
        return Err(format!(
            "expected one PID {pid} content window (id={id:?}), found {}",
            windows.len()
        ));
    }
    Ok(windows.remove(0))
}
pub(in crate::mac_gui) fn screenshot(w: &Window) -> Result<Vec<u8>, String> {
    // SAFETY: caller has preflighted capture permission. IncludingWindow=8 restricts capture
    // to this PID-owned window ID. Create-rule image/context/space are each released once.
    unsafe {
        let image = Owned::take(CGWindowListCreateImage(w.bounds, 8, w.id, 1))?;
        let width = CGImageGetWidth(image.0);
        let height = CGImageGetHeight(image.0);
        let len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n > 0 && *n <= 128 * 1024 * 1024)
            .ok_or("invalid screenshot dimensions")?;
        let mut rgba = vec![0_u8; len];
        let space = Owned::take(CGColorSpaceCreateDeviceRGB())?;
        let context = Owned::take(CGBitmapContextCreate(
            rgba.as_mut_ptr(),
            width,
            height,
            8,
            width * 4,
            space.0,
            0x4001,
        ))?;
        let mut bounds = Rect::default();
        bounds.size.width = width as f64;
        bounds.size.height = height as f64;
        CGContextDrawImage(context.0, bounds, image.0);
        drop(context);
        let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
        for row in rgba.chunks_exact(width * 4).rev() {
            for pixel in row.chunks_exact(4) {
                ppm.extend_from_slice(&pixel[..3]);
            }
        }
        Ok(ppm)
    }
}
