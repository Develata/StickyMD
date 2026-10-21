//! The StickyMD instances a recorder session observes: the verified candidate extracted
//! into a session directory, a fresh copy seeded and started per scenario, and cleanup.
//!
//! - Session directories live under `<temp>/stickymd-manual-sessions/`, each named
//!   `<pid>-<nanos>` beside a `<name>.lock` its session holds. Creating a session and
//!   sweeping stale ones both happen under the root's own lock, so a sweep never sees a
//!   session between creating its lock file and taking it. A sweep removes only entries
//!   with the session name pattern whose lock it can take (or that have no lock file),
//!   holding that lock while it removes them; anything it cannot open or judge stays.
//! - Each instance starts inside a job that kills it when the recorder exits, by any route
//!   (`managed_process::Job`: created suspended, joined, resumed). Processes StickyMD starts
//!   itself (a browser opened from a link) break away from the job. A forced termination
//!   between creation and joining leaves a suspended instance that never ran; the next
//!   session refuses to start and names it as left by an interrupted recorder.
//! - The recorder refuses to start while another StickyMD runs and never stops a process it
//!   did not start.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::exact_desktop::{
    self, ChildGuard, copy_directory, expand_zip, running_stickymd_processes,
};
use super::super::file_lock::sidecar;
use super::super::manual_observation::log::Instance;
use super::super::manual_observation::registry::Fixture;
use super::super::receipt::{self, Candidate};
use super::fixture;
use super::flow::Tested;
use crate::managed_process::{Descendants, Job, MANUAL_SESSIONS as SESSIONS, STOP_WAIT};

/// The root's lock; its name never matches the session name pattern.
const ROOT_LOCK: &str = "sessions.lock";

/// The extracted candidate and the job its instances run in.
pub(super) struct Workshop {
    // Declared first so it closes first: anything still running stops before the
    // directory is removed.
    job: Job,
    template: PathBuf,
    exe_sha256: String,
    launches: u32,
    directory: SessionDirectory,
}

impl Workshop {
    /// Verifies `zip` and its EXE against the candidate and extracts it.
    pub(super) fn prepare(zip: &Path, candidate: &Candidate) -> Result<Self, String> {
        refuse_other_instances()?;
        let directory = SessionDirectory::create()?;
        if receipt::sha256(zip)? != candidate.zip_sha256 {
            return Err(format!(
                "{} does not match the candidate ZIP {}",
                zip.display(),
                candidate.zip_sha256
            ));
        }
        let extracted = directory.path.join("template");
        expand_zip(zip, &extracted, "manual recorder")?;
        let template = extracted.join("StickyMD");
        if receipt::sha256(&template.join("StickyMD.exe"))? != candidate.exe_sha256 {
            return Err("the extracted StickyMD.exe does not match the candidate EXE".to_owned());
        }
        Ok(Self {
            job: Job::create(Descendants::BreakAway)?,
            template,
            exe_sha256: candidate.exe_sha256.clone(),
            launches: 0,
            directory,
        })
    }

    /// Copies the candidate, seeds `fixture` and starts it; the copy's EXE is hashed
    /// again, so the recorded digest is measured, not assumed.
    pub(super) fn launch(&mut self, fixture: Fixture) -> Result<Launched, String> {
        refuse_other_instances()?;
        self.launches += 1;
        // Owns the copy until an instance runs from it; any failure below removes it.
        let mut copy = Copy {
            path: self
                .directory
                .path
                .join(format!("instance-{}", self.launches)),
            armed: true,
        };
        copy_directory(&self.template, &copy.path)?;
        let (child, instance, startup_error) = start(
            &self.job,
            &copy.path,
            fixture,
            &self.exe_sha256,
            exact_desktop::TIMEOUT,
        )?;
        copy.armed = false;
        Ok(Launched {
            child,
            program: copy.path.clone(),
            instance,
            startup_error,
        })
    }
}

/// A copied program directory, removed on drop while `armed`.
struct Copy {
    path: PathBuf,
    armed: bool,
}

impl Drop for Copy {
    fn drop(&mut self) {
        if self.armed {
            remove_owned(&self.path, "instance copy");
        }
    }
}

