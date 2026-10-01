//! Native facts for resource comparisons on a smoke-owned disposable child only.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use super::*;
use std::process::Child;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowLongPtrW(window: isize, index: i32, value: isize) -> isize;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetLastError(error: u32);
}

fn owned(child: &Child, window: WindowHandle) -> Result<(), String> {
    ensure_window(window)?;
    let mut pid = 0;
    // SAFETY: HWND is live; the output is writable scalar storage.
    let thread = unsafe { GetWindowThreadProcessId(window.0, &raw mut pid) };
    if thread == 0 || pid != child.id() {
        return Err("resource comparison HWND does not belong to the supplied child".into());
    }
    Ok(())
}

/// The control arm changes only APPWINDOW/TOOLWINDOW after normal startup.
/// It is a native-style resource comparison, not a supported product mode.
pub(crate) fn apply_normal_style(child: &Child, window: WindowHandle) -> Result<(), String> {
    owned(child, window)?;
    // SAFETY: synchronous read of the verified test child's HWND.
    let original = unsafe { GetWindowLongPtrW(window.0, GWL_EXSTYLE) };
    let target = normal_style(original);
    // SAFETY: these calls affect only the verified disposable test window;
    // no pointer or handle ownership is transferred.
    let previous = unsafe {
        SetLastError(0);
        SetWindowLongPtrW(window.0, GWL_EXSTYLE, target)
    };
    if previous == 0 && std::io::Error::last_os_error().raw_os_error() != Some(0) {
        return Err(format!(
            "cannot apply comparison style: {}",
            std::io::Error::last_os_error()
        ));
    }
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;
    const SWP_NOOWNERZORDER: u32 = 0x0200;
    // SAFETY: recalculates the verified HWND frame without moving, resizing,
    // activating, or changing the z-order of this or another window.
    if unsafe {
        SetWindowPos(
            window.0,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOACTIVATE
                | SWP_NOZORDER
                | SWP_FRAMECHANGED
                | SWP_NOOWNERZORDER,
        )
    } == 0
    {
        return Err(format!(
            "cannot refresh comparison frame: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn normal_style(original: isize) -> isize {
    (original & !WS_EX_TOOLWINDOW) | WS_EX_APPWINDOW
}

pub(crate) fn facts(
    child: &Child,
    window: WindowHandle,
    normal: bool,
) -> Result<(f64, f64, u32), String> {
    owned(child, window)?;
    let style = style_facts(window)?;
    validate_style(style, normal)?;
    let rect = client_rect(window)?;
    // SAFETY: scalar query of the verified child's live HWND.
    let dpi = unsafe { GetDpiForWindow(window.0) };
    if dpi == 0 {
        return Err("cannot observe comparison window DPI".into());
    }
    let scale = f64::from(dpi) / 96.0;
    Ok((
        f64::from(rect.width) / scale,
        f64::from(rect.height) / scale,
        dpi,
    ))
}

fn validate_style(style: WindowStyleFacts, normal: bool) -> Result<(), String> {
    if style.tool_window == normal
        || style.app_window != normal
        || style.no_activate
        || style.transparent
    {
        return Err(format!(
            "resource comparison style changed or is ineligible: {style:?}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_style_keeps_opacity_activation_topmost_and_unknown_bits() {
        let original = WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TOPMOST | 0x4000;
        assert_eq!(
            normal_style(original) ^ original,
            WS_EX_TOOLWINDOW | WS_EX_APPWINDOW
        );
        let valid = WindowStyleFacts {
            tool_window: false,
            app_window: true,
            no_activate: false,
            transparent: false,
        };
        assert!(validate_style(valid, true).is_ok());
        assert!(validate_style(valid, false).is_err());
        assert!(
            validate_style(
                WindowStyleFacts {
                    no_activate: true,
                    ..valid
                },
                true
            )
            .is_err()
        );
        assert!(
            validate_style(
                WindowStyleFacts {
                    transparent: true,
                    ..valid
                },
                true
            )
            .is_err()
        );
    }
}
