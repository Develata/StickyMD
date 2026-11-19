//! `mac-logout`: P16-W12's real logout, one case per fresh GUI session of the test VM (USER
//! 2026-11-19). A logout ends the GUI session and every process connected to the window server,
//! so the coordinating process never touches CoreGraphics: a child it starts (`--prepare`) does
//! the GUI part — starts the copied product through LaunchServices as Finder does, focuses it,
//! types and opens the export dialog or makes the save fail — and exits; the coordinator then
//! asks loginwindow to log out over Apple Events (no consent prompt, RISK-macos-logout-ui-element),
//! survives it in the SSH session, and judges the note, any exported file and the system log.
//! Expectations follow plan 09's 系统终止 row: the request reaches the product; unsaved text is
//! saved and it exits normally; an open export dialog is cancelled, the note saved, nothing
//! exported; a failing save refuses (`NO`), the old note stays, and the system kills it. Each
//! case is the ledger row `W12-logout-<case>`; W12 counts only with all of them and `mac-gui`'s
//! W12 under one identity. After a case the VM sits at the login window: restart it before the
//! next one.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping
use std::path::PathBuf;

pub(super) const HELP: &str = "usage: stickymd-smoke mac-logout --app <StickyMD.app> --case <text|dialog|fault> [--out <dir>] [--batch <name>]
VM ONLY (kern.hv_vmm_present must be 1). Logs the VM's GUI user out; run it over SSH, one case per
fresh boot of the VM, and restart the VM afterwards. Writes mac-gui.json (schema 2) with the row
P16-W12-logout-<case> for mac-ledger record.
  text    type, then log out at once (the text is not yet autosaved)
  dialog  a line autosaved, a second typed, the export dialog opened, then log out
  fault   a line autosaved, the note directory made read-only, a second typed, then log out
Requires Accessibility and PostEvent for the driver (the GUI part runs in a child).";

/// The cases, in the order the ledger lists them.
pub(crate) const CASES: [&str; 3] = ["text", "dialog", "fault"];

/// The ledger rows of the logout cases.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn rows() -> Vec<String> {
    CASES.iter().map(|c| format!("W12-logout-{c}")).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub app: PathBuf,
    pub case: String,
    pub out: PathBuf,
    pub batch: Option<String>,
    /// Internal: the child that does the GUI part in this fixture directory.
    pub prepare: Option<PathBuf>,
    pub help: bool,
}

pub(crate) fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        app: PathBuf::new(),
        case: String::new(),
        out: "dist/evidence/mac-logout".into(),
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
            "--out" => o.out = value.into(),
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

