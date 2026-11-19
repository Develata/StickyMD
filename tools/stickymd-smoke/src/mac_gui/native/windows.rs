//! CoreGraphics window facts and optional normalized PPM screenshot, never product state.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::super::model::{Rect, Window, select_window};
use super::super::pixels::{Image, Shot};
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
    let windows = all(pid)?
        .into_iter()
        .filter(|w| w.bounds.size.width >= 220.0 && w.bounds.size.height >= 120.0)
        .collect();
    select_window(windows, id).map_err(|found| {
        let scope = if id.is_some() { "" } else { " on-screen" };
        format!("expected one PID {pid}{scope} content window (id={id:?}), found {found}")
    })
}
/// Every window `pid` owns (on screen or not, desktop elements excluded), with its level.
pub(in crate::mac_gui) fn all(pid: u32) -> Result<Vec<Window>, String> {
    // SAFETY: all windows excluding desktop, owned Create-rule array; no image capture.
    let array = Owned::take(unsafe { CGWindowListCopyWindowInfo(16, 0) })?;
    let mut windows = Vec::new();
    for dict in array.array()? {
        if dict.get("kCGWindowOwnerPID")?.number()? != f64::from(pid) {
            continue;
        }
        let number = dict.get("kCGWindowNumber")?.number()? as u32;
        let b = dict.get("kCGWindowBounds")?;
        let mut bounds = Rect::default();
        // SAFETY: SDK output storage matches CGRect; b is a live bounds dictionary.
        if !unsafe { CGRectMakeWithDictionaryRepresentation(b.0, &mut bounds) } {
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
    Ok(windows)
}
/// The owner name of an on-screen window of `pid`, to name what covers the product in a
/// diagnostic; no titles or image capture.
pub(in crate::mac_gui) fn owner_name(pid: u32) -> Option<String> {
    // SAFETY: OnScreenOnly | ExcludeDesktopElements; owned Create-rule array.
    let array = Owned::take(unsafe { CGWindowListCopyWindowInfo(1 | 16, 0) }).ok()?;
    array.array().ok()?.iter().find_map(|dict| {
        let owner = dict.get("kCGWindowOwnerPID").ok()?.number().ok()?;
        (owner == f64::from(pid))
            .then(|| dict.get("kCGWindowOwnerName").ok()?.text())
            .flatten()
    })
}
/// Captures `rect` for the window `id` as RGBA rows from top to bottom; window shadows are
/// excluded (`kCGWindowImageBoundsIgnoreFraming`). The bitmap context's memory holds the top row
/// first, so no row is reversed (the F01 captures written with reversed rows were upside down).
pub(in crate::mac_gui) fn capture(rect: Rect, shot: Shot, id: u32) -> Result<Image, String> {
    let (list, window) = match shot {
        Shot::Alone => (8, id),
        Shot::Composite => (1, 0),
        Shot::Below => (2, id),
    };
    // SAFETY: caller has preflighted capture permission. Create-rule image/context/space are each
    // released once; the context writes only into `rgba`, which outlives it.
    unsafe {
        let image = Owned::take(CGWindowListCreateImage(rect, list, window, 1))?;
        let width = CGImageGetWidth(image.0);
        let height = CGImageGetHeight(image.0);
        let len = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(4))
            .filter(|n| *n > 0 && *n <= 128 * 1024 * 1024)
            .ok_or("invalid capture dimensions")?;
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
        Ok(Image {
            width,
            height,
            rgba,
        })
    }
}
/// The window alone, as a binary PPM (F01's optional evidence).
pub(in crate::mac_gui) fn screenshot(w: &Window) -> Result<Vec<u8>, String> {
    let image = capture(w.bounds, Shot::Alone, w.id)?;
    let mut ppm = format!("P6\n{} {}\n255\n", image.width, image.height).into_bytes();
    for pixel in image.rgba.chunks_exact(4) {
        ppm.extend_from_slice(&pixel[..3]);
    }
    Ok(ppm)
}
