//! A Windows job that terminates its processes when its last handle closes: dropping the
//! `Job`, or the owning process exiting by any route. Processes are started inside it:
//! created suspended, joined, then resumed, so none runs an instruction (or starts a
//! descendant) outside the job. Creating a process inside a job atomically needs
//! `PROC_THREAD_ATTRIBUTE_JOB_LIST`, which `std` does not expose on the pinned toolchain;
//! if the owner is killed between creation and joining, the process is left suspended,
//! never having run.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use super::{ChildGuard, STOP_WAIT};

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
const THREAD_SUSPEND_RESUME: u32 = 0x0000_0002;
const INVALID_HANDLE: isize = -1;
const RESUME_FAILED: u32 = u32::MAX;
const ERROR_NO_MORE_FILES: i32 = 18;

const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: u32 = 0x0000_1000;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;

/// `JOBOBJECT_BASIC_LIMIT_INFORMATION`.
#[repr(C)]
#[derive(Default)]
struct BasicLimits {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

/// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`.
#[repr(C)]
#[derive(Default)]
struct ExtendedLimits {
    basic: BasicLimits,
    io_counters: [u64; 6],
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[cfg(target_pointer_width = "64")]
const _: () = assert!(size_of::<ExtendedLimits>() == 144);

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> isize;
    fn SetInformationJobObject(
        job: isize,
        class: i32,
        information: *const c_void,
        length: u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: isize, process: isize) -> i32;
    fn CloseHandle(handle: isize) -> i32;
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> isize;
    fn Thread32First(snapshot: isize, entry: *mut ThreadEntry32) -> i32;
    fn Thread32Next(snapshot: isize, entry: *mut ThreadEntry32) -> i32;
    fn OpenThread(access: u32, inherit_handle: i32, thread_id: u32) -> isize;
    fn ResumeThread(thread: isize) -> u32;
}

/// `THREADENTRY32`.
#[repr(C)]
#[derive(Default)]
struct ThreadEntry32 {
    size: u32,
    usage: u32,
    thread_id: u32,
    owner_process_id: u32,
    base_priority: i32,
    delta_priority: i32,
    flags: u32,
}

/// Closes a Toolhelp snapshot or thread handle this module opened.
struct OwnedHandle(isize);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner of a handle a successful call returned.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Who else the job takes along when it closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Descendants {
    /// Processes the joined process starts are killed with it (helpers whose children
    /// may hold their output pipes).
    Included,
    /// Processes the joined process starts leave the job (StickyMD opening a browser).
    BreakAway,
}

pub(crate) struct Job(isize);

impl Job {
    pub(crate) fn create(descendants: Descendants) -> Result<Self, String> {
        // SAFETY: null attributes and name request an unnamed job with default security.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw == 0 {
            return Err(format!(
                "cannot create a job object: {}",
                io::Error::last_os_error()
            ));
        }
        let job = Self(raw);
        let mut limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if descendants == Descendants::BreakAway {
            limit_flags |= JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
        }
        let limits = ExtendedLimits {
            basic: BasicLimits {
                limit_flags,
                ..BasicLimits::default()
            },
            ..ExtendedLimits::default()
        };
        // SAFETY: `job` is a live job; `limits` is a JOBOBJECT_EXTENDED_LIMIT_INFORMATION
        // laid out as the SDK declares it (size checked above) and outlives the call.
        let set = unsafe {
            SetInformationJobObject(
                job.0,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                (&raw const limits).cast(),
                size_of::<ExtendedLimits>() as u32,
            )
        };
        if set == 0 {
            return Err(format!(
                "cannot configure a job object: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(job)
    }

    /// Starts `command` inside this job (suspended, joined, resumed). The command's
    /// creation flags are replaced. On any failure the process, which has not run, is
    /// stopped.
    pub(crate) fn spawn(&self, command: &mut Command, context: &str) -> Result<ChildGuard, String> {
        command.creation_flags(CREATE_SUSPENDED);
        let mut child = ChildGuard::spawn(command, context)?;
        let pid = child.id();
        if let Err(error) = self.assign(&child).and_then(|()| resume(pid)) {
            return Err(match child.stop_within(STOP_WAIT) {
                Ok(()) => error,
                Err(stop) => format!("{error}; stopping PID {pid} also failed: {stop}"),
            });
        }
        Ok(child)
    }

    /// Starts the GUI `executable` inside this job, as `ChildGuard::start` would outside.
    pub(crate) fn start(&self, executable: &Path) -> Result<ChildGuard, String> {
        self.spawn(
            &mut super::gui_command(executable)?,
            &format!("cannot start {}", executable.display()),
        )
    }

    /// Adds `child`. Fails for a process that already exited, among other reasons.
    pub(crate) fn assign(&self, child: &ChildGuard) -> Result<(), String> {
        // SAFETY: the job handle is live, and `child` owns its process handle for the
        // duration of the call.
        let assigned = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as isize) };
        if assigned == 0 {
            return Err(format!(
                "cannot add PID {} to a job object: {}",
                child.id(),
                io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner of a successful CreateJobObjectW handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Resumes every thread of the suspended process `pid` (one, for a fresh process).
fn resume(pid: u32) -> Result<(), String> {
    // SAFETY: value arguments; a successful result is an owned snapshot handle.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE {
        return Err(format!(
            "cannot list threads to resume PID {pid}: {}",
            io::Error::last_os_error()
        ));
    }
    let snapshot = OwnedHandle(snapshot);
    let mut entry = ThreadEntry32 {
        size: size_of::<ThreadEntry32>() as u32,
        ..ThreadEntry32::default()
    };
    let mut resumed = 0;
    // SAFETY: the snapshot is live and `entry` is writable with its size field set.
    let mut available = unsafe { Thread32First(snapshot.0, &raw mut entry) } != 0;
    while available {
        if entry.owner_process_id == pid {
            // SAFETY: value arguments; a non-zero result is an owned thread handle.
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.thread_id) };
            if thread == 0 {
                return Err(format!(
                    "cannot open a thread of PID {pid}: {}",
                    io::Error::last_os_error()
                ));
            }
            let thread = OwnedHandle(thread);
            // SAFETY: `thread` is a live handle with THREAD_SUSPEND_RESUME access.
            let previous = unsafe { ResumeThread(thread.0) };
            if previous == RESUME_FAILED {
                return Err(format!(
                    "cannot resume PID {pid}: {}",
                    io::Error::last_os_error()
                ));
            }
            resumed_once(pid, previous)?;
            resumed += 1;
        }
        // SAFETY: as for Thread32First; the loop owns `entry` throughout.
        available = unsafe { Thread32Next(snapshot.0, &raw mut entry) } != 0;
    }
    // The enumeration ends with ERROR_NO_MORE_FILES; anything else may have missed a thread.
    let end = io::Error::last_os_error();
    if end.raw_os_error() != Some(ERROR_NO_MORE_FILES) {
        return Err(format!(
            "listing the threads of PID {pid} stopped early: {end}"
        ));
    }
    if resumed == 0 {
        return Err(format!("PID {pid} has no thread to resume"));
    }
    Ok(())
}

/// `ResumeThread` returns the previous suspend count: a thread created suspended had 1 and
/// now runs. Anything else means someone else suspended it too, and it is still suspended;
/// the caller stops the process instead of resuming a suspension it does not own.
fn resumed_once(pid: u32, previous: u32) -> Result<(), String> {
    if previous == 1 {
        Ok(())
    } else {
        Err(format!(
            "a thread of PID {pid} had suspend count {previous}, not 1, so it is not running"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::resumed_once;

    #[test]
    fn only_a_single_creation_suspension_counts_as_resumed() {
        assert_eq!(resumed_once(7, 1), Ok(()));
        for previous in [0, 2, 5] {
            assert!(resumed_once(7, previous).is_err(), "{previous}");
        }
    }
}