/// The pure part: what each case types and how its facts are judged.
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) mod check {
    use super::super::model::{Row, Verdict};

    /// The note before every case, and what each case types.
    pub(in crate::mac_gui) const NOTE: &str = "before logout\n";
    pub(in crate::mac_gui) const TYPED: &str = "typed just before logout";
    pub(in crate::mac_gui) const SAVED: &str = "saved line";
    pub(in crate::mac_gui) const UNSAVED: &str = "unsaved line";
    pub(in crate::mac_gui) const LOST: &str = "line the failing save cannot keep";

    /// What the coordinator observed around the logout.
    #[derive(Debug, Default)]
    pub(in crate::mac_gui) struct Facts {
        /// `/dev/console` belongs to root afterwards: the GUI session ended.
        pub logged_out: bool,
        /// The product ended within the wait.
        pub gone: bool,
        /// The note just before the logout request and after it.
        pub before: String,
        pub after: String,
        /// `.md` files written outside the note during the case.
        pub exported: Vec<String>,
        /// From the product's AppKit log: the quit event, its decision and its reply.
        pub quit_event: bool,
        pub decision: Option<String>,
        pub reply: Option<String>,
        /// runningboard's termination record for the PID, e.g. `(0, 0, 0)` or `(2, 9, 9)`.
        pub termination: Option<String>,
    }

    /// The facts the system log gives for `pid`.
    pub(in crate::mac_gui) fn from_log(log: &str, pid: u32, facts: &mut Facts) {
        let own = format!("StickyMD[{pid}]");
        let theirs = format!(":{pid}] termination reported by launchd ");
        for line in log.lines() {
            if line.contains(&own) {
                if line.contains("Handling Quit AppleEvent") {
                    facts.quit_event = true;
                }
                if let Some(rest) = line.split("applicationShouldTerminate: ").nth(1) {
                    facts.decision = Some(rest.trim().to_owned());
                }
                if let Some(rest) = line.split("replyToApplicationShouldTerminate:").nth(1) {
                    facts.reply = Some(rest.trim().to_owned());
                }
            }
            if let Some(rest) = line.split(&theirs).nth(1) {
                facts.termination = Some(rest.trim().to_owned());
            }
        }
    }

    fn record(row: &mut Row, name: &str, verdict: Verdict, detail: String) {
        if verdict == Verdict::NotTested {
            row.skip(name, &detail);
        } else {
            row.check(name, verdict == Verdict::Pass, detail);
        }
    }

    /// PASS when `ok`, FAIL when the fact was seen otherwise, NOT_TESTED when it was not seen.
    fn seen(fact: &Option<String>, ok: impl Fn(&str) -> bool) -> Verdict {
        match fact {
            Some(value) if ok(value) => Verdict::Pass,
            Some(_) => Verdict::Fail,
            None => Verdict::NotTested,
        }
    }

    /// The checks of one case.
    pub(in crate::mac_gui) fn judge(case: &str, f: &Facts, row: &mut Row) {
        let facts = format!("{f:?}");
        if !f.logged_out {
            row.skip(
                "logout happened",
                &format!("the GUI session did not end; {facts}"),
            );
            return;
        }
        row.check(
            "logout happened",
            true,
            "the console belongs to root".to_string(),
        );
        record(
            row,
            "quit request reached the product",
            if f.quit_event {
                Verdict::Pass
            } else {
                Verdict::NotTested
            },
            format!("decision {:?}, reply {:?}", f.decision, f.reply),
        );
        let normal = |t: &str| t == "(0, 0, 0)";
        match case {
            "text" => {
                record(
                    row,
                    "unsaved text saved at logout",
                    if f.after.contains(TYPED) && !f.before.contains(TYPED) {
                        Verdict::Pass
                    } else {
                        Verdict::Fail
                    },
                    format!("before {:?}, after {:?}", f.before, f.after),
                );
                record(row, "exits normally", seen(&f.termination, normal), facts);
            }
            "dialog" => {
                record(
                    row,
                    "export dialog cancelled",
                    seen(&f.decision, |d| d == "NSTerminateCancel"),
                    format!("decision {:?}", f.decision),
                );
                record(
                    row,
                    "note saved and nothing exported",
                    if f.after.contains(&format!("{SAVED}\n{UNSAVED}")) && f.exported.is_empty() {
                        Verdict::Pass
                    } else {
                        Verdict::Fail
                    },
                    format!("after {:?}, exported {:?}", f.after, f.exported),
                );
                record(row, "exits normally", seen(&f.termination, normal), facts);
            }
            "fault" => {
                record(
                    row,
                    "refuses to quit on the failing save",
                    seen(&f.reply, |r| r == "NO"),
                    format!("reply {:?}", f.reply),
                );
                record(
                    row,
                    "old note intact",
                    if f.after == f.before && f.before.contains(SAVED) && !f.after.contains(LOST) {
                        Verdict::Pass
                    } else {
                        Verdict::Fail
                    },
                    format!("before {:?}, after {:?}", f.before, f.after),
                );
                // Refused, so only the system ends it: a normal exit would have dropped the edit.
                record(
                    row,
                    "ended by the system",
                    seen(&f.termination, |t| t == "(2, 9, 9)" || t == "(2, 15, 15)"),
                    facts,
                );
            }
            _ => row.skip("case", &format!("unknown case {case:?}")),
        }
    }
}

#[cfg(target_os = "macos")]
mod live {
    use super::Options;
    use super::check::{Facts, LOST, NOTE, SAVED, UNSAVED, from_log, judge};
    use crate::mac_common::{fixture, session::Session};
    use crate::mac_gui::{
        input::{CMD, Injection, SHIFT},
        live::command,
        model::{Permissions, Row, quote},
        native::{accessibility as ax, events, permissions, windows},
        observation::{KeyTarget, key_target},
    };
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const CONFIG: &str = "version = 1\nview_mode = \"source\"\nopacity = 100\nalways_on_top = false\ncontent_zoom_percent = 100\n[window]\nwidth_dip = 520\nheight_dip = 480\ndock_edge = \"none\"\n";
    /// Key codes on the US/ABC layout: `A` (Cmd+A selects the whole note first, so each case
    /// starts from its own text) and `S` (Cmd+Shift+S opens the export dialog).
    const KEY_A: u16 = 0;
    const KEY_S: u16 = 1;

    pub(in crate::mac_gui) fn execute(options: &Options) -> Result<(), String> {
        let vm = command("/usr/sbin/sysctl", &["-n", "kern.hv_vmm_present"])?;
        if vm.trim() != "1" {
            return Err(format!(
                "mac-logout refused: kern.hv_vmm_present={} (requires the test VM)",
                vm.trim()
            ));
        }
        match &options.prepare {
            Some(dir) => prepare(options, dir),
            None => coordinate(options),
        }
    }

