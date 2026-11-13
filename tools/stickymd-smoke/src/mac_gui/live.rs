//! External-only backend. Fixtures are frozen before launch; runtime files are read only. The one
//! change is W11/W12's save fault (USER 2026-11-11): the fixture's note directory loses its write
//! permission, and the runner gives it back before cleanup on every path.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, Injection, SHIFT},
    model::*,
    native::{
        accessibility as ax,
        events::{self, Events},
        windows,
    },
    observation::KeyTarget,
    scenarios::{Backend, Op, Value},
};
use crate::mac_common::session::Session;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub(super) struct Live<'a> {
    pub session: &'a mut Session,
    pub input: &'a mut Injection<Events>,
    pub pid: u32,
    pub window_id: u32,
    pub out: &'a Path,
    pub permissions: Permissions,
    /// The copied product, for a scenario that starts it again after it quit.
    pub executable: std::path::PathBuf,
}
pub(super) fn command(program: &str, args: &[&str]) -> Result<String, String> {
    // Reuse the same owned child/timeout/identity cleanup for helper processes too.
    let mut session = Session::new()?;
    let stdout = session.path().join("stdout");
    let stderr = session.path().join("stderr");
    let out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stdout)
        .map_err(|e| e.to_string())?;
    let err = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stderr)
        .map_err(|e| e.to_string())?;
    session.spawn(
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err),
    )?;
    let status = session
        .wait_until(Instant::now() + Duration::from_secs(10))?
        .ok_or_else(|| format!("{program} timed out"))?;
    let text = fs::read_to_string(stdout).map_err(|e| e.to_string())?;
    let error = fs::read_to_string(stderr).map_err(|e| e.to_string())?;
    session.cleanup()?;
    if status.success() {
        Ok(text)
    } else {
        Err(format!("{program}: {status}: {error}"))
    }
}
impl Live<'_> {
    fn read(&self, path: &str) -> Result<String, String> {
        fs::read_to_string(self.session.path().join(path)).map_err(|e| format!("{path}: {e}"))
    }
    fn quiet_note(&self) -> Result<String, String> {
        // Full bytes must stay unchanged for 1000ms, after at least 1500ms for debounce/save.
        events::wait(1500)?;
        let start = Instant::now();
        let mut stable = Instant::now();
        let mut previous = self.read("note/note.md")?;
        loop {
            events::wait(100)?;
            let next = self.read("note/note.md")?;
            if next != previous {
                previous = next;
                stable = Instant::now();
            }
            if stable.elapsed() >= Duration::from_millis(1000) {
                return Ok(previous);
            }
            if start.elapsed() > Duration::from_secs(8) {
                return Err("note never reached 1s quiet period".into());
            }
        }
    }
    /// Plan 11 (physical input): a press goes only where the system's hit test finds the
    /// product. A covering window (Tips, a system dialog) would take the press and make the
    /// product look unresponsive. Stacking can still change between this check and the press,
    /// so the product's observed reaction stays the evidence.
    fn require_target(&self, p: Point) -> Result<(), String> {
        let owner = ax::pid_at(p.x, p.y)?;
        if owner == self.pid {
            return Ok(());
        }
        let name = windows::owner_name(owner).unwrap_or_default();
        Err(format!(
            "press at ({}, {}) would reach PID {owner} {name:?}, not the product (PID {}); \
             the desktop is obstructed and no press was injected",
            p.x, p.y, self.pid
        ))
    }
    /// Plan 11 (physical input): keys go only to a target window that is frontmost and focused.
    /// Checked before every key press, typed text included; a mismatch or an unreadable focus
    /// injects nothing.
    fn require_key_target(&self, target: KeyTarget) -> Result<(), String> {
        super::observation::key_target(ax::front(), ax::focused_window(self.pid), self.pid, target)
    }
    /// Points the open save dialog at `<session>/export.md` without answering it, and reads the
    /// destination back: the name field holds `export.md`, the location popup names the session
    /// directory, and the Go to Folder sheet is gone. Anything else is an error, not an aim.
    fn target_panel(&mut self) -> Result<String, String> {
        self.require_key_target(KeyTarget::Dialog)?;
        self.input.chord(&[CMD, SHIFT, 5])?;
        events::wait(300)?;
        let path = self
            .session
            .path()
            .to_str()
            .ok_or("non UTF-8 export path")?
            .to_owned();
        let mut log = ax::folder(self.pid, &path)?;
        // Return confirms whatever has focus: only the Go to Folder path field may get it.
        super::observation::goto_field(
            ax::front(),
            ax::goto_sheet_open(self.pid),
            ax::focused_element(self.pid),
            self.pid,
            &path,
        )?;
        self.input.chord(&[36])?;
        events::wait(400)?;
        log.push_str(&ax::filename(self.pid, "export.md")?);
        events::wait(200)?;
        let directory = self
            .session
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("non UTF-8 session directory")?;
        let (name, location, sheet) = ax::panel_destination(self.pid)?;
        if name != "export.md" || location != directory || sheet {
            return Err(format!(
                "dialog not aimed: name={name:?}, location={location:?} (want {directory:?}), \
                 go-to sheet open={sheet}"
            ));
        }
        log.push_str(&format!("\naimed at {directory}/{name}"));
        Ok(log)
    }
    fn open(&mut self) -> Result<String, String> {
        self.session.open_bundle(|bundle| {
            command(
                "/usr/bin/open",
                &[bundle.to_str().ok_or("non UTF-8 bundle path")?],
            )
        })
    }
    fn panel(&mut self, confirm: bool) -> Result<String, String> {
        events::wait(300)?;
        if !confirm {
            return ax::panel_button(self.pid, false);
        }
        let mut log = self.target_panel()?;
        log.push_str(&ax::panel_button(self.pid, true)?);
        Ok(log)
    }
}
impl Backend for Live<'_> {
    fn now(&self) -> u64 {
        self.input.backend.started.elapsed().as_millis() as u64
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        events::running()?;
        self.input.require_running()?;
        Ok(match op {
            Op::Wait(ms) => {
                events::wait(ms)?;
                Value::Unit
            }
            Op::Window => Value::Window(windows::window(self.pid, Some(self.window_id))?),
            Op::Screen => Value::Rect(windows::screen()),
            Op::Front => Value::Pid(ax::front()?),
            Op::Finder => {
                if let Some(status) = self.session.poll()? {
                    return Err(format!("refused open Finder: owned child exited: {status}"));
                }
                command("/usr/bin/open", &["-a", "Finder"])?;
                events::wait(200)?;
                let finder = command("/usr/bin/pgrep", &["-x", "Finder"])?;
                let pid = finder
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| "ambiguous Finder PID")?;
                if ax::front()? != pid {
                    return Err("Finder did not become frontmost".into());
                }
                Value::Pid(pid)
            }
            Op::Activate => {
                self.open()?;
                events::wait(300)?;
                ax::activate(self.pid)?;
                events::wait(100)?;
                if ax::front()? != self.pid {
                    return Err("product did not acquire focus".into());
                }
                Value::Unit
            }
            Op::Move(p) => {
                self.input.move_to(p)?;
                Value::Unit
            }
            Op::Drag(a, b) => {
                self.require_target(a)?;
                self.input.drag(a, b)?;
                Value::Unit
            }
            Op::Click(p) => {
                self.require_target(p)?;
                self.input.click(p)?;
                Value::Unit
            }
            Op::Chord(keys) => {
                self.require_key_target(KeyTarget::Note)?;
                self.input.chord(&keys)?;
                events::wait(60)?;
                Value::Unit
            }
            Op::Text(s) => {
                // Checked before each character: focus taken mid-text stops the rest.
                for character in s.chars() {
                    self.require_key_target(KeyTarget::Note)?;
                    self.input.text(character.encode_utf8(&mut [0; 4]))?;
                }
                let last_key = self
                    .input
                    .backend
                    .last_key_ms
                    .ok_or("no typed key timestamp")?;
                events::wait(60)?;
                Value::Time(last_key)
            }
            Op::Menu(label) => {
                let (waited, detail) = ax::menu(self.pid, &label)?;
                Value::Text(format!(
                    "status item in AX after {} ms; {detail}",
                    waited.as_millis()
                ))
            }
            Op::Config => Value::Text(self.read("note/config.toml")?),
            Op::Note => Value::Text(self.quiet_note()?),
            Op::NoteNow => Value::Text(self.read("note/note.md")?),
            Op::Alive => match self.session.poll()? {
                None => Value::Bool(true),
                Some(status) if status.success() => Value::Bool(false),
                Some(status) => return Err(format!("product exited unsuccessfully: {status}")),
            },
            Op::Reopen => Value::Text(self.open()?),
            Op::AppType => {
                let found = command(
                    "/usr/bin/lsappinfo",
                    &["find", &format!("pid={}", self.pid)],
                )?;
                let asn = super::observation::lsappinfo_asn(&found)?;
                Value::Text(command(
                    "/usr/bin/lsappinfo",
                    &["info", "-only", "ApplicationType", &asn],
                )?)
            }
            Op::Dock => {
                let pid = command("/usr/bin/pgrep", &["-x", "Dock"])?
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| "ambiguous Dock PID")?;
                Value::Text(ax::dock_titles(pid)?)
            }
            Op::Clipboard => Value::Text(command("/usr/bin/pbpaste", &["-Prefer", "txt"])?),
            Op::Panel(confirm) => Value::Text(self.panel(confirm)?),
            Op::PanelTarget => {
                events::wait(300)?;
                Value::Text(self.target_panel()?)
            }
            Op::PanelOpen => Value::Bool(ax::panel_open(self.pid)?),
            Op::Export => {
                let path = self.session.path().join("export.md");
                Value::Text(match fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => "<absent>".into(),
                    Err(e) => return Err(e.to_string()),
                })
            }
            Op::Fonts => Value::Text(self.read("stderr.log")?),
            Op::Fault(on) => {
                super::fault::note_writable(&self.session.anchor(), !on)?;
                Value::Unit
            }
            Op::Title => Value::Text(ax::window_titles(self.pid)?),
            Op::Terminate => {
                if let Some(status) = self.session.poll()? {
                    return Err(format!("refused terminate: owned child exited: {status}"));
                }
                // The request goes to the owned PID, so it cannot start another copy as a request by
                // name could. That it needs no Automation consent is only observed (no prompt in the
                // test VM), not established for every environment.
                let script = format!(
                    "ObjC.import('AppKit'); $.NSRunningApplication.runningApplicationWithProcessIdentifier({}).terminate",
                    self.pid
                );
                Value::Text(command(
                    "/usr/bin/osascript",
                    &["-l", "JavaScript", "-e", &script],
                )?)
            }
            Op::Relaunch => {
                match self.session.poll()? {
                    Some(status) if status.success() => {}
                    Some(status) => {
                        return Err(format!("refused relaunch: product exited {status}"));
                    }
                    None => return Err("refused relaunch: the product is still running".into()),
                }
                self.session.stop()?;
                let (pid, window) = super::runner::launch(self.session, &self.executable, true)?;
                self.pid = pid;
                self.window_id = window.id;
                Value::Pid(pid)
            }
            // What launchd sends at a logout to an instance still running
            // (RISK-macos-logout-ui-element).
            Op::Sigterm => {
                self.session.signal_terminate()?;
                Value::Unit
            }
            Op::Focus => {
                // No wait here: W06 samples the window from the moment focus is asked for.
                ax::activate(self.pid)?;
                Value::Unit
            }
            Op::Mapped => Value::Text(command(
                "/usr/sbin/lsof",
                &[
                    "-n",
                    "-P",
                    "-a",
                    "-p",
                    &self.pid.to_string(),
                    "-d",
                    "txt",
                    "-F",
                    "n",
                ],
            )?),
            Op::Screenshot => {
                if !self.permissions.screen {
                    Value::Text("NOT_TESTED screenshot: Screen Recording (CGPreflightScreenCaptureAccess) not granted".into())
                } else {
                    let w = windows::window(self.pid, Some(self.window_id))?;
                    let path = self.out.join("F01.ppm");
                    crate::atomic_evidence::write(&path, &windows::screenshot(&w)?)?;
                    Value::Text(format!(
                        "screenshot {} sha256={}",
                        path.display(),
                        crate::integrity::sha256(&path)?
                    ))
                }
            }
        })
    }
}
