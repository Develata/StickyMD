//! `mac-logout`: P16-W12's real logout, one case per fresh GUI session of the test VM (USER
//! 2026-11-19). A logout ends the GUI session and every process connected to the window server,
//! so the coordinating process never touches CoreGraphics: a child it starts (`--prepare`) does
//! the GUI part — starts the copied product through LaunchServices as Finder does, focuses it,
//! types and opens the export dialog or makes the save fail — and exits; the coordinator then
//! asks loginwindow to log out over Apple Events (no consent prompt, RISK-macos-logout-ui-element),
//! survives it in the SSH session, and judges the note, the export destination and the system log.
//! Expectations follow plan 09's 系统终止 row: the request reaches the product while its text is
//! unsaved; that text is saved and it exits normally; an open export dialog is ended, the note
//! saved, nothing exported; a failing save refuses (`NO`), the old note stays, and the system
//! kills it. Each case is the ledger row `W12-logout-<case>`; W12 counts only with all of them
//! and `mac-gui`'s W12 under one identity. After a case the VM sits at the login window: restart
//! it before the next one (the coordinator refuses a second case in one boot).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping
use std::path::PathBuf;

#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) mod check;
#[cfg(target_os = "macos")]
mod live;

pub(super) const HELP: &str = "usage: stickymd-smoke mac-logout --app <StickyMD.app> --case <text|dialog|fault> [--out <dir>] [--batch <name>]
VM ONLY (kern.hv_vmm_present must be 1). Logs the VM's GUI user out; run it over SSH, one case per
fresh boot of the VM, and restart the VM afterwards. Writes mac-gui.json (schema 2) with the row
P16-W12-logout-<case> for mac-ledger record; finished stays false until the product is gone and
the fixture is removed. --out defaults to dist/evidence/mac-logout/<case>-<unix seconds> and must
not hold a mac-gui.json yet.
  text    type, then log out at once (the text is not yet autosaved)
  dialog  a line autosaved, a second typed, the export dialog opened at <fixture>/export.md, then
          log out
  fault   a line autosaved, the note directory made read-only, a second typed, then log out
Requires Accessibility and PostEvent for the driver (the GUI part runs in a child).";

/// The cases, in the order the ledger lists them.
pub(crate) const CASES: [&str; 3] = ["text", "dialog", "fault"];

/// Whether `name` (lowercased) is one `mac-logout` may write into --out: the evidence and its
/// `W12-logout-*` logs (review round 3 B2; the one rule its driver and the tests use).
#[cfg(target_os = "macos")]
pub(crate) fn written_here(name: &str) -> bool {
    name == "mac-gui.json" || name.starts_with("w12-logout-")
}

/// The ledger rows of the logout cases.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn rows() -> Vec<String> {
    CASES.iter().map(|c| format!("W12-logout-{c}")).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub app: PathBuf,
    pub case: String,
    /// The evidence directory; one of its own per run when not given.
    pub out: Option<PathBuf>,
    pub batch: Option<String>,
    /// Internal: the child that does the GUI part in this fixture directory.
    pub prepare: Option<PathBuf>,
    pub help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        app: PathBuf::new(),
        case: String::new(),
        out: None,
        batch: None,
        prepare: None,
        help: false,
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut iter = args.iter();
    while let Some(key) = iter.next() {
        if !seen.insert(key) {
            return Err(format!("duplicate option: {key}"));
        }
        if matches!(key.as_str(), "--help" | "-h") {
            o.help = true;
            continue;
        }
        let value = iter
            .next()
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| format!("{key} needs a value"))?;
        match key.as_str() {
            "--app" => o.app = value.into(),
            "--out" => o.out = Some(value.into()),
            "--prepare" => o.prepare = Some(value.into()),
            "--case" if CASES.contains(&value.as_str()) => o.case = value.clone(),
            "--case" => return Err(format!("unknown case {value:?}; one of {CASES:?}")),
            "--batch" if crate::mac_ledger::is_batch(value) => o.batch = Some(value.clone()),
            "--batch" => return Err(format!("--batch {value:?} is not 1-64 of [A-Za-z0-9._-]")),
            _ => return Err(format!("unknown mac-logout option: {key}")),
        }
    }
    if !o.help && (o.app.as_os_str().is_empty() || o.case.is_empty()) {
        return Err(HELP.into());
    }
    Ok(o)
}

pub(crate) fn execute(options: &Options) -> Result<(), String> {
    if options.help {
        println!("{HELP}");
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    return live::execute(options);
    #[cfg(not(target_os = "macos"))]
    Err("mac-logout requires macOS; nothing was started".into())
}