    /// The child: the GUI part of a case, ending before the logout. Prints the product's PID.
    fn prepare(options: &Options, dir: &Path) -> Result<(), String> {
        let p: Permissions = permissions::check();
        if !p.ax || !p.post {
            return Err("mac-logout needs Accessibility and PostEvent for the driver".into());
        }
        let app = dir.join("StickyMD.app");
        let executable = fixture::contained_executable(&app)?;
        let stderr = dir.join("stderr.log");
        command(
            "/usr/bin/open",
            &[
                "-n",
                &app.to_string_lossy(),
                "--stderr",
                &stderr.to_string_lossy(),
            ],
        )?;
        let start = Instant::now();
        let pid = loop {
            let found = command("/usr/bin/pgrep", &["-f", &executable.to_string_lossy()])
                .unwrap_or_default();
            let pid = found
                .split_whitespace()
                .next()
                .and_then(|p| p.parse::<u32>().ok());
            if let Some(pid) = pid
                && windows::window(pid, None).is_ok_and(|w| w.onscreen)
            {
                break pid;
            }
            if start.elapsed() > Duration::from_secs(20) {
                return Err("the product showed no window within 20 s".into());
            }
            events::wait(200)?;
        };
        // Opening the bundle again shows the window and takes focus (W07).
        command("/usr/bin/open", &[&app.to_string_lossy()])?;
        events::wait(2000)?;
        let mut input = Injection::new(events::Events::new());
        let guard = |input: &Injection<events::Events>| -> Result<(), String> {
            input.require_running()?;
            key_target(ax::front(), ax::focused_window(pid), pid, KeyTarget::Note)
        };
        let note = dir.join("note/note.md");
        let typed = |input: &mut Injection<events::Events>, text: &str| -> Result<(), String> {
            for c in text.chars() {
                guard(input)?;
                input.text(c.encode_utf8(&mut [0; 4]))?;
            }
            Ok(())
        };
        let result = (|| -> Result<(), String> {
            guard(&input)?;
            input.chord(&[CMD, KEY_A])?;
            match options.case.as_str() {
                "text" => typed(&mut input, super::check::TYPED)?,
                "dialog" => {
                    typed(&mut input, SAVED)?;
                    events::wait(1500)?;
                    typed(&mut input, &format!("\n{UNSAVED}"))?;
                    guard(&input)?;
                    input.chord(&[CMD, SHIFT, KEY_S])?;
                    let start = Instant::now();
                    while !ax::panel_open(pid).unwrap_or(false) {
                        if start.elapsed() > Duration::from_secs(5) {
                            return Err("the export dialog did not open within 5 s".into());
                        }
                        events::wait(100)?;
                    }
                    events::wait(1500)?;
                }
                "fault" => {
                    typed(&mut input, SAVED)?;
                    events::wait(1500)?;
                    let dir = note.parent().ok_or("note has no directory")?;
                    fs::set_permissions(dir, fs::Permissions::from_mode(0o555))
                        .map_err(|e| e.to_string())?;
                    typed(&mut input, &format!("\n{LOST}"))?;
                    events::wait(1500)?;
                }
                other => return Err(format!("unknown case {other}")),
            }
            Ok(())
        })();
        let release = input.release();
        result.and(release)?;
        println!("pid={pid}");
        Ok(())
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap_or_default()
    }

