//! Owned child signals (termination, the latency recorder's flush) and the uptime clock.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::{
    ffi::{c_int, c_void},
    os::unix::ffi::OsStrExt,
    path::Path,
};
#[link(name = "System")]
unsafe extern "C" {
    fn kill(pid: c_int, signal: c_int) -> c_int;
    fn getuid() -> u32;
    fn proc_listpids(kind: u32, info: u32, buffer: *mut c_void, bytes: c_int) -> c_int;
    fn proc_pidpath(pid: c_int, buffer: *mut c_void, bytes: u32) -> c_int;
    fn clock_gettime_nsec_np(clock: u32) -> u64;
}
/// The caller owns an unreaped Child, so its PID cannot have been reused.
pub(super) fn terminate(pid: u32) -> Result<(), String> {
    signal(pid, 15, "SIGTERM")
}
/// SIGUSR1 (30 on Darwin), which makes the product's opt-in latency recorder write its file.
/// Same ownership precondition as `terminate`.
pub(super) fn user1(pid: u32) -> Result<(), String> {
    signal(pid, 30, "SIGUSR1")
}
fn signal(pid: u32, number: c_int, name: &str) -> Result<(), String> {
    let pid = c_int::try_from(pid).map_err(|_| "PID is out of range")?;
    if pid <= 0 {
        return Err("Refusing to signal a process group".into());
    }
    // SAFETY: positive PID names only the caller-owned child; the number is a Darwin signal.
    if unsafe { kill(pid, number) } != 0 {
        return Err(format!("{name}: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}
/// Nanoseconds of `CLOCK_UPTIME_RAW` (8 on Darwin): the clock of `mach_absolute_time` and of
/// event timestamps, so the driver's times compare with the product's.
pub(crate) fn uptime_ns() -> u64 {
    // SAFETY: reads a monotonic clock by its id; no preconditions.
    unsafe { clock_gettime_nsec_np(8) }
}

fn executable(pid: i32) -> Result<Option<Vec<u8>>, String> {
    let mut buffer = [0u8; 4096]; // PROC_PIDPATHINFO_MAXSIZE in the Darwin SDK.
    // SAFETY: positive PID and writable buffer with the exact SDK maximum size.
    let n = unsafe { proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if n <= 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(3) {
            return Ok(None);
        } // ESRCH: already gone.
        return Err(format!("proc_pidpath({pid}): {error}"));
    }
    let end = buffer
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated process path")?;
    Ok(Some(buffer[..end].to_vec()))
}

pub(super) fn matching_processes(path: &Path) -> Result<Vec<u32>, String> {
    // The fixture root is private (0700); LaunchServices runs under this same UID.
    // Query only that UID to avoid treating unreadable system processes as ours.
    // SAFETY: getuid has no preconditions or ownership effects.
    let uid = unsafe { getuid() };
    let mut pids = vec![0i32; 1024];
    loop {
        let bytes = (pids.len() * size_of::<i32>()) as i32;
        // SAFETY: initialized writable PID buffer; PROC_UID_ONLY=4, size in bytes.
        let n = unsafe { proc_listpids(4, uid, pids.as_mut_ptr().cast(), bytes) };
        // At minimum this driver belongs to our UID. Zero cannot prove absence.
        if n <= 0 {
            return Err(format!(
                "proc_listpids returned {n}: {}",
                std::io::Error::last_os_error()
            ));
        }
        if n >= bytes {
            if pids.len() >= 65536 {
                return Err("process enumeration exceeded budget".into());
            }
            pids.resize(pids.len() * 2, 0);
            continue;
        }
        if n % 4 != 0 {
            return Err("incomplete PID entry".into());
        }
        pids.truncate(n as usize / 4);
        let mut matching = Vec::new();
        for pid in pids.into_iter().filter(|pid| *pid > 0) {
            if executable(pid)?.as_deref() == Some(path.as_os_str().as_bytes()) {
                matching.push(pid as u32);
            }
        }
        return Ok(matching);
    }
}

pub(super) fn signal_matching(pid: u32, path: &Path, signal: i32) -> Result<(), String> {
    let pid = i32::try_from(pid).map_err(|_| "invalid foreign PID")?;
    if pid <= 0 || ![9, 15].contains(&signal) {
        return Err("invalid fixture signal".into());
    }
    if executable(pid)?.as_deref() != Some(path.as_os_str().as_bytes()) {
        return Ok(());
    }
    // SAFETY: positive PID rechecked against the exact private executable immediately
    // before signalling; never a process group. A PID-reuse race remains at the OS call.
    if unsafe { kill(pid, signal) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(3) {
            return Err(format!("signal {signal} to {pid}: {error}"));
        }
    }
    Ok(())
}
