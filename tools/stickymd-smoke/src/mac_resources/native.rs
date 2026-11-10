//! All macOS FFI for resource measurements; SDK layouts from sys/resource.h and CoreGraphics.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::model::Sample;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::time::Instant;

type Cf = *const c_void;

#[repr(C)]
#[derive(Default)]
struct RusageInfoV4 {
    uuid: [u8; 16],
    user_time: u64,
    system_time: u64,
    pkg_idle_wkups: u64,
    interrupt_wkups: u64,
    pageins: u64,
    wired_size: u64,
    resident_size: u64,
    phys_footprint: u64,
    // ri_proc_start_abstime through ri_runnable_time, in SDK order.
    remaining: [u64; 27],
}

#[repr(C)]
#[derive(Default)]
struct Timebase {
    numer: u32,
    denom: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Size {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

#[link(name = "System")]
unsafe extern "C" {
    fn proc_pid_rusage(pid: c_int, flavor: c_int, buffer: *mut *mut c_void) -> c_int;
    fn mach_timebase_info(info: *mut Timebase) -> c_int;
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn sysctlbyname(
        name: *const c_char,
        old: *mut c_void,
        size: *mut usize,
        new: *mut c_void,
        new_size: usize,
    ) -> c_int;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Cf);
    fn CFArrayGetCount(array: Cf) -> isize;
    fn CFArrayGetValueAtIndex(array: Cf, index: isize) -> Cf;
    fn CFDictionaryGetValue(dictionary: Cf, key: Cf) -> Cf;
    fn CFNumberGetValue(number: Cf, kind: c_int, value: *mut c_void) -> bool;
    fn CFGetTypeID(value: Cf) -> usize;
    fn CFNumberGetTypeID() -> usize;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGWindowOwnerPID: Cf;
    static kCGWindowBounds: Cf;
    fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> Cf;
    fn CGRectMakeWithDictionaryRepresentation(dictionary: Cf, rect: *mut Rect) -> bool;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> Rect;
    fn CGDisplayCopyDisplayMode(display: u32) -> Cf;
    fn CGDisplayModeGetPixelWidth(mode: Cf) -> usize;
    fn CGDisplayModeGetPixelHeight(mode: Cf) -> usize;
    fn CGDisplayModeRelease(mode: Cf);
}

pub(super) fn timebase() -> Result<(u32, u32), String> {
    let mut info = Timebase::default();
    // SAFETY: info is writable storage with the SDK's two-u32 layout.
    if unsafe { mach_timebase_info(&mut info) } != 0 || info.numer == 0 || info.denom == 0 {
        return Err("mach_timebase_info failed".into());
    }
    Ok((info.numer, info.denom))
}

pub(super) fn sample(pid: u32, started: Instant) -> Result<Sample, String> {
    let pid = c_int::try_from(pid).map_err(|_| "PID is out of range")?;
    let mut info = RusageInfoV4::default();
    // SAFETY: flavor 4 writes exactly rusage_info_v4 (296 bytes); the API's opaque
    // rusage_info_t* points to that storage, not to an additional indirection.
    if unsafe { proc_pid_rusage(pid, 4, (&raw mut info).cast()) } != 0 {
        return Err(format!(
            "proc_pid_rusage: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(Sample {
        elapsed_seconds: started.elapsed().as_secs_f64(),
        footprint_bytes: info.phys_footprint,
        user_ticks: info.user_time,
        system_ticks: info.system_time,
        pkg_idle_wakeups: info.pkg_idle_wkups,
        interrupt_wakeups: info.interrupt_wkups,
    })
}

/// The caller owns an unreaped Child, so its PID cannot have been reused.
pub(super) fn terminate(pid: u32) -> Result<(), String> {
    let pid = c_int::try_from(pid).map_err(|_| "PID is out of range")?;
    if pid <= 0 {
        return Err("Refusing to signal a process group".into());
    }
    // SAFETY: positive PID names only the caller-owned child; SIGTERM is 15 on Darwin.
    if unsafe { kill(pid, 15) } != 0 {
        return Err(format!("SIGTERM: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// No titles or images are requested. Exclude the tiny status-item window.
pub(super) fn window_ready(pid: u32) -> Result<bool, String> {
    // SAFETY: options are OnScreenOnly | ExcludeDesktopElements; 0 is the null window ID.
    let array = unsafe { CGWindowListCopyWindowInfo(1 | 16, 0) };
    if array.is_null() {
        return Err("CGWindowListCopyWindowInfo returned null".into());
    }
    let mut ready = false;
    // SAFETY: the Create-rule array owns dictionaries with the documented owner and bounds
    // entries. Borrowed values stay alive until CFRelease below. Numbers are type-checked,
    // indices are bounded, and all output storage has the corresponding SDK layout.
    unsafe {
        for index in 0..CFArrayGetCount(array) {
            let dictionary = CFArrayGetValueAtIndex(array, index);
            let owner = CFDictionaryGetValue(dictionary, kCGWindowOwnerPID);
            let bounds = CFDictionaryGetValue(dictionary, kCGWindowBounds);
            let mut owner_pid = 0_i64;
            let mut rect = Rect::default();
            if !owner.is_null()
                && !bounds.is_null()
                && CFGetTypeID(owner) == CFNumberGetTypeID()
                && CFNumberGetValue(owner, 4, (&raw mut owner_pid).cast())
                && owner_pid == i64::from(pid)
                && CGRectMakeWithDictionaryRepresentation(bounds, &mut rect)
                && rect.size.width >= 220.0
                && rect.size.height >= 120.0
            {
                ready = true;
                break;
            }
        }
        CFRelease(array);
    }
    Ok(ready)
}

pub(super) fn display() -> Result<[f64; 4], String> {
    // SAFETY: getters borrow the main display; the copied mode is released exactly once.
    let dimensions = unsafe {
        let display = CGMainDisplayID();
        let mode = CGDisplayCopyDisplayMode(display);
        if mode.is_null() {
            return Err("No main display mode".into());
        }
        let points = CGDisplayBounds(display).size;
        let result = [
            CGDisplayModeGetPixelWidth(mode) as f64,
            CGDisplayModeGetPixelHeight(mode) as f64,
            points.width,
            points.height,
        ];
        CGDisplayModeRelease(mode);
        result
    };
    if dimensions.iter().any(|n| !n.is_finite() || *n <= 0.0) {
        return Err("Invalid main display dimensions".into());
    }
    Ok(dimensions)
}

fn sysctl(name: &CStr) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0_u8; 4096];
    let mut size = bytes.len();
    // SAFETY: name is NUL-terminated, bytes is writable for size bytes, and a null
    // new pointer with zero length requests a read without changing system state.
    if unsafe {
        sysctlbyname(
            name.as_ptr(),
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return Err(format!(
            "sysctl {name:?}: {}",
            std::io::Error::last_os_error()
        ));
    }
    bytes.truncate(size);
    Ok(bytes)
}

pub(super) fn system_text(name: &CStr) -> Result<String, String> {
    let bytes = sysctl(name)?;
    String::from_utf8(bytes.strip_suffix(&[0]).unwrap_or(&bytes).to_vec())
        .map_err(|e| e.to_string())
}

pub(super) fn ram_bytes() -> Result<u64, String> {
    let bytes = sysctl(c"hw.memsize")?;
    Ok(u64::from_ne_bytes(
        bytes.try_into().map_err(|_| "hw.memsize is not a u64")?,
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn current_process_rusage_is_plausible() {
        assert_eq!(std::mem::size_of::<super::RusageInfoV4>(), 296);
        let started = std::time::Instant::now();
        let first = super::sample(std::process::id(), started).unwrap();
        let second = super::sample(std::process::id(), started).unwrap();
        assert!(first.footprint_bytes > 0 && second.footprint_bytes > 0);
        assert!(second.user_ticks >= first.user_ticks);
        assert!(second.system_ticks >= first.system_ticks);
        let (numer, denom) = super::timebase().unwrap();
        assert!(numer > 0 && denom > 0);
    }
}
