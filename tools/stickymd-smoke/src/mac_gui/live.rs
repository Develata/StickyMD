//! External-only backend. Fixtures are frozen before launch; runtime files are read only.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, Injection, SHIFT},
    model::*,
    native::{
        accessibility as ax,
        events::{self, Events},
        windows,
    },
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
    fn open(&self) -> Result<String, String> {
        let bundle = self.session.path().join("StickyMD.app");
        command(
            "/usr/bin/open",
            &[bundle.to_str().ok_or("non UTF-8 bundle path")?],
        )
    }
    fn panel(&mut self, confirm: bool) -> Result<String, String> {
        events::wait(300)?;
        if !confirm {
            return ax::panel_button(self.pid, false);
        }
        self.input.chord(&[CMD, SHIFT, 5])?;
        events::wait(300)?;
        let mut log = ax::folder(
            self.pid,
            self.session
                .path()
                .to_str()
                .ok_or("non UTF-8 export path")?,
        )?;
        self.input.chord(&[36])?;
        events::wait(400)?;
        log.push_str(&ax::filename(self.pid, "export.md")?);
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
                self.input.drag(a, b)?;
                Value::Unit
            }
            Op::Click(p) => {
                self.input.click(p)?;
                Value::Unit
            }
            Op::Chord(keys) => {
                self.input.chord(&keys)?;
                events::wait(60)?;
                Value::Unit
            }
            Op::Text(s) => {
                self.input.text(&s)?;
                events::wait(60)?;
                Value::Unit
            }
            Op::Menu(label) => Value::Text(ax::menu(self.pid, &label)?),
            Op::Config => Value::Text(self.read("note/config.toml")?),
            Op::Note => Value::Text(self.quiet_note()?),
            Op::Alive => match self.session.poll()? {
                None => Value::Bool(true),
                Some(status) if status.success() => Value::Bool(false),
                Some(status) => return Err(format!("product exited unsuccessfully: {status}")),
            },
            Op::Reopen => Value::Text(self.open()?),
            Op::AppType => Value::Text(command(
                "/usr/bin/lsappinfo",
                &[
                    "info",
                    "-only",
                    "ApplicationType",
                    &format!("pid={}", self.pid),
                ],
            )?),
            Op::Dock => {
                let pid = command("/usr/bin/pgrep", &["-x", "Dock"])?
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| "ambiguous Dock PID")?;
                Value::Text(ax::dock_titles(pid)?)
            }
            Op::Clipboard => Value::Text(command("/usr/bin/pbpaste", &["-Prefer", "txt"])?),
            Op::Panel(confirm) => Value::Text(self.panel(confirm)?),
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
