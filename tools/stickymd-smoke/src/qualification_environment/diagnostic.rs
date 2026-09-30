//! Transient desktop identity for opt-in diagnostic reuse; never qualification authority.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::QualificationEnvironmentStatus;
mod logon;

#[repr(C)]
#[derive(Default)]
struct PowerStatus {
    ac: u8,
    flags: u8,
    percent: u8,
    saver: u8,
    life: u32,
    full_life: u32,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn GetSystemMetrics(index: i32) -> i32;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *const std::ffi::c_void,
    ) -> isize;
    fn DestroyWindow(window: isize) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
}

pub(crate) fn capture() -> Result<String, String> {
    let environment = super::inspect();
    if environment.status != QualificationEnvironmentStatus::Valid || environment.display_count != 1
    {
        return Err("diagnostic reuse requires an unlocked, valid single-monitor desktop".into());
    }
    crate::window_control::enable_per_monitor_v2_dpi_awareness()?;
    let work = crate::window_control::primary_work_area()?;
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    // SAFETY: predefined STATIC class and NUL-terminated strings; no parent or user pointers.
    // No WS_VISIBLE: this owned window cannot activate or obstruct the desktop.
    let window = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0x8000_0000,
            work.x,
            work.y,
            1,
            1,
            0,
            0,
            0,
            std::ptr::null(),
        )
    };
    if window == 0 {
        return Err("cannot inspect primary display DPI".into());
    }
    // SAFETY: the hidden window belongs to this thread and remains live until DestroyWindow.
    let (dpi, destroyed) = unsafe { (GetDpiForWindow(window), DestroyWindow(window)) };
    let mut power = PowerStatus::default();
    let mut session = 0;
    // SAFETY: out pointers reference correctly sized live structs/scalars. Metrics take no pointers.
    let (power_ok, session_ok, width, height) = unsafe {
        (
            GetSystemPowerStatus(&mut power),
            ProcessIdToSessionId(std::process::id(), &mut session),
            GetSystemMetrics(0),
            GetSystemMetrics(1),
        )
    };
    if destroyed == 0
        || dpi == 0
        || power_ok == 0
        || session_ok == 0
        || power.ac == 255
        || width <= 0
        || height <= 0
    {
        return Err("incomplete diagnostic desktop/power identity".into());
    }
    Ok(format!(
        "session={session};display={width}x{height};dpi={dpi};work={work:?};ac={};saver={};logon={}",
        power.ac,
        power.saver,
        logon::identity()?
    ))
}
