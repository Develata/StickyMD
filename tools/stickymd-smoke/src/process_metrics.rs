//! Windows process resource metrics for the development-only smoke harness.

use std::ffi::c_void;
use std::mem;
use std::os::windows::io::AsRawHandle;
use std::process::Child;
use std::time::Duration;

#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCountersEx2 {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
    private_working_set_size: usize,
    shared_commit_usage: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileTime {
    low_date_time: u32,
    high_date_time: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MemorySample {
    pub(crate) private_working_set_bytes: u64,
    pub(crate) private_bytes: u64,
    pub(crate) peak_working_set_bytes: u64,
    pub(crate) peak_private_bytes: u64,
}

unsafe extern "system" {
    fn K32GetProcessMemoryInfo(
        process: *mut c_void,
        counters: *mut ProcessMemoryCountersEx2,
        size: u32,
    ) -> i32;
    fn GetProcessTimes(
        process: *mut c_void,
        creation: *mut FileTime,
        exit: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
}

pub(crate) fn memory(child: &Child) -> Result<MemorySample, String> {
    let mut counters = ProcessMemoryCountersEx2 {
        cb: u32::try_from(mem::size_of::<ProcessMemoryCountersEx2>())
            .map_err(|_| "PROCESS_MEMORY_COUNTERS_EX2 size does not fit DWORD".to_owned())?,
        ..Default::default()
    };
    let handle = child.as_raw_handle();
    // SAFETY: `Child` owns a live process handle for the duration of the call; `counters` points to
    // writable storage whose `cb` exactly describes the EX2 layout required by the Win32 API.
    let succeeded = unsafe { K32GetProcessMemoryInfo(handle.cast(), &mut counters, counters.cb) };
    if succeeded == 0 {
        return Err(format!(
            "K32GetProcessMemoryInfo failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(MemorySample {
        private_working_set_bytes: counters.private_working_set_size as u64,
        private_bytes: counters.private_usage as u64,
        peak_working_set_bytes: counters.peak_working_set_size as u64,
        peak_private_bytes: counters.peak_pagefile_usage as u64,
    })
}

pub(crate) fn cpu_time(child: &Child) -> Result<Duration, String> {
    let mut creation = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let handle = child.as_raw_handle();
    // SAFETY: `Child` keeps the process handle valid; every output pointer refers to initialized,
    // writable `FILETIME` storage that remains alive until the call returns.
    let succeeded = unsafe {
        GetProcessTimes(
            handle.cast(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    if succeeded == 0 {
        return Err(format!(
            "GetProcessTimes failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    let ticks = file_time_ticks(kernel)
        .checked_add(file_time_ticks(user))
        .ok_or_else(|| "process CPU time overflowed".to_owned())?;
    Ok(Duration::from_nanos(ticks.saturating_mul(100)))
}

const fn file_time_ticks(value: FileTime) -> u64 {
    ((value.high_date_time as u64) << 32) | value.low_date_time as u64
}
