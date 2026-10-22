//! Read-only native facts for the manual recorder; no input or window mutation.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::*;

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: NativeRect,
    work: NativeRect,
    flags: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumDisplayMonitors(
        dc: isize,
        clip: *const NativeRect,
        callback: unsafe extern "system" fn(isize, isize, *mut NativeRect, isize) -> i32,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    fn IsIconic(window: isize) -> i32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Monitor {
    pub(crate) primary: bool,
    pub(crate) bounds: WindowRect,
    pub(crate) work: WindowRect,
}

pub(crate) fn monitors() -> Result<Vec<Monitor>, String> {
    let mut found: Result<Vec<Monitor>, String> = Ok(Vec::new());
    // SAFETY: callback is synchronous; its context is exclusively borrowed until return.
    if unsafe { EnumDisplayMonitors(0, std::ptr::null(), collect, (&raw mut found) as isize) } == 0
    {
        return Err(found
            .err()
            .unwrap_or_else(|| "cannot enumerate display rectangles".to_owned()));
    }
    let mut monitors = found?;
    monitors.sort_by_key(|monitor| (monitor.bounds.x, monitor.bounds.y));
    if monitors.is_empty()
        || monitors.len() > 16
        || monitors.iter().filter(|m| m.primary).count() != 1
    {
        return Err("display topology has no unique primary or exceeds 16 monitors".to_owned());
    }
    Ok(monitors)
}

unsafe extern "system" fn collect(handle: isize, _: isize, _: *mut NativeRect, data: isize) -> i32 {
    // SAFETY: data is the live Result<Vec<Monitor>, String> supplied to this enumeration.
    let result = unsafe { &mut *(data as *mut Result<Vec<Monitor>, String>) };
    let mut info = MonitorInfo {
        size: std::mem::size_of::<MonitorInfo>() as u32,
        monitor: NativeRect::default(),
        work: NativeRect::default(),
        flags: 0,
    };
    // SAFETY: the enumerated monitor is live and info has the advertised Win32 layout.
    if unsafe { GetMonitorInfoW(handle, &raw mut info) } == 0 {
        *result = Err("cannot read enumerated display rectangle".to_owned());
        return 0;
    }
    let rect = |r: NativeRect| WindowRect {
        x: r.left,
        y: r.top,
        width: r.right.saturating_sub(r.left).max(0) as u32,
        height: r.bottom.saturating_sub(r.top).max(0) as u32,
    };
    if let Ok(monitors) = result {
        monitors.push(Monitor {
            primary: info.flags & 1 != 0,
            bounds: rect(info.monitor),
            work: rect(info.work),
        });
        if monitors.len() > 16 {
            *result = Err("more than 16 displays".to_owned());
            return 0;
        }
    }
    1
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    pub(crate) visible: bool,
    pub(crate) minimized: bool,
    pub(crate) collapsed: bool,
    pub(crate) expanded: bool,
    pub(crate) activated: bool,
}

pub(crate) fn state(hwnd: WindowHandle, monitors: &[Monitor]) -> Result<State, String> {
    let rect = window_rect(hwnd)?;
    let visible = is_visible(hwnd)?;
    // SAFETY: the preceding queries validate the borrowed HWND; IsIconic copies one flag.
    let minimized = unsafe { IsIconic(hwnd.0) } != 0;
    let strip = i64::from(dip_pixels(hwnd, 3.0)?);
    let right = i64::from(rect.x) + i64::from(rect.width);
    let bottom = i64::from(rect.y) + i64::from(rect.height);
    let collapsed = monitors.iter().any(|monitor| {
        let w = monitor.work;
        let wr = i64::from(w.x) + i64::from(w.width);
        let wb = i64::from(w.y) + i64::from(w.height);
        let vertical = rect.y >= w.y && bottom <= wb;
        let horizontal = rect.x >= w.x && right <= wr;
        (vertical && rect.x < w.x && right - i64::from(w.x) == strip)
            || (vertical && right > wr && wr - i64::from(rect.x) == strip)
            || (horizontal && rect.y < w.y && bottom - i64::from(w.y) == strip)
    });
    let expanded = visible
        && !minimized
        && monitors.iter().any(|m| {
            rect.x >= m.work.x
                && rect.y >= m.work.y
                && right <= i64::from(m.work.x) + i64::from(m.work.width)
                && bottom <= i64::from(m.work.y) + i64::from(m.work.height)
        });
    let activation = activation_facts(hwnd)?;
    Ok(State {
        visible,
        minimized,
        collapsed,
        expanded,
        activated: activation.foreground && activation.active && activation.focused,
    })
}
