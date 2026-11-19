//! The live part of `mac-logout`: the GUI child, and the coordinator that outlives the logout.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::Options;
use super::check::{Facts, LOST, NOTE, Note, SAVED, TYPED, UNSAVED, from_log, judge};
use crate::mac_common::{end_fixture_processes, fixture, fixture_processes, session::Session};
use crate::mac_gui::{
    input::{CMD, Injection, SHIFT},
    live::{aim_panel, command},
    model::{Permissions, Row, Verdict, aggregate, quote},
    native::{accessibility as ax, events, permissions, windows},
    observation::{KeyTarget, key_target},
};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const CONFIG: &str = "version = 1\nview_mode = \"source\"\nopacity = 100\nalways_on_top = false\ncontent_zoom_percent = 100\n[window]\nwidth_dip = 520\nheight_dip = 480\ndock_edge = \"none\"\n";
/// Key codes on the US/ABC layout: `A` (Cmd+A selects the whole note first, so each case
/// starts from its own text) and `S` (Cmd+Shift+S opens the export dialog).
const KEY_A: u16 = 0;
const KEY_S: u16 = 1;
/// The boot time of the last case that asked for a logout: one case per boot.
const BOOT_MARKER: &str = "/private/tmp/stickymd-mac-logout-boot";

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

fn micros(t: SystemTime) -> Result<i64, String> {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .map_err(|e| e.to_string())
}

/// The child: the GUI part of a case, ending before the logout. Prints the product's PID as soon
/// as it is known, and the time of the last keystroke into the note.
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
        let found = fixture_processes(&executable)?;
        if found.len() > 1 {
            return Err(format!("more than one product process: {found:?}"));
        }
        if let [pid] = found.as_slice()
            && windows::window(*pid, None).is_ok_and(|w| w.onscreen)
        {
            break *pid;
        }
        if start.elapsed() > Duration::from_secs(20) {
            return Err("the product showed no window within 20 s".into());
        }
        events::wait(200)?;
    };
    println!("pid={pid}");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    // Opening the bundle again shows the window and takes focus (W07).
    command("/usr/bin/open", &[&app.to_string_lossy()])?;
    events::wait(2000)?;
    let mut input = Injection::new(events::Events::new());
    let result = steps(&options.case, &mut input, pid, dir);
    let release = input.release();
    let last_key = result?;
    release?;
    println!("last_key_us={last_key}");
    Ok(())
}

/// A case's keys; returns when the last key reached the note (µs since the epoch).
fn steps(
    case: &str,
    input: &mut Injection<events::Events>,
    pid: u32,
    dir: &Path,
) -> Result<i64, String> {
    let guard = |input: &Injection<events::Events>, target: KeyTarget| -> Result<(), String> {
        input.require_running()?;
        key_target(ax::front(), ax::focused_window(pid), pid, target)
    };
    let typed = |input: &mut Injection<events::Events>, text: &str| -> Result<i64, String> {
        for c in text.chars() {
            guard(input, KeyTarget::Note)?;
            input.text(c.encode_utf8(&mut [0; 4]))?;
        }
        micros(SystemTime::now())
    };
    guard(input, KeyTarget::Note)?;
    input.chord(&[CMD, KEY_A])?;
    match case {
        "text" => typed(input, TYPED),
        "dialog" => {
            typed(input, SAVED)?;
            events::wait(1500)?;
            let last = typed(input, &format!("\n{UNSAVED}"))?;
            guard(input, KeyTarget::Note)?;
            input.chord(&[CMD, SHIFT, KEY_S])?;
            let start = Instant::now();
            while !ax::panel_open(pid).unwrap_or(false) {
                if start.elapsed() > Duration::from_secs(5) {
                    return Err("the export dialog did not open within 5 s".into());
                }
                events::wait(100)?;
            }
            events::wait(500)?;
            guard(input, KeyTarget::Dialog)?;
            // The only file this dialog could export is <fixture>/export.md.
            let aimed = aim_panel(input, pid, dir)?;
            println!("aimed: {}", aimed.replace('\n', "; "));
            Ok(last)
        }
        "fault" => {
            typed(input, SAVED)?;
            events::wait(1500)?;
            fs::set_permissions(dir.join("note"), fs::Permissions::from_mode(0o555))
                .map_err(|e| e.to_string())?;
            let last = typed(input, &format!("\n{LOST}"))?;
            // Long enough for the autosave to try and fail.
            events::wait(1500)?;
            Ok(last)
        }
        other => Err(format!("unknown case {other}")),
    }
}

/// The note's bytes and modification time, read through one open file.
fn read_note(path: &Path) -> Result<Note, String> {
    let error = |e: std::io::Error| format!("{}: {e}", path.display());
    let mut file = fs::File::open(path).map_err(error)?;
    let modified = file.metadata().and_then(|m| m.modified()).map_err(error)?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).map_err(error)?;
    Ok(Note {
        text,
        modified: micros(modified)?,
    })
}

