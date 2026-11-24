//! Sequential macOS cohorts with checkpoints outside measurement intervals.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::{
    Options,
    model::{self, Run, Scenario},
    native,
    report::{Evidence, quote},
    rows::Conditions,
};
use crate::mac_common::{
    fixture::{app_inputs, bundle_hash, contained_executable, copy_fixture},
    session::Session,
};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(super) fn execute(options: &Options) -> Result<(), String> {
    let app = options
        .app
        .canonicalize()
        .map_err(|e| format!("app path: {e}"))?;
    if app.extension().is_none_or(|extension| extension != "app") || !app.is_dir() {
        return Err("--app must name a .app directory".into());
    }
    let executable = contained_executable(&app)?;
    // Hidden needs Accessibility; say so before any run rather than after the visible ones.
    if model::SCENARIOS.iter().any(|scenario| scenario.hidden)
        && !crate::mac_gui::accessibility_trusted()
    {
        return Err("Accessibility permission missing: Hidden chooses the menu-bar item through Accessibility; grant it to this binary, then measure".into());
    }
    let fixture = model::fixture();
    // The output may not reach the input bundle, its note directory or the mac-gui ledger.
    let out = crate::mac_common::output::guarded(&options.out, &app)?;
    // A first run creates its directory; an earlier campaign's evidence is never replaced
    // (review round 3, B2 and its first-run finding).
    crate::mac_common::output::prepare(&out, &|name| name == "mac-resources.json", &[])?;
    // One reading of the environment gives both the evidence and the conditions it is judged
    // under (review B3); every sample checks the displays are still these.
    let screen = Screen::now()?;
    let display = screen.dimensions;
    let [pixel_width, pixel_height, point_width, point_height] = display;
    let displays = u32::try_from(screen.active.len()).map_err(|e| e.to_string())?;
    let mut evidence = Evidence {
        environment: environment(display, displays)?,
        conditions: Conditions {
            macos_version: native::system_text(c"kern.osproductversion")?,
            scale: [pixel_width / point_width, pixel_height / point_height],
            displays,
        },
        app_hash: crate::integrity::sha256(&executable)?,
        bundle_hash: bundle_hash(&app)?,
        inputs: app_inputs(&app)?,
        driver: crate::integrity::sha256(
            &std::env::current_exe().map_err(|e| format!("driver path: {e}"))?,
        )?,
        finished: false,
        fixture_hash: crate::integrity::sha256_bytes(fixture.as_bytes())?,
        repetitions: options.repetitions,
        timebase: native::timebase()?,
        scenarios: Vec::new(),
    };
    let output = out.join("mac-resources.json");
    // Review B7: an earlier campaign's evidence is never replaced; the first write is exclusive.
    crate::atomic_evidence::write_new(&output, evidence.json().as_bytes())?;
    for scenario in model::SCENARIOS {
        let name = scenario.name;
        evidence.scenarios.push(Vec::new());
        for repetition in 1..=options.repetitions {
            println!(
                "{name} run {repetition}/{}: starting isolated measurement",
                options.repetitions
            );
            let run = one_run(&app, &fixture, scenario, repetition, &evidence, &screen);
            println!("{name} run {repetition}: {} {}", run.status, run.reason);
            let cleanup_error = run.cleanup_error.clone();
            evidence
                .scenarios
                .last_mut()
                .expect("current scenario")
                .push(run);
            crate::atomic_evidence::write(&output, evidence.json().as_bytes())?;
            if let Some(error) = cleanup_error {
                evidence.print_summary();
                return Err(format!(
                    "Cleanup failed; stopped before launching another run: {error}"
                ));
            }
        }
    }
    // Every run ran: only now is the campaign finished (a cut-short one returned above).
    evidence.finished = true;
    crate::atomic_evidence::write(&output, evidence.json().as_bytes())?;
    evidence.print_summary();
    println!("Evidence: {}", output.display());
    if evidence.measured() {
        Ok(())
    } else {
        Err("P16-R01 or P16-R02 does not hold: every run must be valid and within its memory gate, and each scenario's median idle CPU at most 0.1%; see mac-resources.json".into())
    }
}

/// The displays a campaign is measured on: the main display's mode and size, its ID and the
/// active displays' IDs.
#[derive(Debug, PartialEq)]
struct Screen {
    dimensions: [f64; 4],
    main: u32,
    active: Vec<u32>,
}

impl Screen {
    fn now() -> Result<Self, String> {
        let (main, active) = native::displays()?;
        Ok(Self {
            dimensions: native::display()?,
            main,
            active,
        })
    }
}