    fn alive(pid: u32) -> bool {
        Command::new("/bin/kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }

    /// The coordinator: no CoreGraphics here, so it outlives the logout.
    fn coordinate(options: &Options) -> Result<(), String> {
        let app = options
            .app
            .canonicalize()
            .map_err(|e| format!("app path: {e}"))?;
        let executable = fixture::contained_executable(&app)?;
        let exe_hash = crate::integrity::sha256(&executable)?;
        let driver = std::env::current_exe().map_err(|e| e.to_string())?;
        let driver_hash = crate::integrity::sha256(&driver)?;
        let bundle_hash = fixture::bundle_hash(&app)?;
        let inputs = fixture::app_inputs(&app)?;
        let out = std::path::absolute(&options.out).map_err(|e| e.to_string())?;
        fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let mut session = Session::new()?;
        fixture::copy_fixture(&mut session, &app, NOTE, CONFIG, &exe_hash)?;
        // Files written from here on are this case's: the copied bundle keeps its old times.
        let marker = session.path().join("case-started");
        fs::write(&marker, b"").map_err(|e| e.to_string())?;
        let dir = session.path().to_path_buf();
        let mut row = Row::new(&format!("W12-logout-{}", options.case));
        let child = Command::new(&driver)
            .args([
                "mac-logout",
                "--app",
                &app.to_string_lossy(),
                "--case",
                &options.case,
                "--prepare",
                &dir.to_string_lossy(),
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("prepare child: {e}"))?;
        let stdout = String::from_utf8_lossy(&child.stdout).into_owned();
        let pid = stdout
            .lines()
            .find_map(|l| l.strip_prefix("pid="))
            .and_then(|p| p.trim().parse::<u32>().ok());
        let mut facts = Facts::default();
        let finished = match pid {
            Some(pid) if child.status.success() => {
                let note = dir.join("note/note.md");
                facts.before = read(&note);
                let mut logout = Command::new("/usr/bin/osascript")
                    .args(["-e", "tell application \"loginwindow\" to «event aevtrlgo»"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("osascript: {e}"))?;
                let start = Instant::now();
                while alive(pid) && start.elapsed() < Duration::from_secs(20) {
                    std::thread::sleep(Duration::from_millis(500));
                }
                facts.gone = !alive(pid);
                std::thread::sleep(Duration::from_secs(3));
                let _ = logout.kill();
                let _ = logout.wait();
                facts.logged_out = command("/usr/bin/stat", &["-f", "%Su", "/dev/console"])
                    .is_ok_and(|owner| owner.trim() == "root");
                if options.case == "fault" {
                    let _ =
                        fs::set_permissions(dir.join("note"), fs::Permissions::from_mode(0o755));
                }
                facts.after = read(&note);
                let home = std::env::var("HOME").unwrap_or_default();
                for root in [home.as_str(), &dir.to_string_lossy()] {
                    let found = command(
                        "/usr/bin/find",
                        &[
                            root,
                            "-newer",
                            &marker.to_string_lossy(),
                            "-name",
                            "*.md",
                            "-not",
                            "-path",
                            &format!("{}/note/*", dir.display()),
                            "-not",
                            "-path",
                            &format!("{home}/Library/*"),
                        ],
                    )
                    .unwrap_or_default();
                    facts
                        .exported
                        .extend(found.lines().map(str::to_owned).filter(|l| !l.is_empty()));
                }
                let log = command(
                    "/usr/bin/log",
                    &[
                        "show",
                        "--last",
                        "3m",
                        "--style",
                        "syslog",
                        "--predicate",
                        "process == \"StickyMD\" OR eventMessage CONTAINS[c] \"stickymd\"",
                    ],
                )
                .unwrap_or_default();
                crate::atomic_evidence::write(
                    &out.join(format!("W12-logout-{}-system.log", options.case)),
                    log.as_bytes(),
                )?;
                from_log(&log, pid, &mut facts);
                judge(&options.case, &facts, &mut row);
                true
            }
            _ => {
                row.skip(
                    "execution",
                    &format!(
                        "the GUI part did not finish: {}; stdout {stdout:?}; stderr {:?}",
                        child.status,
                        String::from_utf8_lossy(&child.stderr)
                    ),
                );
                false
            }
        };
        if let Ok(stderr) = fs::read(dir.join("stderr.log")) {
            crate::atomic_evidence::write(
                &out.join(format!("W12-logout-{}-stderr.log", options.case)),
                &stderr,
            )?;
        }
        let environment = format!(
            "{{\"os\":{},\"architecture\":{},\"hv_vmm_present\":\"1\",\"allow_host\":false,\"unix_seconds\":{started},\"fixture_config\":{},\"case\":{}}}",
            quote(&command("/usr/bin/sw_vers", &[])?),
            quote(std::env::consts::ARCH),
            quote(CONFIG),
            quote(&options.case)
        );
        let batch = options
            .batch
            .as_deref()
            .map_or_else(String::new, |b| format!(",\"batch\":{}", quote(b)));
        let inputs = inputs.map_or_else(String::new, |i| format!(",\"app_inputs\":{}", quote(&i)));
        let json = format!(
            "{{\"schema_version\":2,\"acceptance_status\":\"INCOMPLETE\",\"finished\":{finished}{batch},\"environment\":{environment}{inputs},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"scenarios\":[{}]}}\n",
            quote(&exe_hash),
            quote(&bundle_hash),
            quote(&driver_hash),
            row.json()
        );
        crate::atomic_evidence::write(&out.join("mac-gui.json"), json.as_bytes())?;
        let _ = session.cleanup();
        let verdict = super::super::model::aggregate(&row.checks);
        println!("W12-logout-{}: {}", options.case, verdict.name());
        println!("Evidence: {}", out.join("mac-gui.json").display());
        if verdict == super::super::model::Verdict::Fail || !finished {
            Err("mac-logout did not pass; see mac-gui.json".into())
        } else {
            Ok(())
        }
    }
}