fn console_owner() -> Result<String, String> {
    command("/usr/bin/stat", &["-f", "%Su", "/dev/console"]).map(|o| o.trim().to_owned())
}

/// `kern.boottime`'s seconds.
fn boot_seconds() -> Result<u64, String> {
    let text = command("/usr/sbin/sysctl", &["-n", "kern.boottime"])?;
    text.split("sec = ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .and_then(|sec| sec.trim().parse().ok())
        .ok_or_else(|| format!("kern.boottime unreadable: {text:?}"))
}

/// An `osascript` that waits for `trigger` to exist, then asks loginwindow to log out. Started
/// before the GUI part, so only the event is left to send once the note holds unsaved text.
fn armed_logout(trigger: &Path) -> Result<std::process::Child, String> {
    let path = trigger.to_str().ok_or("non UTF-8 trigger path")?;
    if path.contains(['"', '\\']) {
        return Err(format!(
            "trigger path {path:?} cannot be quoted for AppleScript"
        ));
    }
    Command::new("/usr/bin/osascript")
        .args([
            "-e",
            &format!("set trigger to POSIX file \"{path}\""),
            "-e",
            "repeat",
            "-e",
            "try",
            "-e",
            "trigger as alias",
            "-e",
            "exit repeat",
            "-e",
            "end try",
            "-e",
            "delay 0.02",
            "-e",
            "end repeat",
            "-e",
            "tell application \"loginwindow\" to «event aevtrlgo»",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("osascript: {e}"))
}

/// The process identities the record carries: who coordinated, who prepared, the product, the
/// boot, and when the last key reached the note.
struct Ids {
    coordinator: u32,
    prepare: Option<u32>,
    product: Option<u32>,
    boot: u64,
    last_key: Option<i64>,
}

/// The coordinator: no CoreGraphics here, so it outlives the logout.
fn coordinate(options: &Options) -> Result<(), String> {
    if console_owner()? == "root" {
        return Err(
            "mac-logout refused: no GUI session (the console belongs to root); restart the VM"
                .into(),
        );
    }
    let boot = boot_seconds()?;
    if fs::read_to_string(BOOT_MARKER).is_ok_and(|b| b.trim() == boot.to_string()) {
        return Err(
            "mac-logout refused: a case already logged out in this boot; restart the VM".into(),
        );
    }
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
    let clock = Instant::now();
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let out = options.out.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "dist/evidence/mac-logout/{}-{started}",
            options.case
        ))
    });
    let out = crate::mac_gui::runner::output_path(&out)?;
    if out.starts_with(&app)
        || out.starts_with(app.parent().ok_or("bundle has no parent")?.join("note"))
    {
        return Err("evidence cannot be inside the input bundle or its note directory".into());
    }
    if out.join("mac-gui.json").exists() {
        return Err(format!(
            "{} already holds a mac-gui.json; each run needs its own --out",
            out.display()
        ));
    }
    fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut session = Session::new()?;
    fixture::copy_fixture(&mut session, &app, NOTE, CONFIG, &exe_hash)?;
    let dir = session.path().to_path_buf();
    let fixture_exe = fixture::contained_executable(&dir.join("StickyMD.app"))?;
    let note = dir.join("note/note.md");
    let mut row = Row::new(&format!("W12-logout-{}", options.case));
    let mut ids = Ids {
        coordinator: std::process::id(),
        prepare: None,
        product: None,
        boot,
        last_key: None,
    };
    let publish = |row: &Row, ids: &Ids, finished: bool| -> Result<(), String> {
        let optional = |v: Option<String>| v.unwrap_or_else(|| "null".into());
        let environment = format!(
            "{{\"os\":{},\"architecture\":{},\"hv_vmm_present\":\"1\",\"allow_host\":false,\"unix_seconds\":{started},\"fixture_config\":{},\"case\":{},\"boot_seconds\":{},\"processes\":{{\"coordinator\":{},\"prepare\":{},\"product\":{}}},\"last_key_us\":{}}}",
            quote(&command("/usr/bin/sw_vers", &[])?),
            quote(std::env::consts::ARCH),
            quote(CONFIG),
            quote(&options.case),
            ids.boot,
            ids.coordinator,
            optional(ids.prepare.map(|p| p.to_string())),
            optional(ids.product.map(|p| p.to_string())),
            optional(ids.last_key.map(|k| k.to_string())),
        );
        let batch = options
            .batch
            .as_deref()
            .map_or_else(String::new, |b| format!(",\"batch\":{}", quote(b)));
        let inputs = inputs
            .as_deref()
            .map_or_else(String::new, |i| format!(",\"app_inputs\":{}", quote(i)));
        let json = format!(
            "{{\"schema_version\":2,\"acceptance_status\":\"INCOMPLETE\",\"finished\":{finished}{batch},\"environment\":{environment}{inputs},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"scenarios\":[{}]}}\n",
            quote(&exe_hash),
            quote(&bundle_hash),
            quote(&driver_hash),
            row.json()
        );
        crate::atomic_evidence::write(&out.join("mac-gui.json"), json.as_bytes())
    };
    // A started record that is never taken for a finished one, whatever happens next.
    publish(&row, &ids, false)?;
    let trigger = dir.join("logout-now");
    let mut sender = armed_logout(&trigger)?;
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("prepare child: {e}"))?;
    ids.prepare = Some(child.id());
    let child = child
        .wait_with_output()
        .map_err(|e| format!("prepare child: {e}"))?;
    let stdout = String::from_utf8_lossy(&child.stdout).into_owned();
    let field = |key: &str| {
        stdout
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .map(|v| v.trim().to_owned())
    };
    ids.product = field("pid=").and_then(|p| p.parse().ok());
    ids.last_key = field("last_key_us=").and_then(|k| k.parse().ok());
    let mut facts = Facts::default();
    let executed = match ids.product {
        Some(pid) if child.status.success() => {
            facts.before = read_note(&note);
            fs::write(BOOT_MARKER, boot.to_string()).map_err(|e| format!("{BOOT_MARKER}: {e}"))?;
            fs::write(&trigger, b"").map_err(|e| format!("logout trigger: {e}"))?;
            let alive = || fixture_processes(&fixture_exe).map_or(true, |p| p.contains(&pid));
            let start = Instant::now();
            while alive() && start.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(50));
            }
            facts.gone = !alive();
            let start = Instant::now();
            while sender.try_wait().ok().flatten().is_none()
                && start.elapsed() < Duration::from_secs(10)
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            std::thread::sleep(Duration::from_secs(3));
            facts.logged_out = console_owner().is_ok_and(|owner| owner == "root");
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
    let _ = sender.kill();
    let _ = sender.wait();
    // Whatever happened, nothing of this fixture keeps running, and its directory is writable.
    let ended = end_fixture_processes(&fixture_exe);
    let restored = if options.case == "fault" {
        fs::set_permissions(dir.join("note"), fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("restore note directory: {e}"))
    } else {
        Ok(())
    };
    if executed {
        facts.after = read_note(&note);
        if options.case == "dialog" {
            facts.exported = match fs::symlink_metadata(dir.join("export.md")) {
                Ok(_) => Ok(true),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(e) => Err(e.to_string()),
            };
        }
        let minutes = clock.elapsed().as_secs() / 60 + 2;
        let log = command(
            "/usr/bin/log",
            &[
                "show",
                "--last",
                &format!("{minutes}m"),
                "--style",
                "syslog",
                "--predicate",
                "process == \"StickyMD\" OR eventMessage CONTAINS[c] \"stickymd\"",
            ],
        );
        match &log {
            Ok(log) => {
                crate::atomic_evidence::write(
                    &out.join(format!("W12-logout-{}-system.log", options.case)),
                    log.as_bytes(),
                )?;
                if let Some(pid) = ids.product {
                    from_log(log, pid, &mut facts);
                }
            }
            Err(e) => row.trace.push(super::super::model::Trace {
                ms: clock.elapsed().as_millis() as u64,
                event: format!("system log unreadable: {e}"),
            }),
        }
        if let Ok(pids) = &ended
            && !pids.is_empty()
        {
            row.trace.push(super::super::model::Trace {
                ms: clock.elapsed().as_millis() as u64,
                event: format!("still running after the logout, ended by the driver: {pids:?}"),
            });
        }
        judge(&options.case, &facts, &mut row);
    }
    if let Ok(stderr) = fs::read(dir.join("stderr.log")) {
        crate::atomic_evidence::write(
            &out.join(format!("W12-logout-{}-stderr.log", options.case)),
            &stderr,
        )?;
    }
    let cleanup = ended
        .map(|_| ())
        .and(restored)
        .and_then(|()| session.cleanup());
    // Finished only once nothing of the fixture runs and its directory is gone.
    publish(&row, &ids, cleanup.is_ok())?;
    let verdict = aggregate(&row.checks);
    println!("W12-logout-{}: {}", options.case, verdict.name());
    println!("Evidence: {}", out.join("mac-gui.json").display());
    if let Err(e) = cleanup {
        return Err(format!(
            "mac-logout cleanup failed, record left unfinished: {e}"
        ));
    }
    if verdict == Verdict::Fail || !executed {
        Err("mac-logout did not pass; see mac-gui.json".into())
    } else {
        Ok(())
    }
}
