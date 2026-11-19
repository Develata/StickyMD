//! P16-R03: a package downloaded from the internet starts under App Translocation, shows the
//! product's own prompt, and exits after it without writing anything. The driver stands in for
//! the browser and for the USER's first-open approval: it zips the app under test, marks the
//! archive and every extracted file with a quarantine attribute whose user-approved bit (0x40) is
//! set, so macOS skips its first-open confirmation (USER 2026-11-19, the acceptance ladder
//! report); Gatekeeper assessment is off in the test VM either way, so neither path exercises it.
//! It opens the copy through LaunchServices, as a double click does, then judges the process
//! path, the read-only translocation mount, the prompt (AX text), the exit after its button, and
//! the download directory before and after. A system that does not translocate, or a prompt
//! that cannot be read, is NOT_TESTED; a FAIL needs an observed violation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::{Row, Verdict};

/// The flags field of the quarantine attribute the driver writes: 0x83 as a browser download
/// leaves it, plus the user-approved bit 0x40.
pub(super) const QUARANTINE_FLAGS: &str = "00c3";
/// The start of the product's prompt in a directory it cannot write.
pub(super) const PROMPT: &str = "当前目录不可写";
/// The prompt's button.
pub(super) const BUTTONS: &[&str] = &["好", "OK"];

/// What the live run observed.
#[derive(Debug, Default)]
pub(super) struct Observed {
    /// The quarantine attribute read back from the extracted bundle.
    pub quarantine: Option<String>,
    /// The executable path of the one StickyMD process LaunchServices started.
    pub path: Option<String>,
    /// The `mount` line of the bundle's translocation.
    pub mount: Option<String>,
    /// The process ended before any prompt was read.
    pub exited_early: bool,
    /// The prompt's static texts, if the AX tree showed one with its button.
    pub prompt: Option<String>,
    /// After pressing the button: whether the process ended within the wait.
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
    let prompted = o.prompt.as_deref().is_some_and(|t| t.contains(PROMPT));
    let verdict = if prompted {
        Verdict::Pass
    } else if o.exited_early || o.prompt.is_some() {
        // Gone without a prompt, or a prompt with other words.
        Verdict::Fail
    } else {
        // Still running and no prompt in the AX tree: the prompt may be unreadable, not absent.
        Verdict::NotTested
    };
    record(
        row,
        "prompt names the read-only directory",
        verdict,
        format!(
            "prompt={:?}; exited before a prompt {}",
            o.prompt, o.exited_early
        ),
    );
    record(
        row,
        "exits after the prompt",
        match (prompted, o.exited) {
            (true, Some(true)) => Verdict::Pass,
            (true, Some(false)) => Verdict::Fail,
            _ => Verdict::NotTested,
        },
        format!("ended within the wait after the button: {:?}", o.exited),
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
    use super::{BUTTONS, Observed, PROMPT, QUARANTINE_FLAGS, judge};
    use crate::mac_gui::{
        live::command,
        model::{Row, Trace},
        native::{accessibility as ax, events},
    };
    use std::{
        fs,
        path::Path,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

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
        // The prompt: up to 15 s, read from the AX tree.
        let start = Instant::now();
        while start.elapsed().as_secs() < 15 {
            if !alive()? {
                o.exited_early = true;
                break;
            }
            if let Ok((texts, button)) = ax::alert(pid, BUTTONS) {
                if button && texts.contains(PROMPT) {
                    o.prompt = Some(texts);
                    break;
                }
                if button && !texts.is_empty() {
                    o.prompt = Some(texts);
                }
            }
            events::wait(200)?;
        }
        note(
            row,
            format!("prompt {:?}; exited early {}", o.prompt, o.exited_early),
        );
        if o.prompt.as_deref().is_some_and(|t| t.contains(PROMPT)) {
            ax::press(pid, BUTTONS)?;
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
        }
        // Leave nothing running: a product that did not exit is stopped here.
        if alive()? {
            let _ = command("/bin/kill", &["-TERM", &pid.to_string()]);
            events::wait(1000)?;
            if alive()? {
                let _ = command("/bin/kill", &["-KILL", &pid.to_string()]);
            }
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
