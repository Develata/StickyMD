//! Smoke-owned GUI process lifecycle and measurement-isolation checks.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::mem;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

mod bounded;
mod job;
mod product;
mod session;
pub(crate) use product::{Product, process_product};
pub(crate) use session::outside_other_sessions;

pub(crate) use bounded::run_bounded_output;
pub(crate) use job::{Descendants, Job};

/// The manual recorder's session root under the system temp directory; its instances are
/// smoke-owned like every other qualification root.
pub(crate) const MANUAL_SESSIONS: &str = "stickymd-manual-sessions";

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// A terminated process exits in milliseconds; one that has not after this is stuck in
/// the kernel, and waiting longer would only hang the caller.
pub(crate) const STOP_WAIT: Duration = Duration::from_secs(10);
const FALSE: i32 = 0;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const INVALID_SNAPSHOT_HANDLE: isize = -1;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;
const ERROR_INVALID_PARAMETER: i32 = 87;
const ERROR_NO_MORE_FILES: i32 = 18;
const MAX_WINDOWS_PATH_CHARS: usize = 32_768;

/// Owns one smoke-started GUI process and reaps it when the scope ends.
pub(crate) struct ChildGuard(Child);

impl ChildGuard {
    pub(crate) fn start(executable: &Path) -> Result<Self, String> {
        Self::spawn(
            &mut gui_command(executable)?,
            &format!("cannot start {}", executable.display()),
        )
    }

    pub(crate) fn spawn(command: &mut Command, context: &str) -> Result<Self, String> {
        command
            .spawn()
            .map(Self)
            .map_err(|error| format!("{context}: {error}"))
    }

    pub(crate) fn id(&self) -> u32 {
        self.0.id()
    }

    pub(crate) fn is_running(&mut self) -> Result<bool, String> {
        self.0
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| format!("cannot inspect StickyMD child: {error}"))
    }

    /// Terminates the process and reaps it, waiting at most `STOP_WAIT`.
    pub(crate) fn kill_and_wait(&mut self) -> Result<(), String> {
        self.stop_within(STOP_WAIT)
    }

    /// Terminates the process if it runs and waits at most `limit` for it to exit.
    pub(crate) fn stop_within(&mut self, limit: Duration) -> Result<(), String> {
        if self.is_running()? {
            self.0
                .kill()
                .map_err(|error| format!("cannot terminate StickyMD child: {error}"))?;
        }
        let deadline = Instant::now() + limit;
        while self.is_running()? {
            if Instant::now() >= deadline {
                return Err(format!(
                    "PID {} did not exit within {limit:?} of termination",
                    self.0.id()
                ));
            }
            thread::sleep(EXIT_POLL_INTERVAL);
        }
        Ok(())
    }

    pub(crate) fn wait_for_exit(&mut self, timeout: Duration) -> Result<(), String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self
                .0
                .try_wait()
                .map_err(|error| format!("cannot inspect StickyMD child: {error}"))?
            {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("StickyMD exited unsuccessfully: {status}"))
                };
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "StickyMD did not exit within the bounded {timeout:?} qualification timeout"
                ));
            }
            thread::sleep(EXIT_POLL_INTERVAL);
        }
    }
}

impl Deref for ChildGuard {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Bounded, so an unkillable process cannot hang the scope that owns it.
        if let Err(error) = self.stop_within(STOP_WAIT) {
            eprintln!("warning: cannot reap a smoke-started process: {error}");
        }
    }
}

