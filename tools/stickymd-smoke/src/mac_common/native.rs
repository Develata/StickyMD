//! Owned child termination only.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::ffi::c_int;
#[link(name = "System")]
unsafe extern "C" {
    fn kill(pid: c_int, signal: c_int) -> c_int;
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
