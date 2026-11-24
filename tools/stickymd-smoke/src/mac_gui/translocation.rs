//! P16-R03: a package downloaded from the internet starts under App Translocation, shows the
//! product's own prompt, and exits after it without writing anything. The driver stands in for
//! the browser and for the USER's first-open approval: it takes the candidate ZIP given with
//! `--zip` (or, without one, zips the app under test; `staging`), marks the archive and every
//! extracted file with a quarantine attribute whose user-approved bit (0x40) is set, so macOS
//! skips its first-open confirmation (USER 2026-11-19, the acceptance ladder report). It opens the
//! copy through LaunchServices, as a double click does.
//!
//! The prompt is shown before the application has finished launching, so its Accessibility
//! requests do not complete (macOS 27.2 VM: `kAXErrorCannotComplete`) and it owns no AX window.
//! The prompt is therefore judged from what the system does report: a fully shown window of the
//! process (alpha at least 0.9, at least 200×150 pt) on screen at the modal-panel level or above,
//! and the message on its stderr, which the product writes just before showing the same text
//! (`fatal_startup`, one string for both); the window's image is kept with the evidence. The
//! message must be the translocation advice, not the advice for any other unwritable folder.
//! It is answered as the manual procedure does: LaunchServices is asked to open the copy again,
//! which activates the running instance; then, only if that same process still shows that same
//! window, Return is posted to that process alone, so no other application can receive it. The
//! exit status comes from runningboard's termination record for the PID.
//! A system that does not translocate, a prompt window not seen while the process runs, or a
//! prompt that could not be answered is NOT_TESTED; a FAIL needs an observed violation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::{Row, Verdict};

/// The flags field of the quarantine attribute the driver writes: 0x83 as a browser download
/// leaves it, plus the user-approved bit 0x40.
pub(super) const QUARANTINE_FLAGS: &str = "00c3";
/// The start of the product's prompt in a directory it cannot write.
pub(super) const PROMPT: &str = "当前目录不可写";
/// The advice to move the app out, which a translocated bundle must get: today's wording
/// (`apps/stickymd-mac/src/main.rs`) and the phrase Phase 18 keeps for every unwritable portable
/// bundle (`apps/stickymd-mac/src/paths.rs`, Phase 18 session 2026-11-19). The advice for a
/// folder a development build cannot write, which a translocated bundle must not get. A unit test
/// holds them to the product's sources.
pub(super) const TRANSLOCATED_ADVICE: [&str; 2] = [
    "请先用访达把 StickyMD.app 移到别处",
    "把整个 StickyMD 文件夹移到别处",
];
pub(super) const GENERIC_ADVICE: &str = "请将程序移动到有写权限的文件夹";
/// runningboard's termination record for an exit with status 1 (the prompt's `exit(1)`): the
/// third field is the raw wait status, `1 << 8`. A probe app in the macOS 27.2 VM exiting with 0
/// and with 1 gave `(0, 0, 0)` and `(0, 0, 256)` (2026-11-19).
pub(super) const EXIT_ONE: &str = "(0, 0, 256)";
/// The cause the prompt carries on a read-only translocation.
pub(super) const CAUSE: &str = "Read-only file system";
/// `NSModalPanelWindowLevel`: an alert run modally sits at this level or above.
pub(super) const MODAL_LEVEL: i32 = 8;