/// The command that starts a GUI `executable` from its own directory with no console I/O.
pub(crate) fn gui_command(executable: &Path) -> Result<Command, String> {
    let parent = executable
        .parent()
        .ok_or_else(|| format!("{} has no parent", executable.display()))?;
    let mut command = Command::new(executable);
    command
        .current_dir(parent)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProcessSnapshot {
    pid: u32,
    /// Why the image path could not be read, when it could not.
    executable: Result<PathBuf, String>,
}

/// Rejects a performance/resource measurement contaminated by an older smoke child, and
/// one whose isolation cannot be confirmed because a StickyMD image path is unreadable.
pub(crate) fn ensure_no_stale_smoke_stickymd() -> Result<(), String> {
    let mut stale = Vec::new();
    let mut unconfirmed = Vec::new();
    for process in running_stickymd_processes()? {
        match &process.executable {
            Ok(executable) if is_smoke_owned_stickymd(executable) => stale.push(process.pid),
            // The reported path may name a junction or carry 8.3 names; the final path
            // resolves both, and a smoke root under either name counts.
            Ok(executable) => match std::fs::canonicalize(executable) {
                Ok(final_path) if is_smoke_owned_stickymd(&final_path) => {
                    stale.push(process.pid);
                }
                Ok(_) => {}
                Err(error) => unconfirmed.push(format!(
                    "PID {}: cannot resolve {}: {error}",
                    process.pid,
                    executable.display()
                )),
            },
            Err(error) => unconfirmed.push(format!("PID {}: {error}", process.pid)),
        }
    }
    if !unconfirmed.is_empty() {
        return Err(format!(
            "cannot confirm measurement isolation: {}; the preflight will not terminate pre-existing processes",
            unconfirmed.join("; ")
        ));
    }
    stale.sort_unstable();
    stale.dedup();
    if stale.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "stale smoke-owned StickyMD process detected before isolated measurement; pids={stale:?}; the preflight will not terminate pre-existing processes"
        ))
    }
}

/// Whether `executable` runs from a qualification root the tooling names. Any directory of
/// the path counts, not only one under this process's temp directory: another terminal may
/// run the tooling with another `TMP`. A user directory that happens to carry such a name
/// only makes a measurement refuse to start; nothing is ever terminated.
fn is_smoke_owned_stickymd(executable: &Path) -> bool {
    if !executable
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("StickyMD.exe"))
    {
        return false;
    }
    executable
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .any(|component| {
            let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
            [
                "stickymd-smoke-",
                "stickymd-g3-",
                "stickymd-g4-",
                "stickymd-g5-",
                "stickymd-downloaded-smoke-",
                "stickymd-verify-",
                MANUAL_SESSIONS,
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        })
}

/// The process IDs of every running StickyMD, whatever its program directory.
pub(crate) fn running_stickymd_pids() -> Result<Vec<u32>, String> {
    Ok(running_stickymd_processes()?
        .into_iter()
        .map(|process| process.pid)
        .collect())
}

/// The StickyMD processes in `running` that this run does not own (`owned`).
pub(crate) fn foreign_stickymd(running: &[u32], owned: &[u32]) -> Vec<u32> {
    running
        .iter()
        .copied()
        .filter(|pid| !owned.contains(pid))
        .collect()
}

fn running_stickymd_processes() -> Result<Vec<ProcessSnapshot>, String> {
    // SAFETY: CreateToolhelp32Snapshot takes value arguments and returns an
    // owned snapshot handle on success.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_SNAPSHOT_HANDLE {
        return Err(format!(
            "cannot enumerate processes for measurement isolation: {}",
            std::io::Error::last_os_error()
        ));
    }
    let snapshot = SnapshotHandle(snapshot);
    // SAFETY: ProcessEntry32W is a plain C record. The required size field is
    // initialized before the first Toolhelp call.
    let mut entry: ProcessEntry32W = unsafe { mem::zeroed() };
    entry.size = u32::try_from(mem::size_of::<ProcessEntry32W>())
        .map_err(|_| "PROCESSENTRY32W size overflow".to_owned())?;
    // SAFETY: snapshot is valid and entry points to writable storage with the
    // documented size field initialized.
    let mut available = unsafe { Process32FirstW(snapshot.0, &raw mut entry) } != FALSE;
    let mut processes = Vec::new();
    while available {
        if executable_name(&entry.executable).eq_ignore_ascii_case("StickyMD.exe") {
            // A process that exited since the snapshot is no longer a contaminant.
            match query_executable(entry.process_id) {
                Ok(Some(executable)) => processes.push(ProcessSnapshot {
                    pid: entry.process_id,
                    executable: Ok(executable),
                }),
                Ok(None) => {}
                Err(error) => processes.push(ProcessSnapshot {
                    pid: entry.process_id,
                    executable: Err(error),
                }),
            }
        }
        // SAFETY: snapshot and entry remain valid across the enumeration loop.
        available = unsafe { Process32NextW(snapshot.0, &raw mut entry) } != FALSE;
    }
    // The enumeration ends with ERROR_NO_MORE_FILES; anything else lost processes.
    let end = std::io::Error::last_os_error();
    if end.raw_os_error() != Some(ERROR_NO_MORE_FILES) {
        return Err(format!(
            "process enumeration for measurement isolation stopped early: {end}"
        ));
    }
    Ok(processes)
}

