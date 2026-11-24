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
//! "Nothing written" (USER 2026-11-24, option A) reads a kqueue watch over the folder holding
//! the bundle (`native::vnodes`) as well as two snapshots of it (`snapshot`): a write undone
//! before the end is still a write. The rule here is pure, and mac-ledger recomputes the checks
//! from the facts the run records (`facts`).
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
/// Renamed with the kqueue watch: a check list from a driver that compared snapshots only
/// (review #24 B9) cannot be recorded as an R03 PASS.
pub(crate) const UNWRITTEN: &str =
    "nothing written in the folder holding the package (kqueue and snapshots)";
pub(crate) const CHECKS: [&str; 6] = [PRESET, TRANSLOCATED, PROMPTED, EXITS, STATUS, UNWRITTEN];

/// `EVFILT_VNODE` flags (`sys/event.h`).
pub(super) const NOTE_DELETE: u32 = 0x01;
pub(super) const NOTE_WRITE: u32 = 0x02;
pub(super) const NOTE_EXTEND: u32 = 0x04;
pub(super) const NOTE_ATTRIB: u32 = 0x08;
pub(super) const NOTE_LINK: u32 = 0x10;
pub(super) const NOTE_RENAME: u32 = 0x20;
pub(super) const NOTE_REVOKE: u32 = 0x40;
/// What the watch registers for each entry.
pub(super) const REGISTERED: u32 =
    NOTE_DELETE | NOTE_WRITE | NOTE_EXTEND | NOTE_ATTRIB | NOTE_LINK | NOTE_RENAME | NOTE_REVOKE;
/// The flags that are a write: an entry's bytes, its folder's names or a link count changed.
/// Attributes (`NOTE_ATTRIB`: permissions, extended attributes, times) are not, since Gatekeeper
/// itself changes the bundle's quarantine attribute; a revoked entry ended the watch early.
pub(super) const WRITTEN: u32 = NOTE_DELETE | NOTE_WRITE | NOTE_EXTEND | NOTE_LINK | NOTE_RENAME;
const NAMES: [(u32, &str); 7] = [
    (NOTE_DELETE, "DELETE"),
    (NOTE_WRITE, "WRITE"),
    (NOTE_EXTEND, "EXTEND"),
    (NOTE_ATTRIB, "ATTRIB"),
    (NOTE_LINK, "LINK"),
    (NOTE_RENAME, "RENAME"),
    (NOTE_REVOKE, "REVOKE"),
];

/// `flags` as `WRITE|ATTRIB`; bits outside [`REGISTERED`] as hexadecimal.
pub(super) fn flag_names(flags: u32) -> String {
    let mut out: Vec<String> = NAMES
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect();
    if flags & !REGISTERED != 0 {
        out.push(format!("{:#x}", flags & !REGISTERED));
    }
    out.join("|")
}

/// The kqueue watch over the folder holding the bundle: the entries registered before the
/// start, as the snapshot names them, and the raw flags each reported by the end (entries that
/// reported nothing left out).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Watched {
    pub entries: Vec<String>,
    pub events: Vec<(String, u32)>,
}
/// The release ZIP R03 opened as given (`mac-gui --zip`); its observation is `sha256=<digest>`.
pub(crate) const ZIP_CHECK: &str = "opened the candidate ZIP as given";

/// What the live run observed.
#[derive(Debug, Default, PartialEq)]
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
    /// The write watch over the same folder, when it ran to the end.
    pub watch: Option<Watched>,
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
    let (verdict, detail) = unwritten(o, ended);
    record(row, UNWRITTEN, verdict, detail);
}

/// "Nothing written": both snapshots equal and no entry of the watch written, extended,
/// linked, renamed or deleted (plan 11 #phase-16-macos-host). kqueue does not say who wrote;
/// in the VM nothing else runs, and the driver only reads between the two snapshots.
fn unwritten(o: &Observed, ended: bool) -> (Verdict, String) {
    if !ended {
        return (
            Verdict::NotTested,
            "the process did not end after the prompt".into(),
        );
    }
    if o.before != o.after {
        return (
            Verdict::Fail,
            format!(
                "changed: {:?}",
                super::snapshot::changed(&o.before, &o.after)
            ),
        );
    }
    let Some(watch) = &o.watch else {
        return (Verdict::NotTested, "no write watch was read".into());
    };
    // The same entries, whatever their order: the snapshot sorts its lines by kind.
    let mut listed: Vec<&str> = o
        .before
        .iter()
        .map(|line| super::snapshot::name(line).unwrap_or(line))
        .collect();
    listed.sort_unstable();
    let mut registered: Vec<&str> = watch.entries.iter().map(String::as_str).collect();
    registered.sort_unstable();
    if listed != registered {
        return (
            Verdict::NotTested,
            format!("the watch registered {registered:?}, the snapshot lists {listed:?}"),
        );
    }
    let with = |mask: u32| -> Vec<String> {
        watch
            .events
            .iter()
            .filter(|(_, flags)| flags & mask != 0)
            .map(|(entry, flags)| format!("{entry}: {}", flag_names(*flags)))
            .collect()
    };
    let written = with(WRITTEN);
    if !written.is_empty() {
        return (
            Verdict::Fail,
            format!("written while the product ran (kqueue), snapshots equal: {written:?}"),
        );
    }
    let revoked = with(NOTE_REVOKE);
    if !revoked.is_empty() {
        return (
            Verdict::NotTested,
            format!("the watch was revoked: {revoked:?}"),
        );
    }
    (
        Verdict::Pass,
        format!(
            "{} entries under the folder holding the bundle unchanged (sizes, SHA-256, times) and none written, extended, linked, renamed or deleted (kqueue); attributes changed: {:?}",
            o.before.len(),
            with(NOTE_ATTRIB)
        ),
    )
}

pub(crate) mod facts;
#[cfg(target_os = "macos")]
mod live;
#[cfg(target_os = "macos")]
pub(super) use live::run;