/// The checks R03 records, in this order; with `--zip`, [`ZIP_CHECK`] comes first. mac-ledger
/// records an R03 PASS only with exactly these (plan 11 #phase-16-macos-host).
pub(crate) const PRESET: &str = "approved quarantine preset (USER 2026-11-19)";
pub(crate) const TRANSLOCATED: &str = "started under App Translocation";
pub(crate) const PROMPTED: &str = "prompt asks to move the translocated app";
pub(crate) const EXITS: &str = "exits after the prompt is answered";
pub(crate) const STATUS: &str = "exit status 1";
pub(crate) const UNWRITTEN: &str = "nothing written beside the package";
pub(crate) const CHECKS: [&str; 6] = [PRESET, TRANSLOCATED, PROMPTED, EXITS, STATUS, UNWRITTEN];
/// Why equal snapshots are not yet "nothing written" (review #24 B9): they show the listed
/// attributes unchanged at the end, not that no write happened and was undone in between.
pub(crate) const WRITE_UNPROVEN: &str = "a write undone between the two snapshots, or a change to permissions or extended attributes, is not excluded (review #24 B9; awaiting Develata's decision)";
/// The release ZIP R03 opened as given (`mac-gui --zip`); its observation is `sha256=<digest>`.
pub(crate) const ZIP_CHECK: &str = "opened the candidate ZIP as given";

/// What the live run observed.
#[derive(Debug, Default)]
pub(super) struct Observed {
    /// The SHA-256 of the candidate ZIP the download directory was laid out from, when `--zip`
    /// gave one and staging matched it.
    pub zip: Option<String>,
    /// The quarantine attribute read back from the extracted bundle.
    pub quarantine: Option<String>,
    /// The executable path of the one StickyMD process LaunchServices started.
    pub path: Option<String>,
    /// The `mount` line of the bundle's translocation.
    pub mount: Option<String>,
    /// The process ended before a prompt window was seen.
    pub exited_early: bool,
    /// The on-screen modal-level window of the process, as seen.
    pub window: Option<String>,
    /// The process's stderr when the prompt window was seen (or when the wait ended).
    pub stderr: String,
    /// Why the process's stderr could not be read, when it could not.
    pub stderr_unreadable: Option<String>,
    /// Why Return was not posted, when the prompt was seen but could not be answered.
    pub unanswered: Option<String>,
    /// After Return: whether the process ended within the wait.
    pub exited: Option<bool>,
    /// runningboard's termination record for the PID, e.g. `(0, 0, 0)`.
    pub termination: Option<String>,
    /// The entries of the folder holding the bundle (where a portable `note/` would be
    /// written) before the start and after the exit.
    pub before: Vec<String>,
    pub after: Vec<String>,
}