/// Seeds the copy, checks its EXE and starts it in `job`. An error means the preparation
/// failed and nothing runs. Once the candidate runs, a failure to initialize (no note and
/// configuration within `startup_timeout`, the layout timeout in a session) is returned
/// with the instance: it may be the product failing, which only the observer can record.
fn start(
    job: &Job,
    program: &Path,
    fixture: Fixture,
    expected_sha256: &str,
    startup_timeout: Duration,
) -> Result<(ChildGuard, Instance, Option<String>), String> {
    fixture::seed(program, fixture)?;
    let executable = program.join("StickyMD.exe");
    let exe_sha256 = receipt::sha256(&executable)?;
    if exe_sha256 != expected_sha256 {
        return Err("the copied StickyMD.exe does not match the candidate EXE".to_owned());
    }
    // Joined before it runs: a failure here means the candidate never ran.
    let mut child = job.start(&executable)?;
    let instance = Instance {
        exe_sha256,
        pid: child.id(),
    };
    let startup_error = wait_for_startup(&mut child, program, startup_timeout).err();
    Ok((child, instance, startup_error))
}

/// Waits until the started candidate has its note and configuration, as the exact harness
/// does, but stops as soon as the process exits.
fn wait_for_startup(
    child: &mut ChildGuard,
    program: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if program.join("note/note.md").is_file() && program.join("note/config.toml").is_file() {
            return Ok(());
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot inspect StickyMD: {error}"))?
        {
            return Err(format!(
                "StickyMD exited ({status}) before writing its note and configuration"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "StickyMD did not write its note and configuration within {timeout:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A started StickyMD; dropping it stops the process and removes its copy.
pub(super) struct Launched {
    child: ChildGuard,
    program: PathBuf,
    instance: Instance,
    startup_error: Option<String>,
}

impl Tested for Launched {
    fn instance(&self) -> &Instance {
        &self.instance
    }

    fn startup_error(&self) -> Option<&str> {
        self.startup_error.as_deref()
    }

    fn still_sole(&mut self) -> Result<bool, String> {
        Ok(self.child.is_running()? && running_stickymd_processes()? == [self.instance.pid])
    }
}

impl Drop for Launched {
    fn drop(&mut self) {
        // Stop the process before removing the files it holds open.
        if let Err(error) = self.child.stop_within(STOP_WAIT) {
            eprintln!(
                "warning: cannot stop StickyMD PID {}: {error}; close it from its tray icon",
                self.instance.pid
            );
        }
        remove_owned(&self.program, "instance copy");
    }
}

/// Removes a directory this session created, reporting (not hiding) a failure.
fn remove_owned(path: &Path, what: &str) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!(
            "warning: cannot remove the {what} {}: {error}; a later recorder session removes it",
            path.display()
        ),
    }
}

fn refuse_other_instances() -> Result<(), String> {
    let running = running_stickymd_processes()?;
    if running.is_empty() {
        return Ok(());
    }
    let described = running
        .iter()
        .map(|pid| {
            let left_behind =
                crate::managed_process::process_executable(*pid).is_some_and(|path| {
                    path.to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&format!("\\{SESSIONS}\\"))
                });
            if left_behind {
                format!("{pid} (left by an interrupted recorder session)")
            } else {
                pid.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "another StickyMD is running (PID {described}); exit it from its tray icon first, the recorder does not stop processes it did not start"
    ))
}

/// `<pid>-<nanos>`: the only names a sweep may remove.
fn is_session_name(name: &str) -> bool {
    name.split_once('-').is_some_and(|(pid, nanos)| {
        [pid, nanos]
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    })
}

/// The sessions root, refused when it is a link: a sweep must stay inside it.
fn sessions_root(temp: &Path) -> Result<PathBuf, String> {
    let root = temp.join(SESSIONS);
    fs::create_dir_all(&root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let metadata = fs::symlink_metadata(&root)
        .map_err(|error| format!("cannot inspect {}: {error}", root.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{} is not a plain directory; the recorder will not create or sweep sessions through it",
            root.display()
        ));
    }
    Ok(root)
}

/// `<root>/<name>/`, owned while `<name>.lock` is held.
struct SessionDirectory {
    root: PathBuf,
    path: PathBuf,
    lock_path: PathBuf,
    lock: Option<File>,
}

impl SessionDirectory {
    fn create() -> Result<Self, String> {
        let root = sessions_root(&std::env::temp_dir())?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
            .as_nanos();
        Self::create_in(&root, &format!("{}-{nanos}", std::process::id()))
    }

    fn create_in(root: &Path, name: &str) -> Result<Self, String> {
        let root_lock = sidecar(&root.join(ROOT_LOCK), true, "manual recorder sessions")?;
        let warnings = sweep(root);
        let lock_path = root.join(format!("{name}.lock"));
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|error| format!("cannot create {}: {error}", lock_path.display()))?;
        lock.try_lock()
            .map_err(|error| format!("cannot lock {}: {error}", lock_path.display()))?;
        let path = root.join(name);
        fs::create_dir(&path)
            .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
        // Reported after the root lock is released, so output never holds it.
        drop(root_lock);
        for warning in warnings {
            eprintln!("warning: {warning}");
        }
        Ok(Self {
            root: root.to_path_buf(),
            path,
            lock_path,
            lock: Some(lock),
        })
    }
}

impl Drop for SessionDirectory {
    fn drop(&mut self) {
        // Under the root lock like creation and sweeping; without it the session is left,
        // its own lock released, for a later sweep.
        let root_lock = match sidecar(&self.root.join(ROOT_LOCK), true, "manual recorder sessions")
        {
            Ok(guard) => guard,
            Err(error) => {
                eprintln!(
                    "warning: {error}; {} is left for a later recorder session",
                    self.path.display()
                );
                return;
            }
        };
        // Removed within the sweep's bounds while the session's lock is held, so no sweep
        // judges it meanwhile. Left over, it keeps its lock file and is stale next time.
        let mut removals = MAX_REMOVALS;
        let warning = match remove_within(&self.path, &mut removals, Instant::now() + SWEEP_BUDGET)
        {
            Ok(true) => {
                drop(self.lock.take());
                // A sweep may remove the unlocked file first; that is the same outcome.
                match fs::remove_file(&self.lock_path) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => Some(format!(
                        "cannot remove {}: {error}; a later recorder session removes it",
                        self.lock_path.display()
                    )),
                    _ => None,
                }
            }
            Ok(false) => Some(format!(
                "{} is partly removed; a later recorder session continues",
                self.path.display()
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => Some(format!(
                "cannot remove the recorder session directory {}: {error}; a later recorder session removes it",
                self.path.display()
            )),
        };
        drop(self.lock.take());
        drop(root_lock);
        if let Some(warning) = warning {
            eprintln!("warning: {warning}");
        }
    }
}

/// What one sweep may spend, at most: root entries listed, stale sessions removed, file
/// system removals, time, and warnings kept (the rest are counted). Whatever is left
/// waits for a later session, so a backlog cannot hold the root lock for long; a session
/// that runs out of budget mid-removal keeps its lock file and stays stale.
const MAX_LISTED: usize = 1024;
const MAX_SWEPT: usize = 16;
const MAX_REMOVALS: usize = 20_000;
const SWEEP_BUDGET: Duration = Duration::from_secs(15);
const MAX_WARNINGS: usize = 16;

/// Collects warnings up to `MAX_WARNINGS`, counting the rest.
#[derive(Default)]
struct Warnings {
    kept: Vec<String>,
    omitted: usize,
}

impl Warnings {
    fn push(&mut self, warning: String) {
        if self.kept.len() < MAX_WARNINGS {
            self.kept.push(warning);
        } else {
            self.omitted += 1;
        }
    }

    fn finish(mut self) -> Vec<String> {
        if self.omitted > 0 {
            self.kept.push(format!("and {} more", self.omitted));
        }
        self.kept
    }
}

/// Removes `directory` entry by entry, spending one unit of `removals` per file system
/// removal and stopping at `deadline`. Returns whether the directory is gone; when the
/// budget or time runs out, what remains stays for a later sweep. Links (including
/// junctions) are removed themselves, never followed.
fn remove_within(
    directory: &Path,
    removals: &mut usize,
    deadline: Instant,
) -> std::io::Result<bool> {
    let mut stack = vec![directory.to_path_buf()];
    while let Some(current) = stack.last().cloned() {
        let mut descend = None;
        for entry in fs::read_dir(&current)? {
            if *removals == 0 || Instant::now() >= deadline {
                return Ok(false);
            }
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                descend = Some(entry.path());
                break;
            }
            let path = entry.path();
            // A directory link is removed with `remove_dir` on Windows; that is a second
            // removal, checked against the same budget and deadline.
            if fs::remove_file(&path).is_err() {
                *removals -= 1;
                if *removals == 0 || Instant::now() >= deadline {
                    return Ok(false);
                }
                fs::remove_dir(&path)?;
            }
            *removals -= 1;
        }
        match descend {
            Some(child) => stack.push(child),
            None => {
                if *removals == 0 || Instant::now() >= deadline {
                    return Ok(false);
                }
                fs::remove_dir(&current)?;
                *removals -= 1;
                stack.pop();
            }
        }
    }
    Ok(true)
}

/// Removes sessions that no longer hold their lock, within the bounds above. The caller
/// holds the root lock, so no session is being created meanwhile: a session directory
/// without a lock file is the remainder of a failed removal. Returns what it left in doubt,
/// could not remove or left for later; never fails, a later session retries.
fn sweep(root: &Path) -> Vec<String> {
    let deadline = Instant::now() + SWEEP_BUDGET;
    let out_of_time = || Instant::now() >= deadline;
    let mut removals = MAX_REMOVALS;
    let mut warnings = Warnings::default();
    let mut entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => return vec![format!("cannot list {}: {error}", root.display())],
    };
    let mut names = std::collections::BTreeSet::new();
    let mut listed = 0;
    while listed < MAX_LISTED && !out_of_time() {
        let Some(entry) = entries.next() else {
            break;
        };
        listed += 1;
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                warnings.push(format!(
                    "cannot list an entry of {}: {error}",
                    root.display()
                ));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let session = name.strip_suffix(".lock").unwrap_or(&name);
        if is_session_name(session) {
            names.insert(session.to_owned());
        }
    }
    if listed == MAX_LISTED || out_of_time() {
        warnings.push(format!(
            "listed part of {}; the rest waits for a later session",
            root.display()
        ));
    }
    let candidates = names.len();
    let mut swept = 0;
    for (judged, name) in names.into_iter().enumerate() {
        if swept == MAX_SWEPT || removals == 0 || out_of_time() {
            warnings.push(format!(
                "{} recorder sessions left for a later sweep",
                candidates - judged
            ));
            break;
        }
        let directory = root.join(&name);
        let lock_path = root.join(format!("{name}.lock"));
        let held = match OpenOptions::new().read(true).write(true).open(&lock_path) {
            Ok(file) => match file.try_lock() {
                Ok(()) => Some(file),
                // A live session holds it.
                Err(std::fs::TryLockError::WouldBlock) => continue,
                Err(std::fs::TryLockError::Error(error)) => {
                    warnings.push(format!("left {name}: cannot lock its lock file: {error}"));
                    continue;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                warnings.push(format!("left {name}: cannot open its lock file: {error}"));
                continue;
            }
        };
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                match remove_within(&directory, &mut removals, deadline) {
                    Ok(true) => {}
                    Ok(false) => {
                        // The lock file stays, so the session is still stale next time.
                        warnings.push(format!(
                            "{} is partly removed; a later sweep continues",
                            directory.display()
                        ));
                        continue;
                    }
                    Err(error) => {
                        warnings.push(format!(
                            "cannot remove stale recorder directory {}: {error}",
                            directory.display()
                        ));
                        continue;
                    }
                }
            }
            Ok(_) => {
                warnings.push(format!("left {name}: not a plain directory"));
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                warnings.push(format!("left {name}: cannot inspect it: {error}"));
                continue;
            }
        }
        swept += 1;
        if let Some(lock) = held {
            drop(lock);
            removals = removals.saturating_sub(1);
            if let Err(error) = fs::remove_file(&lock_path) {
                warnings.push(format!(
                    "cannot remove stale {}: {error}",
                    lock_path.display()
                ));
            }
        }
    }
    warnings.finish()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::{
        ChildGuard, Descendants, Fixture, Job, SessionDirectory, is_session_name, sessions_root,
        sweep,
    };

    const JOB_CHILD_SENTINEL: &str = "STICKYMD_MANUAL_JOB_CHILD";

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("stickymd-manual-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            // Created exclusively by this test under the temp directory.
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn session(root: &Path, name: &str, lock: bool) {
        fs::create_dir_all(root.join(name).join("instance-1")).unwrap();
        if lock {
            fs::write(root.join(format!("{name}.lock")), b"").unwrap();
        }
    }

    /// A stale session (lock free or lock file gone) is removed; a live one and anything
    /// not named like a session stay.
    #[test]
    fn sweeping_removes_only_stale_sessions() {
        let scratch = Scratch::new("sweep");
        let root = &scratch.0;
        session(root, "11-1", true);
        session(root, "22-2", true);
        session(root, "33-3", false);
        fs::create_dir_all(root.join("backup/keep")).unwrap();
        fs::write(root.join("notes.lock"), b"").unwrap();
        let held = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("22-2.lock"))
            .unwrap();
        held.lock().unwrap();
        assert!(sweep(root).is_empty());
        assert!(!root.join("11-1").exists() && !root.join("11-1.lock").exists());
        assert!(!root.join("33-3").exists());
        assert!(root.join("22-2/instance-1").is_dir() && root.join("22-2.lock").exists());
        assert!(root.join("backup/keep").is_dir() && root.join("notes.lock").exists());
        drop(held);
    }

    /// A large session directory is removed within the removal budget: what is left stays
    /// for later, and the next call continues.
    #[test]
    fn a_large_directory_is_removed_within_its_budget() {
        let scratch = Scratch::new("large-remove");
        let target = scratch.0.join("11-1");
        for directory in ["a", "a/b", "c"] {
            fs::create_dir_all(target.join(directory)).unwrap();
        }
        for index in 0..30 {
            fs::write(target.join(format!("a/b/{index}.txt")), b"x").unwrap();
            fs::write(target.join(format!("c/{index}.txt")), b"x").unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut budget = 10;
        assert!(!super::remove_within(&target, &mut budget, deadline).unwrap());
        assert_eq!(budget, 0);
        assert!(target.is_dir());
        let mut budget = 1000;
        assert!(super::remove_within(&target, &mut budget, deadline).unwrap());
        assert!(!target.exists());
        // 60 files and 4 directories, of which 10 were removed the first time.
        assert_eq!(1000 - budget, 64 - 10);
    }

    /// A backlog is removed a bounded batch at a time; the remainder is reported and left.
    #[test]
    fn a_backlog_is_swept_in_bounded_batches() {
        let scratch = Scratch::new("backlog");
        let root = &scratch.0;
        for index in 0..super::MAX_SWEPT + 4 {
            session(root, &format!("9-{index}"), true);
        }
        let warnings = sweep(root);
        assert_eq!(warnings, ["4 recorder sessions left for a later sweep"]);
        let left = || {
            fs::read_dir(root)
                .unwrap()
                .filter(|entry| entry.as_ref().unwrap().path().is_dir())
                .count()
        };
        assert_eq!(left(), 4);
        assert!(sweep(root).is_empty());
        assert_eq!(left(), 0);
    }

    #[test]
    fn a_session_is_created_under_the_root_lock_and_removed_on_drop() {
        let scratch = Scratch::new("session");
        let root = sessions_root(&scratch.0).unwrap();
        session(&root, "44-4", true);
        let created = SessionDirectory::create_in(&root, "55-5").unwrap();
        assert!(!root.join("44-4").exists(), "the stale session was swept");
        assert!(created.path.is_dir());
        // Its own lock is held, so another sweep leaves it alone.
        assert!(sweep(&root).is_empty());
        assert!(created.path.is_dir());
        drop(created);
        assert!(!root.join("55-5").exists() && !root.join("55-5.lock").exists());
        assert!(root.join("sessions.lock").exists());
        assert!(!is_session_name("sessions"));
        for name in ["1-2", "123-456"] {
            assert!(is_session_name(name), "{name}");
        }
        for name in ["backup", "1-", "-2", "1-2-3", "a-1", "1_2"] {
            assert!(!is_session_name(name), "{name}");
        }
    }

    /// Creating and removing a session both wait for the root lock, so neither interleaves
    /// with a sweep or with each other.
    #[test]
    fn sessions_are_created_and_removed_under_the_root_lock() {
        let scratch = Scratch::new("root-lock");
        let root = sessions_root(&scratch.0).unwrap();
        let held = super::sidecar(&root.join(super::ROOT_LOCK), true, "test").unwrap();
        let (started, waiting) = std::sync::mpsc::channel();
        let creator = {
            let root = root.clone();
            std::thread::spawn(move || {
                started.send(()).unwrap();
                SessionDirectory::create_in(&root, "77-7")
            })
        };
        waiting.recv().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            !root.join("77-7.lock").exists() && !root.join("77-7").exists(),
            "created while the root was locked"
        );
        drop(held);
        let created = creator.join().unwrap().unwrap();
        assert!(created.path.is_dir());

        let held = super::sidecar(&root.join(super::ROOT_LOCK), true, "test").unwrap();
        let (started, waiting) = std::sync::mpsc::channel();
        let remover = std::thread::spawn(move || {
            started.send(()).unwrap();
            drop(created);
        });
        waiting.recv().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            root.join("77-7").is_dir(),
            "removed while the root was locked"
        );
        drop(held);
        remover.join().unwrap();
        assert!(!root.join("77-7").exists() && !root.join("77-7.lock").exists());
    }

    /// A candidate that exits at once is reported as a startup failure of a started
    /// instance, with its PID and measured digest, not as a preparation error.
    #[test]
    fn a_candidate_that_exits_at_once_is_a_startup_failure() {
        let scratch = Scratch::new("quick-exit");
        let program = scratch.0.join("program");
        fs::create_dir_all(&program).unwrap();
        let system = std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        // `hostname` prints a line and exits: a stand-in that never writes a configuration.
        fs::copy(
            system.join(r"System32\hostname.exe"),
            program.join("StickyMD.exe"),
        )
        .unwrap();
        let digest = crate::qualification::receipt::sha256(&program.join("StickyMD.exe")).unwrap();
        // Far beyond any start and exit on a loaded machine, so returning before it shows
        // the wait ended when the process exited rather than at its deadline.
        const STARTUP_TIMEOUT: Duration = Duration::from_secs(120);
        let job = Job::create(Descendants::BreakAway).unwrap();
        let started = std::time::Instant::now();
        let (mut child, instance, startup_error) =
            super::start(&job, &program, Fixture::Blank, &digest, STARTUP_TIMEOUT).unwrap();
        assert_eq!(
            (instance.exe_sha256.as_str(), instance.pid),
            (digest.as_str(), child.id())
        );
        let error = startup_error.expect("the stand-in never initializes");
        assert!(error.contains("exited"), "{error}");
        assert!(
            started.elapsed() < STARTUP_TIMEOUT,
            "waited for the full timeout"
        );
        assert!(!child.is_running().unwrap());
    }

    /// A sessions root that is a junction is refused, so a sweep cannot reach its target.
    #[test]
    fn a_linked_sessions_root_is_refused() {
        let scratch = Scratch::new("linked-root");
        let target = scratch.0.join("elsewhere");
        fs::create_dir_all(target.join("66-6")).unwrap();
        let temp = scratch.0.join("temp");
        fs::create_dir_all(&temp).unwrap();
        let link = temp.join(super::SESSIONS);
        // A junction needs no symlink privilege. Paths travel in the environment.
        let made = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "New-Item -ItemType Junction -Path $env:STICKY_LINK -Target $env:STICKY_TARGET -ErrorAction Stop | Out-Null",
            ])
            .env("STICKY_LINK", &link)
            .env("STICKY_TARGET", &target)
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let error = sessions_root(&temp).unwrap_err();
        assert!(error.contains("not a plain directory"), "{error}");
        assert!(target.join("66-6").is_dir());
        // Remove the junction itself, not its target.
        fs::remove_dir(&link).unwrap();
    }

    /// Closing the job's handle kills a process that joined it.
    #[test]
    fn closing_the_job_stops_its_instance() {
        if std::env::var_os(JOB_CHILD_SENTINEL).is_some() {
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "qualification::manual::instance::tests::closing_the_job_stops_its_instance",
            ])
            .env(JOB_CHILD_SENTINEL, "1");
        let mut child = ChildGuard::spawn(&mut command, "cannot start the job test child").unwrap();
        let job = Job::create(Descendants::BreakAway).unwrap();
        job.assign(&child).unwrap();
        assert!(child.is_running().unwrap());
        drop(job);
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.is_running().unwrap() {
            assert!(
                Instant::now() < deadline,
                "the job did not stop its process"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
