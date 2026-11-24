//! Shared macOS smoke lifecycle and isolated fixture preparation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
pub(crate) mod fixture;
mod native;
pub(crate) mod output;
pub(crate) use native::uptime_ns;
mod processes;
pub(crate) mod session;

/// The PIDs of this user running exactly `executable` (a fixture's private copy).
pub(crate) fn fixture_processes(executable: &std::path::Path) -> Result<Vec<u32>, String> {
    native::matching_processes(executable)
}

/// Ends every process of this user running exactly `executable` — SIGTERM, then SIGKILL after
/// 3 s — and confirms each gone, observing for at least 3 s; returns the PIDs it ended.
pub(crate) fn end_fixture_processes(executable: &std::path::Path) -> Result<Vec<u32>, String> {
    processes::drain(
        &mut processes::Native {
            executable,
            started: std::time::Instant::now(),
        },
        0,
    )
}