fn record(row: &mut Row, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        row.skip(name, &detail);
    } else {
        row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// Where a `mount` line mounts its source: `<source> on <point> (<options>)`.
pub(super) fn mount_point(line: &str) -> Option<&str> {
    line.split_once(" on ")
        .and_then(|(_, rest)| rest.split_once(" ("))
        .map(|(point, _)| point)
}

/// Whether the prompt was shown: its window seen, and the message with its cause on stderr.
pub(super) fn prompted(o: &Observed) -> bool {
    o.window.is_some() && o.stderr.contains(PROMPT) && o.stderr.contains(CAUSE)
}

/// The R03 checks from what was observed. Each depends on the one before it: without the
/// preset attribute nothing is a download; without translocation the prompt is not owed.
pub(super) fn judge(o: &Observed, row: &mut Row) {
    if let Some(digest) = &o.zip {
        row.check(ZIP_CHECK, true, format!("sha256={digest}"));
    }
    let preset = o
        .quarantine
        .as_deref()
        .is_some_and(|q| q.starts_with(&format!("{QUARANTINE_FLAGS};")));
    record(
        row,
        PRESET,
        if preset {
            Verdict::Pass
        } else {
            Verdict::NotTested
        },
        format!("com.apple.quarantine={:?}", o.quarantine),
    );
    let translocated = o
        .mount
        .as_deref()
        .filter(|m| m.contains("nullfs") && m.contains("read-only"))
        .and_then(mount_point)
        .is_some_and(|point| {
            point.contains("/AppTranslocation/")
                && o.path
                    .as_deref()
                    .is_some_and(|p| p.starts_with(&format!("{point}/")))
        });
    // Whether macOS translocates is the system's decision; without it the row cannot be judged.
    record(
        row,
        TRANSLOCATED,
        if preset && translocated {
            Verdict::Pass
        } else {
            Verdict::NotTested
        },
        format!("path={:?}; mount={:?}", o.path, o.mount),
    );
    if !(preset && translocated) {
        for name in [PROMPTED, EXITS, STATUS, UNWRITTEN] {
            row.skip(name, "not started under App Translocation");
        }
        return;
    }
    let translocated_advice = TRANSLOCATED_ADVICE.iter().any(|a| o.stderr.contains(a));
    let generic_advice = o.stderr.contains(GENERIC_ADVICE);
    let seen = o.window.is_some() || o.exited_early;
    let prompt = if o.stderr_unreadable.is_some() {
        // Without the message, a seen window says nothing about the prompt's words.
        Verdict::NotTested
    } else if prompted(o) && translocated_advice {
        Verdict::Pass
    } else if generic_advice && seen {
        // The advice for any unwritable folder: moving the app out is not asked for.
        Verdict::Fail
    } else if prompted(o) {
        // The prompt, worded in a way this driver does not know: the product's text changed.
        Verdict::NotTested
    } else if o.exited_early || (o.window.is_some() && !o.stderr.contains(PROMPT)) {
        // Gone without a prompt, or a prompt about something else.
        Verdict::Fail
    } else {
        // Still running and no modal window seen: the prompt may be unobservable, not absent.
        Verdict::NotTested
    };
    record(
        row,
        PROMPTED,
        prompt,
        format!(
            "window={:?}; exited before a prompt {}; stderr={:?}",
            o.window, o.exited_early, o.stderr
        ),
    );
    let answered = prompt == Verdict::Pass && o.unanswered.is_none();
    record(
        row,
        EXITS,
        if !answered {
            Verdict::NotTested
        } else {
            // Return goes to the process with no receipt of which window took it, so a process
            // still running after it is not an observed refusal to exit.
            match o.exited {
                Some(true) => Verdict::Pass,
                Some(false) | None => Verdict::NotTested,
            }
        },
        format!(
            "not answered: {:?}; ended within the wait after Return: {:?}",
            o.unanswered, o.exited
        ),
    );
    record(
        row,
        STATUS,
        if !(answered && o.exited == Some(true)) {
            Verdict::NotTested
        } else {
            match o.termination.as_deref() {
                Some(t) if t == EXIT_ONE => Verdict::Pass,
                Some(_) => Verdict::Fail,
                None => Verdict::NotTested,
            }
        },
        format!("termination record {:?}", o.termination),
    );
    let ended = o.exited_early || o.exited == Some(true);
    record(
        row,
        UNWRITTEN,
        // Fail closed (review #24 B9): a difference is an observed write; equal snapshots do
        // not prove that nothing was written while the product ran.
        if !ended || o.before == o.after {
            Verdict::NotTested
        } else {
            Verdict::Fail
        },
        if o.before == o.after {
            format!(
                "{} entries under the folder holding the bundle unchanged (sizes, SHA-256, times), but {WRITE_UNPROVEN}",
                o.before.len()
            )
        } else {
            format!(
                "changed: {:?}",
                super::snapshot::changed(&o.before, &o.after)
            )
        },
    );
}

#[cfg(target_os = "macos")]
pub(super) use live::run;

#[cfg(target_os = "macos")]
mod live {
    use super::{MODAL_LEVEL, Observed, QUARANTINE_FLAGS, judge, mount_point, prompted};
    use crate::mac_gui::{
        live::{command, command_within},
        model::{Row, Trace, Window},
        native::{events, windows},
        staging::{self, Bytes, Candidate},
    };
    use std::{
        fs,
        path::Path,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    /// The virtual key code of Return.
    const RETURN: u16 = 36;

    /// The PIDs of processes named StickyMD with their executable paths.
    fn products() -> Result<Vec<(u32, String)>, String> {
        let found = match command("/usr/bin/pgrep", &["-x", "StickyMD"]) {
            Ok(text) => text,
            // pgrep exits 1 when nothing matches.
            Err(e) if e.contains("exit status: 1") => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for pid in found.split_whitespace() {
            let pid: u32 = pid.parse().map_err(|_| format!("pgrep printed {pid:?}"))?;
            // ps exits 1 for a PID that is gone; any other failure is not a finding about the
            // process and must not read as its exit (review #24 S1).
            match command("/bin/ps", &["-o", "comm=", "-p", &pid.to_string()]) {
                Ok(path) => out.push((pid, path.trim().to_owned())),
                Err(e) if e.contains("exit status: 1") => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// runningboard's termination record for `pid` (e.g. `(0, 0, 0)`), from the system log; the
    /// log is kept beside the stderr. The record can trail the exit, so the log is read up to
    /// three times.
    fn termination(pid: u32, dir: &Path) -> Result<Option<String>, String> {
        let marker = format!(":{pid}] termination reported by launchd ");
        for _ in 0..3 {
            events::wait(1000)?;
            let log = command_within(
                "/usr/bin/log",
                &[
                    "show",
                    "--last",
                    "1m",
                    "--style",
                    "syslog",
                    "--predicate",
                    "process == \"runningboardd\" AND eventMessage CONTAINS \"termination reported by launchd\"",
                ],
                std::time::Duration::from_secs(60),
            )?;
            if let Some(record) = log
                .lines()
                .find_map(|l| l.split(&marker).nth(1).map(|r| r.trim().to_owned()))
            {
                crate::atomic_evidence::write(&dir.join("system.log"), log.as_bytes())?;
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    /// From `open` to the answered prompt: finds the one process started from `copy`, waits for
    /// its prompt, keeps its image, and answers it. Errors here still leave the caller's cleanup
    /// to run.
    fn observe(
        o: &mut Observed,
        copy: &Path,
        dir: &Path,
        read_stderr: &dyn Fn() -> Result<String, String>,
        row: &mut Row,
        note: &dyn Fn(&mut Row, String),
    ) -> Result<(), String> {
        let text = |p: &Path| p.to_string_lossy().into_owned();
        // The process started from this copy: up to 20 s for LaunchServices, AMFI and TCC. Only
        // a process whose executable lies in this copy or in its translocated mount is it; any
        // other StickyMD is left alone.
        let start = Instant::now();
        let mut pid = None;
        while start.elapsed().as_secs() < 20 {
            let mounts = command("/sbin/mount", &[])?;
            o.mount = mounts
                .lines()
                .find(|l| l.starts_with(&format!("{} on ", copy.display())))
                .map(str::to_owned);
            let point = o.mount.as_deref().and_then(mount_point).map(str::to_owned);
            let found: Vec<_> = products()?
                .into_iter()
                .filter(|(_, path)| {
                    path.starts_with(&format!("{}/", text(copy)))
                        || point
                            .as_deref()
                            .is_some_and(|p| path.starts_with(&format!("{p}/")))
                })
                .collect();
            if let [(p, path)] = found.as_slice() {
                pid = Some(*p);
                o.path = Some(path.clone());
                break;
            }
            if found.len() > 1 {
                note(
                    row,
                    format!("more than one process from this copy: {found:?}"),
                );
                return Ok(());
            }
            events::wait(100)?;
        }
        let Some(pid) = pid else {
            note(row, "no StickyMD process from this copy within 20s".into());
            return Ok(());
        };
        note(row, format!("PID {pid} at {:?}", o.path));
        let alive = || products().map(|p| p.iter().any(|(q, _)| *q == pid));
        // The prompt: up to 15 s for its window and its message.
        let modal = |w: &Window| {
            // A fully shown alert, not one still fading or scaling in.
            w.onscreen
                && w.layer >= MODAL_LEVEL
                && w.alpha >= 0.9
                && w.bounds.size.width >= 200.0
                && w.bounds.size.height >= 150.0
        };
        let start = Instant::now();
        let mut shown = None;
        while start.elapsed().as_secs() < 15 {
            if !alive()? {
                o.exited_early = true;
                break;
            }
            shown = windows::all(pid)?.into_iter().find(|w| modal(w));
            o.window = shown.as_ref().map(|w| format!("{w:?}"));
            match read_stderr() {
                Ok(text) => o.stderr = text,
                Err(e) => o.stderr_unreadable = Some(e),
            }
            if prompted(o) {
                break;
            }
            events::wait(200)?;
        }
        match read_stderr() {
            Ok(text) => o.stderr = text,
            Err(e) => o.stderr_unreadable = Some(e),
        }
        note(
            row,
            format!("window {:?}; exited early {}", o.window, o.exited_early),
        );
        let (true, Some(window)) = (prompted(o), &shown) else {
            return Ok(());
        };
        match windows::screenshot(window) {
            Ok(ppm) => crate::atomic_evidence::write(&dir.join("prompt.ppm"), &ppm)?,
            Err(e) => note(row, format!("prompt image not captured: {e}")),
        }
        // Activate the running instance as the manual procedure does, then answer it, but only
        // while that same process still shows that same window.
        command("/usr/bin/open", &[&text(copy)])?;
        events::wait(1500)?;
        let others: Vec<_> = products()?.into_iter().filter(|(q, _)| *q != pid).collect();
        let same = windows::all(pid)
            .map(|all| all.iter().any(|w| w.id == window.id && modal(w)))
            .unwrap_or(false);
        o.unanswered = if !others.is_empty() {
            Some(format!("reopening started another process: {others:?}"))
        } else if !alive()? {
            Some("the process ended before Return was posted".into())
        } else if !same {
            Some(format!("window {} was gone before Return", window.id))
        } else {
            None
        };
        if o.unanswered.is_some() {
            return Ok(());
        }
        events::key_to_pid(pid, RETURN)?;
        let start = Instant::now();
        let mut ended = false;
        while start.elapsed().as_secs() < 5 {
            if !alive()? {
                ended = true;
                break;
            }
            events::wait(100)?;
        }
        o.exited = Some(ended);
        if ended {
            // An unreadable system log leaves the exit status untested, not the run failed.
            o.termination = match termination(pid, dir) {
                Ok(record) => record,
                Err(e) => {
                    note(row, format!("termination record unreadable: {e}"));
                    None
                }
            };
        }
        Ok(())
    }

    /// Runs R03 in `dir` (the scenario's session directory) for the app under test, whose byte
    /// identities must be `expected`, from the candidate ZIP `zip` when one was given. Leaves no
    /// process of its own running. `Ok(Some(reason))` stops the whole run unfinished: R03 could
    /// not run on the bytes under test (staging, or another StickyMD running), the bytes changed
    /// while it ran, or its processes or translocation outlived it. None of these is a product
    /// result, so none is recorded as a FAIL (review F9, F11).
    pub(in crate::mac_gui) fn run(
        app: &Path,
        dir: &Path,
        expected: &Bytes,
        zip: Option<&Candidate>,
        started: Instant,
        row: &mut Row,
    ) -> Result<Option<String>, String> {
        // Every error of R03 is the infrastructure's (opening, observing, cleaning up); a product
        // result is only ever a check. So any error stops the run unfinished, never a FAIL
        // (review #24 S1).
        Ok(attempt(app, dir, expected, zip, started, row)
            .unwrap_or_else(|error| Some(format!("R03 could not complete: {error}"))))
    }

    fn attempt(
        app: &Path,
        dir: &Path,
        expected: &Bytes,
        zip: Option<&Candidate>,
        started: Instant,
        row: &mut Row,
    ) -> Result<Option<String>, String> {
        let note = |row: &mut Row, event: String| {
            row.trace.push(Trace {
                ms: started.elapsed().as_millis() as u64,
                event,
            });
        };
        if !products()?.is_empty() {
            return Ok(Some(
                "a StickyMD process is already running; R03 was not started".into(),
            ));
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let value = format!("{QUARANTINE_FLAGS};{stamp:x};Safari;");
        let staged = match staging::stage(&dir.join("download"), app, expected, zip, &value) {
            Ok(staged) => staged,
            Err(e) => {
                return Ok(Some(format!(
                    "the download directory was not laid out from the bytes under test: {e}"
                )));
            }
        };
        note(
            row,
            match &staged.zip_sha256 {
                Some(digest) => format!("candidate ZIP sha256={digest}"),
                None => "no --zip: the app under test was zipped here; no ZIP is named".into(),
            },
        );
        let copy = staged.copy;
        // The folder holding the bundle: a portable note/ would be written there.
        let watched = staged.watched;
        let text = |p: &Path| p.to_string_lossy().into_owned();
        let mut o = Observed {
            zip: staged.zip_sha256.clone(),
            quarantine: command(
                "/usr/bin/xattr",
                &["-p", "com.apple.quarantine", &text(&copy)],
            )
            .ok()
            .map(|q| q.trim().to_owned()),
            before: super::super::snapshot::tree(&watched)?,
            ..Observed::default()
        };
        let stderr = dir.join("stderr.log");
        // Not yet written is empty; other read errors are kept. Bytes that are not UTF-8 are kept
        // (lossily), so a garbled message compares unequal rather than vanishing.
        let read_stderr = || match fs::read(&stderr) {
            Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("{}: {e}", stderr.display())),
        };
        note(
            row,
            format!("open {} (quarantine {:?})", copy.display(), o.quarantine),
        );
        command("/usr/bin/open", &[&text(&copy), "--stderr", &text(&stderr)])?;
        let observed = observe(&mut o, &copy, dir, &read_stderr, row, &note);
        // Leave nothing of this run running, on every path: only processes started from this
        // copy (the copy itself or its translocated mount), never another StickyMD.
        let mountpoint = o.mount.as_deref().and_then(mount_point).map(str::to_owned);
        let ours = |path: &str| {
            path.starts_with(&format!("{}/", text(&copy)))
                || mountpoint
                    .as_deref()
                    .is_some_and(|point| path.starts_with(&format!("{point}/")))
        };
        let own = || -> Result<Vec<u32>, String> {
            Ok(products()?
                .into_iter()
                .filter(|(_, path)| ours(path))
                .map(|(q, _)| q)
                .collect())
        };
        // Cleanup waits do not end on a cancelled run: the run is stopping anyway.
        for signal in ["-TERM", "-KILL"] {
            for q in own()? {
                let _ = command("/bin/kill", &[signal, &q.to_string()]);
            }
            let start = Instant::now();
            while !own()?.is_empty() && start.elapsed().as_secs() < 3 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        let left = own()?;
        if !left.is_empty() {
            return Ok(Some(format!(
                "this run's StickyMD processes did not end: {left:?}"
            )));
        }
        observed?;
        o.after = super::super::snapshot::tree(&watched)?;
        // macOS detaches the translocation after the process ends; wait for it before cleanup.
        let start = Instant::now();
        let mounted = || -> Result<bool, String> {
            Ok(command("/sbin/mount", &[])?
                .lines()
                .any(|l| l.starts_with(&format!("{} on ", copy.display()))))
        };
        while mounted()? && start.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        if mounted()? {
            return Ok(Some(
                "this copy's translocation was still mounted 10 s after its process ended".into(),
            ));
        }
        // The bundle opened by path is still the bytes under test (a change undone between the
        // two checks is not seen; the VM runs nothing else).
        if let Err(e) = staging::matches(&copy, expected) {
            return Ok(Some(format!("the copy changed while R03 ran: {e}")));
        }
        note(row, format!("after {} entries", o.after.len()));
        judge(&o, row);
        Ok(None)
    }
}
