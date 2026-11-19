//! P16-R03: a package downloaded from the internet starts under App Translocation, shows the
//! product's own prompt, and exits after it without writing anything. The driver stands in for
//! the browser and for the USER's first-open approval: it zips the app under test, marks the
//! archive and every extracted file with a quarantine attribute whose user-approved bit (0x40) is
//! set, so macOS skips its first-open confirmation (USER 2026-11-19, the acceptance ladder
//! report). It opens the copy through LaunchServices, as a double click does.
//!
//! The prompt is shown before the application has finished launching, so its Accessibility
//! requests do not complete (macOS 27.2 VM: `kAXErrorCannotComplete`) and it owns no AX window.
//! The prompt is therefore judged from what the system does report: a fully shown window of the
//! process (alpha at least 0.9, at least 200×150 pt) on screen at the modal-panel level or above, and the message on its stderr, which the product
//! writes just before showing the same text. It is answered as the manual procedure does:
//! LaunchServices is asked to open the copy again, which activates the running instance, and
//! Return is posted to that process alone, so no other application can receive it.
//! A system that does not translocate, or a prompt window not seen while the process runs, is
//! NOT_TESTED; a FAIL needs an observed violation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::{Row, Verdict};

/// The flags field of the quarantine attribute the driver writes: 0x83 as a browser download
/// leaves it, plus the user-approved bit 0x40.
pub(super) const QUARANTINE_FLAGS: &str = "00c3";
/// The start of the product's prompt in a directory it cannot write.
pub(super) const PROMPT: &str = "当前目录不可写";
/// The cause the prompt carries on a read-only translocation.
pub(super) const CAUSE: &str = "Read-only file system";
/// `NSModalPanelWindowLevel`: an alert run modally sits at this level or above.
pub(super) const MODAL_LEVEL: i32 = 8;

