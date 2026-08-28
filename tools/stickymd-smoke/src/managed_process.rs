//! Smoke-owned GUI process lifecycle and measurement-isolation checks.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::mem;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const FALSE: i32 = 0;
const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
const INVALID_SNAPSHOT_HANDLE: isize = -1;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x0000_1000;
const MAX_WINDOWS_PATH_CHARS: usize = 32_768;

/// Owns one smoke-started GUI process and reaps it when the scope ends.
pub(crate) struct ChildGuard(Child);

impl ChildGuard {
    pub(crate) fn start(executable: &Path) -> Result<Self, String> {
        let parent = executable
            .parent()
            .ok_or_else(|| format!("{} has no parent", executable.display()))?;
        let mut command = Command::new(executable);
        command
            .current_dir(parent)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        Self::spawn(
            &mut command,
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

    pub(crate) fn kill_and_wait(&mut self) -> Result<(), String> {
        if self.is_running()? {
            self.0
                .kill()
                .map_err(|error| format!("cannot terminate StickyMD child: {error}"))?;
        }
        self.0
            .wait()
            .map(|_| ())
            .map_err(|error| format!("cannot wait for StickyMD child: {error}"))
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
        match self.0.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProcessSnapshot {
    pid: u32,
    executable: PathBuf,
}

/// Rejects a performance/resource measurement contaminated by an older smoke child.
pub(crate) fn ensure_no_stale_smoke_stickymd() -> Result<(), String> {
    let temporary = std::env::temp_dir();
    let mut stale = running_stickymd_processes()?
        .into_iter()
        .filter(|process| is_smoke_owned_stickymd(&temporary, &process.executable))
        .map(|process| process.pid)
        .collect::<Vec<_>>();
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

fn is_smoke_owned_stickymd(temporary: &Path, executable: &Path) -> bool {
    if !executable
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("StickyMD.exe"))
    {
        return false;
    }
    let Some(relative) = strip_prefix_case_insensitive(executable, temporary) else {
        return false;
    };
    let Some(root_name) = relative.components().next() else {
        return false;
    };
    let root_name = root_name.as_os_str().to_string_lossy().to_ascii_lowercase();
    [
        "stickymd-smoke-",
        "stickymd-g3-",
        "stickymd-g4-",
        "stickymd-g5-",
        "stickymd-downloaded-smoke-",
    ]
    .iter()
    .any(|prefix| root_name.starts_with(prefix))
}

fn strip_prefix_case_insensitive(path: &Path, prefix: &Path) -> Option<PathBuf> {
    let mut path_components = path.components();
    for expected in prefix.components() {
        let actual = path_components.next()?;
        if !actual
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
        {
            return None;
        }
    }
    Some(path_components.collect())
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
        if executable_name(&entry.executable).eq_ignore_ascii_case("StickyMD.exe")
            && let Some(executable) = process_executable(entry.process_id)
        {
            processes.push(ProcessSnapshot {
                pid: entry.process_id,
                executable,
            });
        }
        // SAFETY: snapshot and entry remain valid across the enumeration loop.
        available = unsafe { Process32NextW(snapshot.0, &raw mut entry) } != FALSE;
    }
    Ok(processes)
}

fn process_executable(pid: u32) -> Option<PathBuf> {
    // SAFETY: OpenProcess receives a PID from the current Toolhelp snapshot.
    // The returned query-only handle is owned by the local guard when non-null.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid) };
    if process == 0 {
        return None;
    }
    let process = ProcessHandle(process);
    let mut buffer = vec![0_u16; MAX_WINDOWS_PATH_CHARS];
    let mut length = u32::try_from(buffer.len()).ok()?;
    // SAFETY: process is a live query handle; buffer is writable UTF-16 storage
    // whose capacity is supplied in length. The API retains neither pointer.
    if unsafe { QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &raw mut length) }
        == FALSE
    {
        return None;
    }
    let length = usize::try_from(length).ok()?;
    buffer.truncate(length);
    Some(PathBuf::from(String::from_utf16_lossy(&buffer)))
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
    use std::fs;
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
        assert!(is_smoke_owned_stickymd(
            &temporary,
            &temporary.join(r"stickymd-smoke-42-99\preview-1\StickyMD.exe")
        ));
        assert!(is_smoke_owned_stickymd(
            &temporary,
            &temporary.join(r"StickyMD-G4-42-99\case\stickymd.EXE")
        ));
        assert!(!is_smoke_owned_stickymd(
            &temporary,
            Path::new(r"D:\Notes\Research\StickyMD.exe")
        ));
        assert!(!is_smoke_owned_stickymd(
            &temporary,
            &temporary.join(r"my-portable-note\StickyMD.exe")
        ));
    }

    #[test]
    fn stale_smoke_preflight_fails_without_killing_the_existing_process() {
        if std::env::var_os(PREFLIGHT_CHILD_ENV).is_some() {
            thread::sleep(Duration::from_secs(10));
            return;
        }

        let _serial = PROCESS_TEST_LOCK.lock().expect("lock process test");
        let root = unique_temp("preflight")
            .with_file_name(format!("stickymd-smoke-{}-preflight", std::process::id()));
        fs::create_dir(&root).expect("create preflight fixture");
        let executable = root.join("StickyMD.exe");
        fs::copy(
            std::env::current_exe().expect("current test executable"),
            &executable,
        )
        .expect("copy preflight child executable");
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

        let deadline = Instant::now() + Duration::from_secs(5);
        let error = loop {
            match ensure_no_stale_smoke_stickymd() {
                Err(error) if error.contains(&pid.to_string()) => break error,
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
        fs::remove_dir_all(root).expect("remove preflight fixture");
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