pub(crate) fn process_executable(pid: u32) -> Option<PathBuf> {
    query_executable(pid).ok().flatten()
}

/// The image path of `pid`; `Ok(None)` when the process no longer exists.
fn query_executable(pid: u32) -> Result<Option<PathBuf>, String> {
    // SAFETY: OpenProcess receives a PID from a Toolhelp snapshot or HWND query.
    // The returned query-only handle is owned by the local guard when non-null.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };
    if process == 0 {
        let error = std::io::Error::last_os_error();
        // ERROR_INVALID_PARAMETER: no process has this ID any more.
        return if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER) {
            Ok(None)
        } else {
            Err(format!("cannot open the process: {error}"))
        };
    }
    let process = ProcessHandle(process);
    let mut buffer = vec![0_u16; MAX_WINDOWS_PATH_CHARS];
    let mut length = u32::try_from(buffer.len()).map_err(|error| error.to_string())?;
    // SAFETY: process is a live query handle; buffer is writable UTF-16 storage
    // whose capacity is supplied in length. The API retains neither pointer.
    if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &raw mut length) }
        == FALSE
    {
        return Err(format!(
            "cannot read the image path: {}",
            std::io::Error::last_os_error()
        ));
    }
    buffer.truncate(usize::try_from(length).map_err(|error| error.to_string())?);
    Ok(Some(PathBuf::from(String::from_utf16_lossy(&buffer))))
}

fn executable_name(buffer: &[u16]) -> String {
    let length = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..length])
}

struct SnapshotHandle(isize);

impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner of a successful Toolhelp snapshot.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct ProcessHandle(isize);

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: this is the sole owner of a successful OpenProcess handle.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    process_id: u32,
    default_heap_id: usize,
    module_id: u32,
    threads: u32,
    parent_process_id: u32,
    base_priority: i32,
    flags: u32,
    executable: [u16; 260],
}