/// What the live run observed.
#[derive(Debug, Default)]
pub(super) struct Observed {
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
    /// After Return: whether the process ended within the wait.
    pub exited: Option<bool>,
    /// The download directory's entries before the start and after the exit.
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

/// Whether the prompt was shown: its window seen, and the message with its cause on stderr.
pub(super) fn prompted(o: &Observed) -> bool {
    o.window.is_some() && o.stderr.contains(PROMPT) && o.stderr.contains(CAUSE)
}

/// The R03 checks from what was observed. Each depends on the one before it: without the
/// preset attribute nothing is a download; without translocation the prompt is not owed.
pub(super) fn judge(o: &Observed, row: &mut Row) {
    let preset = o
        .quarantine
        .as_deref()
        .is_some_and(|q| q.starts_with(&format!("{QUARANTINE_FLAGS};")));
    record(
        row,
        "approved quarantine preset (USER 2026-11-19)",
        if preset {
            Verdict::Pass
        } else {
            Verdict::NotTested
        },
        format!("com.apple.quarantine={:?}", o.quarantine),
    );
    let translocated = o
        .path
        .as_deref()
        .is_some_and(|p| p.contains("/AppTranslocation/"))
        && o.mount
            .as_deref()
            .is_some_and(|m| m.contains("nullfs") && m.contains("read-only"));
    // Whether macOS translocates is the system's decision; without it the row cannot be judged.
    record(
        row,
        "started under App Translocation",
        if preset && translocated {
            Verdict::Pass
        } else {
            Verdict::NotTested
        },
        format!("path={:?}; mount={:?}", o.path, o.mount),
    );
    if !(preset && translocated) {
        for name in [
            "prompt names the read-only directory",
            "exits after the prompt",
            "nothing written beside the package",
        ] {
            row.skip(name, "not started under App Translocation");
        }
        return;
    }
    let shown = prompted(o);
    let verdict = if shown {
        Verdict::Pass
    } else if o.exited_early || (o.window.is_some() && !o.stderr.contains(PROMPT)) {
        // Gone without a prompt, or a prompt about something else.
        Verdict::Fail
    } else {
        // Still running and no modal window seen: the prompt may be unobservable, not absent.
        Verdict::NotTested
    };
    record(
        row,
        "prompt names the read-only directory",
        verdict,
        format!(
            "window={:?}; exited before a prompt {}; stderr={:?}",
            o.window, o.exited_early, o.stderr
        ),
    );
    record(
        row,
        "exits after the prompt",
        match (shown, o.exited) {
            (true, Some(true)) => Verdict::Pass,
            (true, Some(false)) => Verdict::Fail,
            _ => Verdict::NotTested,
        },
        format!("ended within the wait after Return: {:?}", o.exited),
    );
    let ended = o.exited_early || o.exited == Some(true);
    record(
        row,
        "nothing written beside the package",
        if !ended {
            Verdict::NotTested
        } else if o.before == o.after {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        format!("before={:?}; after={:?}", o.before, o.after),
    );
}

#[cfg(target_os = "macos")]
pub(super) use live::run;

#[cfg(target_os = "macos")]
mod live {
    use super::{MODAL_LEVEL, Observed, QUARANTINE_FLAGS, judge, prompted};
    use crate::mac_gui::{
        live::command,
        model::{Row, Trace},
        native::{events, windows},
    };
    use std::{
        fs,
        path::Path,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    /// The virtual key code of Return.
    const RETURN: u16 = 36;

    fn entries(dir: &Path) -> Result<Vec<String>, String> {
        let mut names = fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .map(|e| {
                e.map(|e| e.file_name().to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        names.sort();
        Ok(names)
    }

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
            if let Ok(path) = command("/bin/ps", &["-o", "comm=", "-p", &pid.to_string()]) {
                out.push((pid, path.trim().to_owned()));
            }
        }
        Ok(out)
    }

    /// Runs R03 in `dir` (the scenario's session directory) for the app under test, whose
    /// bundle manifest must be `bundle`. Leaves no process of its own running.
    pub(in crate::mac_gui) fn run(
        app: &Path,
        dir: &Path,
        bundle: &str,
        started: Instant,
        row: &mut Row,
    ) -> Result<(), String> {
        let note = |row: &mut Row, event: String| {
            row.trace.push(Trace {
                ms: started.elapsed().as_millis() as u64,
                event,
            });
        };
        if !products()?.is_empty() {
            return Err("a StickyMD process is already running; refused before R03".into());
        }
        let download = dir.join("download");
        fs::create_dir(&download).map_err(|e| e.to_string())?;
        let download = fs::canonicalize(&download).map_err(|e| e.to_string())?;
        let zip = download.join("StickyMD.zip");
        let copy = download.join("StickyMD.app");
        let text = |p: &Path| p.to_string_lossy().into_owned();
        command(
            "/usr/bin/ditto",
            &["-c", "-k", "--keepParent", &text(app), &text(&zip)],
        )?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let value = format!("{QUARANTINE_FLAGS};{stamp:x};Safari;");
        command(
            "/usr/bin/xattr",
            &["-w", "com.apple.quarantine", &value, &text(&zip)],
        )?;
        command(
            "/usr/bin/ditto",
            &["-x", "-k", &text(&zip), &text(&download)],
        )?;
        // As Archive Utility does, every extracted file carries the archive's attribute.
        command(
            "/usr/bin/xattr",
            &["-w", "-r", "com.apple.quarantine", &value, &text(&copy)],
        )?;
        if crate::mac_common::fixture::bundle_hash(&copy)? != bundle {
            return Err("extracted bundle manifest differs from the app under test".into());
        }
        let mut o = Observed {
            quarantine: command(
                "/usr/bin/xattr",
                &["-p", "com.apple.quarantine", &text(&copy)],
            )
            .ok()
            .map(|q| q.trim().to_owned()),
            before: entries(&download)?,
            ..Observed::default()
        };
        let stderr = dir.join("stderr.log");
        let read_stderr = || fs::read_to_string(&stderr).unwrap_or_default();
        note(
            row,
            format!("open {} (quarantine {:?})", copy.display(), o.quarantine),
        );
        command("/usr/bin/open", &[&text(&copy), "--stderr", &text(&stderr)])?;
        // The process: up to 20 s for LaunchServices, AMFI and TCC.
        let start = Instant::now();
        let mut pid = None;
        while start.elapsed().as_secs() < 20 {
            let found = products()?;
            if let [(p, path)] = found.as_slice() {
                pid = Some(*p);
                o.path = Some(path.clone());
                break;
            }
            if found.len() > 1 {
                return Err(format!("more than one StickyMD process: {found:?}"));
            }
            events::wait(100)?;
        }
        let Some(pid) = pid else {
            note(row, "no StickyMD process within 20s".into());
            judge(&o, row);
            return Ok(());
        };
        note(row, format!("PID {pid} at {:?}", o.path));
        let alive = || products().map(|p| p.iter().any(|(q, _)| *q == pid));
        let mounts = command("/sbin/mount", &[])?;
        o.mount = mounts
            .lines()
            .find(|l| {
                l.contains("AppTranslocation") && l.starts_with(&format!("{} on ", copy.display()))
            })
            .map(str::to_owned);
        // The prompt: up to 15 s for its window and its message.
        let start = Instant::now();
        while start.elapsed().as_secs() < 15 {
            if !alive()? {
                o.exited_early = true;
                break;
            }
            o.window = windows::all(pid)?
                .into_iter()
                .find(|w| {
                    // A fully shown alert, not one still fading or scaling in.
                    w.onscreen
                        && w.layer >= MODAL_LEVEL
                        && w.alpha >= 0.9
                        && w.bounds.size.width >= 200.0
                        && w.bounds.size.height >= 150.0
                })
                .map(|w| format!("{w:?}"));
            o.stderr = read_stderr();
            if prompted(&o) {
                break;
            }
            events::wait(200)?;
        }
        o.stderr = read_stderr();
        note(
            row,
            format!("window {:?}; exited early {}", o.window, o.exited_early),
        );
        if prompted(&o) {
            // Activate the running instance as the manual procedure does, then answer it.
            command("/usr/bin/open", &[&text(&copy)])?;
            events::wait(1500)?;
            let others: Vec<_> = products()?.into_iter().filter(|(q, _)| *q != pid).collect();
            if !others.is_empty() {
                note(
                    row,
                    format!("reopening started another process: {others:?}"),
                );
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
            o.exited = Some(ended && others.is_empty());
        }
        // Leave nothing running: a product that did not exit is stopped here.
        for (q, _) in products()? {
            let _ = command("/bin/kill", &["-TERM", &q.to_string()]);
        }
        events::wait(500)?;
        for (q, _) in products()? {
            let _ = command("/bin/kill", &["-KILL", &q.to_string()]);
        }
        o.after = entries(&download)?;
        // macOS detaches the translocation after the process ends; wait for it before cleanup.
        let start = Instant::now();
        while start.elapsed().as_secs() < 10 {
            let mounts = command("/sbin/mount", &[])?;
            if !mounts
                .lines()
                .any(|l| l.starts_with(&format!("{} on ", copy.display())))
            {
                break;
            }
            events::wait(250)?;
        }
        note(row, format!("after {:?}", o.after));
        judge(&o, row);
        Ok(())
    }
}
