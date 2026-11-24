//! R03's live run: lays out the download, watches and snapshots the folder holding the bundle,
//! opens it through LaunchServices, answers the prompt, and leaves nothing of its own running.
//! What it observes is judged by the parent module's pure rule.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    MODAL_LEVEL, Observed, QUARANTINE_FLAGS, REGISTERED, Watched, facts, judge, mount_point,
    prompted,
};
use crate::mac_gui::{
    live::{command, command_within},
    model::{Row, Trace, Window},
    native::{events, vnodes, windows},
    snapshot,
    staging::{self, Bytes, Candidate},
};
use std::{
    fs,
    io::Write,
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
    write_restore: bool,
    started: Instant,
    row: &mut Row,
) -> Result<Option<String>, String> {
    // Every error of R03 is the infrastructure's (opening, observing, cleaning up); a product
    // result is only ever a check. So any error stops the run unfinished, never a FAIL
    // (review #24 S1).
    Ok(
        attempt(app, dir, expected, zip, write_restore, started, row)
            .unwrap_or_else(|error| Some(format!("R03 could not complete: {error}"))),
    )
}

/// The calibration of the write watch (`--r03-write-restore`): `path` rewritten with its own
/// bytes and its time restored, so both snapshots stay equal and only the watch can report it.
fn write_restore(path: &Path) -> Result<(), String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let bytes = fs::read(path).map_err(error)?;
    let time = fs::metadata(path)
        .and_then(|m| m.modified())
        .map_err(error)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(error)?;
    file.write_all(&bytes).map_err(error)?;
    file.set_modified(time).map_err(error)
}

fn attempt(
    app: &Path,
    dir: &Path,
    expected: &Bytes,
    zip: Option<&Candidate>,
    calibrate: bool,
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
    // The write watch covers every entry before the first snapshot lists them; anything created
    // later is a write to a registered folder (plan 11 #phase-16-macos-host).
    let watch = vnodes::watch(&snapshot::entries(&watched)?, REGISTERED)?;
    note(
        row,
        format!("write watch on {} entries", watch.names().len()),
    );
    let mut o = Observed {
        zip: staged.zip_sha256.clone(),
        quarantine: command(
            "/usr/bin/xattr",
            &["-p", "com.apple.quarantine", &text(&copy)],
        )
        .ok()
        .map(|q| q.trim().to_owned()),
        before: snapshot::tree(&watched)?,
        ..Observed::default()
    };
    if calibrate {
        let target = copy.join("Contents/Info.plist");
        write_restore(&target)?;
        note(
            row,
            format!(
                "calibration: {} rewritten with its own bytes, its time restored",
                target.display()
            ),
        );
    }
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
    // Every process of this run has ended: read the watch before anything else touches the
    // folder, then take the second snapshot.
    let registered = watch.names().to_vec();
    o.watch = Some(Watched {
        entries: registered,
        events: watch.drain()?,
    });
    o.after = snapshot::tree(&watched)?;
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
    row.facts = Some(facts::json(&o));
    Ok(None)
}