#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> isize;
    fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
    fn OpenProcess(access: u32, inherit_handle: i32, process_id: u32) -> isize;
    fn QueryFullProcessImageNameW(
        process: isize,
        flags: u32,
        executable_name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

#[cfg(test)]
mod tests {

    /// Only a StickyMD this run owns may have its tray icon clicked: any other running copy
    /// (the USER's own note) is foreign, and the owned copies are not.
    #[test]
    fn running_copies_not_owned_by_the_run_are_foreign() {
        assert_eq!(
            super::foreign_stickymd(&[10, 20], &[10, 20]),
            Vec::<u32>::new()
        );
        assert_eq!(super::foreign_stickymd(&[10, 20, 30], &[10, 20]), vec![30]);
        assert_eq!(super::foreign_stickymd(&[30], &[10, 20]), vec![30]);
        assert_eq!(super::foreign_stickymd(&[], &[10]), Vec::<u32>::new());
    }

    use std::fs;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::sync::Mutex;
    use std::thread;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use super::{ChildGuard, ensure_no_stale_smoke_stickymd, is_smoke_owned_stickymd};

    const SENTINEL_ENV: &str = "STICKYMD_SMOKE_CHILD_GUARD_SENTINEL";
    const PREFLIGHT_CHILD_ENV: &str = "STICKYMD_SMOKE_PREFLIGHT_CHILD";
    static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn child_guard_reaps_process_when_post_spawn_step_fails() {
        if let Some(sentinel) = std::env::var_os(SENTINEL_ENV) {
            thread::sleep(Duration::from_millis(700));
            fs::write(sentinel, b"leaked").expect("write leak sentinel");
            return;
        }

        let root = unique_temp("child-guard");
        fs::create_dir(&root).expect("create child guard fixture");
        let sentinel = root.join("leaked.txt");
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command
            .args([
                "--exact",
                "managed_process::tests::child_guard_reaps_process_when_post_spawn_step_fails",
            ])
            .env(SENTINEL_ENV, &sentinel);
        let result: Result<(), String> = (|| {
            let _child = ChildGuard::spawn(&mut command, "start child guard regression helper")?;
            Err("injected post-spawn failure".to_owned())
        })();
        assert_eq!(result.unwrap_err(), "injected post-spawn failure");
        thread::sleep(Duration::from_millis(1_200));
        assert!(
            !sentinel.exists(),
            "post-spawn error leaked the child past its RAII scope"
        );
        fs::remove_dir_all(root).expect("remove child guard fixture");
    }

    #[test]
    fn only_known_smoke_temp_roots_are_classified_as_tool_owned() {
        let temporary = PathBuf::from(r"C:\Users\Tester\AppData\Local\Temp");
        for owned in [
            temporary.join(r"stickymd-smoke-42-99\preview-1\StickyMD.exe"),
            temporary.join(r"StickyMD-G4-42-99\case\stickymd.EXE"),
            temporary.join(r"stickymd-verify-42-99\runtime\ascii\StickyMD.exe"),
            temporary.join(r"stickymd-manual-sessions\42-99\instance-1\StickyMD.exe"),
            // Another terminal's TMP: the root is recognized wherever it lives.
            PathBuf::from(r"C:\Temp\Manual\stickymd-manual-sessions\1-2\instance-1\StickyMD.exe"),
            PathBuf::from(r"D:\scratch\stickymd-g5-7-8\case\StickyMD.exe"),
        ] {
            assert!(is_smoke_owned_stickymd(&owned), "{}", owned.display());
        }
        for user in [
            PathBuf::from(r"D:\Notes\Research\StickyMD.exe"),
            temporary.join(r"my-portable-note\StickyMD.exe"),
            temporary.join(r"stickymd-smoke-42-99\helper.exe"),
        ] {
            assert!(!is_smoke_owned_stickymd(&user), "{}", user.display());
        }
    }

    #[test]
    fn stale_smoke_preflight_fails_without_killing_the_existing_process() {
        if std::env::var_os(PREFLIGHT_CHILD_ENV).is_some() {
            thread::sleep(Duration::from_secs(10));
            return;
        }

        let _serial = PROCESS_TEST_LOCK.lock().expect("lock process test");
        for prefix in ["stickymd-smoke", "stickymd-verify", super::MANUAL_SESSIONS] {
            let root = unique_temp("preflight")
                .with_file_name(format!("{prefix}-{}-preflight", std::process::id()));
            assert_stale_preflight_preserves_child(&root, &root, &root);
        }
        // A recorder run with another TMP: its sessions root is not directly under ours.
        let other_tmp = unique_temp("preflight-other-tmp");
        let root = other_tmp.join(super::MANUAL_SESSIONS).join("1-2");
        assert_stale_preflight_preserves_child(&root, &root, &other_tmp);

        // Started through a junction whose own name is not a smoke root.
        let base = unique_temp("preflight-alias");
        let real = base.join("stickymd-smoke-alias-target");
        let alias = base.join("plain-alias");
        fs::create_dir_all(&real).expect("create alias target");
        let made = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:STICKY_LINK -Target $env:STICKY_TARGET -ErrorAction Stop | Out-Null",
            ])
            .env("STICKY_LINK", &alias)
            .env("STICKY_TARGET", &real)
            .output()
            .expect("create junction");
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        assert_stale_preflight_preserves_child(&real, &alias, &base);

        // Started through an 8.3 short path, when the volume generates short names.
        let base = unique_temp("preflight-short");
        let real = base.join("stickymd-smoke-long-directory-name");
        fs::create_dir_all(&real).expect("create short-name target");
        // `raw_arg`: cmd parses its own command line, not the C runtime's quoting.
        let short = Command::new("cmd")
            .args(["/d", "/c"])
            .raw_arg("for %I in (\"%STICKY_LONG%\") do @echo %~sI")
            .env("STICKY_LONG", &real)
            .output()
            .expect("query short path");
        assert!(
            short.status.success() && !short.stdout.is_empty(),
            "the short-name query failed: {}",
            String::from_utf8_lossy(&short.stderr)
        );
        let short = PathBuf::from(String::from_utf8_lossy(&short.stdout).trim());
        // The smoke directory itself must be shortened for this case to mean anything.
        if short
            .file_name()
            .is_some_and(|name| name.to_string_lossy().contains('~'))
        {
            assert!(!is_smoke_owned_stickymd(&short.join("StickyMD.exe")));
            eprintln!("8.3 case runs through {}", short.display());
            assert_stale_preflight_preserves_child(&real, &short, &base);
        } else {
            // Evidence runs set this so a skipped case cannot pass unnoticed.
            assert!(
                std::env::var_os("STICKYMD_REQUIRE_SHORT_NAMES").is_none(),
                "the volume did not shorten {}",
                real.display()
            );
            eprintln!(
                "8.3 case skipped: the volume did not shorten {}",
                real.display()
            );
            fs::remove_dir_all(&base).expect("remove short-name fixture");
        }
    }

    /// Copies the test executable to `<root>/StickyMD.exe`, runs it as
    /// `<launch>/StickyMD.exe` (the same file under another name, or the same path), checks
    /// the preflight names it without terminating it, then removes `cleanup`.
    fn assert_stale_preflight_preserves_child(root: &Path, launch: &Path, cleanup: &Path) {
        fs::create_dir_all(root).expect("create preflight fixture");
        fs::copy(
            std::env::current_exe().expect("current test executable"),
            root.join("StickyMD.exe"),
        )
        .expect("copy preflight child executable");
        let executable = launch.join("StickyMD.exe");
        let mut child = Command::new(&executable)
            .args([
                "--exact",
                "managed_process::tests::stale_smoke_preflight_fails_without_killing_the_existing_process",
            ])
            .env(PREFLIGHT_CHILD_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start preflight child");
        let pid = child.id();
        eprintln!(
            "launched {}; Windows reports {:?}",
            executable.display(),
            super::query_executable(pid)
        );

        let deadline = Instant::now() + Duration::from_secs(5);
        // Classified as stale (not merely unconfirmed), with exactly this PID listed.
        let listed = |error: &str| {
            error.starts_with("stale smoke-owned")
                && error
                    .split_once("pids=[")
                    .and_then(|(_, rest)| rest.split_once(']'))
                    .is_some_and(|(pids, _)| {
                        pids.split(", ").any(|listed| listed == pid.to_string())
                    })
        };
        let error = loop {
            match ensure_no_stale_smoke_stickymd() {
                Err(error) if listed(&error) => break error,
                Ok(()) | Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(25));
                }
                outcome => panic!("preflight did not identify pid {pid}: {outcome:?}"),
            }
        };
        assert!(error.contains("will not terminate pre-existing processes"));
        assert!(
            child.try_wait().expect("inspect preflight child").is_none(),
            "preflight must observe but never terminate a pre-existing process"
        );

        child
            .kill()
            .expect("terminate preflight child after assertion");
        child.wait().expect("reap preflight child after assertion");
        remove_after_exit(cleanup);
    }

    /// Removes a fixture holding the executable of a child that was just reaped. Windows
    /// can keep that image (or a scan of the fresh copy) open for a moment after the
    /// process has exited, so the removal is retried within a bound and still fails if it
    /// never succeeds.
    fn remove_after_exit(fixture: &Path) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut delay = Duration::from_millis(25);
        let mut retried = false;
        loop {
            match fs::remove_dir_all(fixture) {
                Ok(()) => return,
                // A failed attempt may still have removed the directory itself.
                Err(error) if retried && error.kind() == std::io::ErrorKind::NotFound => return,
                Err(error) if Instant::now() >= deadline => {
                    panic!("remove preflight fixture {}: {error}", fixture.display())
                }
                Err(_) => thread::sleep(delay),
            }
            retried = true;
            delay = (delay * 2).min(Duration::from_secs(1));
        }
    }

    fn unique_temp(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "stickymd-managed-process-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    use std::path::{Path, PathBuf};
}