fn one_run(
    app: &Path,
    fixture: &str,
    scenario: Scenario,
    repetition: usize,
    evidence: &Evidence,
    screen: &Screen,
) -> Run {
    let timebase = evidence.timebase;
    let mut run = Run::new(repetition);
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(error) => {
            run.reason = error;
            return run;
        }
    };
    let result = prepare(&mut session, app, fixture, scenario.mode, evidence)
        .and_then(|()| measure(&mut session, &mut run, timebase, scenario.hidden, screen));
    if let Err(error) = result {
        run.reason = error;
    }
    if let Err(error) = session.cleanup() {
        run.cleanup_error = Some(error);
        run.status = if run.status == "EXITED_EARLY" {
            "EXITED_EARLY"
        } else {
            "ERROR"
        };
    }
    run
}

/// Copies the bundle and its fixture and starts it. The copy must be the bundle the evidence
/// names: its executable (checked while copying), its manifest and its input fingerprint, so a
/// bundle changed during the campaign is never measured under the first one's identity.
fn prepare(
    session: &mut Session,
    app: &Path,
    fixture: &str,
    mode: &str,
    evidence: &Evidence,
) -> Result<(), String> {
    let executable = copy_fixture(
        session,
        app,
        fixture,
        &model::config(mode),
        &evidence.app_hash,
    )?;
    let copied = session.path().join("StickyMD.app");
    if bundle_hash(&copied)? != evidence.bundle_hash || app_inputs(&copied)? != evidence.inputs {
        return Err(
            "the copied bundle is not the one this campaign measures; refused before launch".into(),
        );
    }
    session.spawn(
        Command::new(executable)
            .current_dir(session.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
}

fn exited(session: &mut Session, run: &mut Run, started: Instant) -> Result<bool, String> {
    if let Some(status) = session.poll()? {
        run.exited(started.elapsed().as_secs_f64(), status.to_string());
        return Ok(true);
    }
    Ok(false)
}

fn wait(
    session: &mut Session,
    run: &mut Run,
    started: Instant,
    deadline: Instant,
) -> Result<bool, String> {
    if let Some(status) = session.wait_until(deadline)? {
        run.exited(started.elapsed().as_secs_f64(), status.to_string());
        return Ok(false);
    }
    Ok(true)
}

fn capture(
    session: &mut Session,
    run: &mut Run,
    started: Instant,
    screen: &Screen,
) -> Result<bool, String> {
    if exited(session, run, started)? {
        return Ok(false);
    }
    // The scale and the single display the campaign records must be the ones measured under
    // (review B3 and round 3): the main display's mode, its ID and the set of active displays.
    if Screen::now()? != *screen {
        run.status = "DISPLAY_CHANGED";
        return Err("the displays changed during the campaign".into());
    }
    let result = native::sample(run.pid.ok_or("Missing measured PID")?, started);
    let error = match result {
        Ok(sample) => {
            run.samples.push(sample);
            None
        }
        Err(error) => Some(error),
    };
    // A process can exit between try_wait and the native query. Never derive metrics from it.
    if exited(session, run, started)? {
        return Ok(false);
    }
    if let Some(error) = error {
        return Err(error);
    }
    Ok(true)
}

fn measure(
    session: &mut Session,
    run: &mut Run,
    timebase: (u32, u32),
    hidden: bool,
    screen: &Screen,
) -> Result<(), String> {
    let started = Instant::now();
    let pid = session.child.as_ref().ok_or("Missing child")?.id();
    run.pid = Some(pid);
    loop {
        if exited(session, run, started)? {
            return Ok(());
        }
        // One instant decides both readiness and the deadline (review round 3): a window first
        // seen after 30 s is a timeout, never a run the reader would refuse.
        let ready = native::window_ready(pid)?;
        let at = started.elapsed();
        if ready && at <= Duration::from_secs(30) {
            run.ready_seconds = Some(at.as_secs_f64());
            break;
        }
        if at >= Duration::from_secs(30) {
            run.status = "READINESS_TIMEOUT";
            return Err("No PID-owned on-screen content window within 30 seconds".into());
        }
        if !wait(
            session,
            run,
            started,
            Instant::now() + Duration::from_millis(100),
        )? {
            return Ok(());
        }
    }
    if hidden && !hide(session, run, pid, started)? {
        return Ok(());
    }
    if !wait(
        session,
        run,
        started,
        Instant::now() + Duration::from_secs(crate::resource_plan::WARMUP_SECONDS),
    )? || !capture(session, run, started, screen)?
    {
        return Ok(());
    }
    if hidden {
        stays_hidden(run, pid)?;
    }
    let deadline = started
        + Duration::from_secs_f64(run.samples[0].elapsed_seconds)
        + Duration::from_secs(crate::resource_plan::CPU_SECONDS);
    if !wait(session, run, started, deadline)? || !capture(session, run, started, screen)? {
        return Ok(());
    }
    if hidden {
        stays_hidden(run, pid)?;
    }
    run.metrics = Some(model::metrics(
        &run.samples[0],
        &run.samples[1],
        timebase.0,
        timebase.1,
    )?);
    run.status = "VALID";
    Ok(())
}

/// Hides the ready window through the menu-bar menu, as a person would, and waits until it is off
/// screen. False means the product exited meanwhile, recorded in `run`.
fn hide(session: &mut Session, run: &mut Run, pid: u32, started: Instant) -> Result<bool, String> {
    let chosen = crate::mac_gui::choose_status_menu_item(pid, model::HIDE_ITEM);
    // An Accessibility error can mean the product exited meanwhile; record that exit first.
    if exited(session, run, started)? {
        return Ok(false);
    }
    run.status_item_wait_seconds = Some(chosen?.as_secs_f64());
    let deadline = Instant::now() + Duration::from_secs(model::HIDE_SECONDS);
    while native::window_ready(pid)? {
        if Instant::now() >= deadline {
            run.status = "HIDE_TIMEOUT";
            return Err(format!(
                "The window stayed on screen {} s after the menu-bar {}",
                model::HIDE_SECONDS,
                model::HIDE_ITEM
            ));
        }
        if !wait(
            session,
            run,
            started,
            Instant::now() + Duration::from_millis(100),
        )? {
            return Ok(false);
        }
    }
    run.hidden_seconds = Some(started.elapsed().as_secs_f64());
    Ok(true)
}

/// A hidden run's samples count only while its content window is still off screen: one off-screen
/// observation before the warm-up does not prove the window stayed hidden.
fn stays_hidden(run: &mut Run, pid: u32) -> Result<(), String> {
    if native::window_ready(pid)? {
        run.status = "NOT_HIDDEN";
        return Err("The content window was on screen again at a hidden sample".into());
    }
    Ok(())
}

/// `sysctl -n <name>` trimmed, or `unavailable`; as `mac-gui` reads `kern.hv_vmm_present`.
fn sysctl(name: &str) -> String {
    Command::new("/usr/sbin/sysctl")
        .args(["-n", name])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unavailable".into(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )
}

fn environment(display: [f64; 4], displays: u32) -> Result<String, String> {
    let [pixel_width, pixel_height, point_width, point_height] = display;
    // The ledger takes the system from `sw_vers` as it does for mac-gui runs, so one image digest
    // names one system across both tools.
    let sw_vers = Command::new("/usr/bin/sw_vers")
        .output()
        .map_err(|e| format!("sw_vers: {e}"))?;
    if !sw_vers.status.success() {
        return Err("sw_vers failed".into());
    }
    Ok(format!(
        "{{\"os\":{},\"hv_vmm_present\":{},\"display_count\":{displays},\"macos_version\":{},\"os_build\":{},\"model\":{},\"cpu\":{},\"ram_bytes\":{},\"architecture\":{},\"unix_seconds\":{},\"main_display\":{{\"pixel_width\":{},\"pixel_height\":{},\"point_width\":{},\"point_height\":{},\"scale_x\":{},\"scale_y\":{}}},\"release_build\":\"caller-supplied; not inferred from binary\",\"desktop_conditions\":\"caller must keep desktop unlocked, display on, and avoid interaction\"}}",
        quote(&String::from_utf8_lossy(&sw_vers.stdout)),
        quote(&sysctl("kern.hv_vmm_present")),
        quote(&native::system_text(c"kern.osproductversion")?),
        quote(&native::system_text(c"kern.osversion")?),
        quote(&native::system_text(c"hw.model")?),
        quote(&native::system_text(c"machdep.cpu.brand_string")?),
        native::ram_bytes()?,
        quote(std::env::consts::ARCH),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs(),
        pixel_width,
        pixel_height,
        point_width,
        point_height,
        pixel_width / point_width,
        pixel_height / point_height
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn exited_child_is_recorded_before_any_window_query_or_sampling() {
        let mut session = super::Session::new().unwrap();
        session
            .spawn(&mut std::process::Command::new("/usr/bin/false"))
            .unwrap();
        session
            .wait_until(std::time::Instant::now() + std::time::Duration::from_secs(3))
            .unwrap()
            .unwrap();
        let mut run = super::Run::new(1);
        super::measure(&mut session, &mut run, (1, 1), false, [1.0; 4]).unwrap();
        assert_eq!(run.status, "EXITED_EARLY");
        assert!(run.samples.is_empty() && run.metrics.is_none());
        assert!(run.exit_status.as_deref().unwrap().contains('1'));
        session.cleanup().unwrap();
    }
}
