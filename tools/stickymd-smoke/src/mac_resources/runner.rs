//! Sequential macOS cohorts with checkpoints outside measurement intervals.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::{
    Options,
    model::{self, Run},
    native,
    report::{Evidence, quote},
};
use crate::mac_common::{
    fixture::{contained_executable, copy_fixture},
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
    let fixture = model::fixture();
    let mut evidence = Evidence {
        environment: environment()?,
        app_hash: crate::integrity::sha256(&executable)?,
        fixture_hash: crate::integrity::sha256_bytes(fixture.as_bytes())?,
        repetitions: options.repetitions,
        timebase: native::timebase()?,
        scenarios: Vec::new(),
    };
    let output = options.out.join("mac-resources.json");
    crate::atomic_evidence::write(&output, evidence.json().as_bytes())?;
    for (mode, _) in model::SCENARIOS {
        evidence.scenarios.push(Vec::new());
        for repetition in 1..=options.repetitions {
            println!(
                "{mode} run {repetition}/{}: starting isolated measurement",
                options.repetitions
            );
            let run = one_run(
                &app,
                &fixture,
                mode,
                repetition,
                &evidence.app_hash,
                evidence.timebase,
            );
            println!("{mode} run {repetition}: {} {}", run.status, run.reason);
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
    evidence.print_summary();
    println!("Evidence: {}", output.display());
    if evidence.passed() {
        Ok(())
    } else {
        Err("Visible scenarios incomplete or failed: every requested run must be valid and within its memory gate; see mac-resources.json".into())
    }
}

fn one_run(
    app: &Path,
    fixture: &str,
    mode: &str,
    repetition: usize,
    hash: &str,
    timebase: (u32, u32),
) -> Run {
    let mut run = Run::new(repetition);
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(error) => {
            run.reason = error;
            return run;
        }
    };
    let result = prepare(&mut session, app, fixture, mode, hash)
        .and_then(|()| measure(&mut session, &mut run, timebase));
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

fn prepare(
    session: &mut Session,
    app: &Path,
    fixture: &str,
    mode: &str,
    hash: &str,
) -> Result<(), String> {
    let executable = copy_fixture(session, app, fixture, &model::config(mode), hash)?;
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

fn capture(session: &mut Session, run: &mut Run, started: Instant) -> Result<bool, String> {
    if exited(session, run, started)? {
        return Ok(false);
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

fn measure(session: &mut Session, run: &mut Run, timebase: (u32, u32)) -> Result<(), String> {
    let started = Instant::now();
    let pid = session.child.as_ref().ok_or("Missing child")?.id();
    run.pid = Some(pid);
    loop {
        if exited(session, run, started)? {
            return Ok(());
        }
        if native::window_ready(pid)? {
            break;
        }
        if started.elapsed() >= Duration::from_secs(30) {
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
    run.ready_seconds = Some(started.elapsed().as_secs_f64());
    if !wait(
        session,
        run,
        started,
        Instant::now() + Duration::from_secs(crate::resource_plan::WARMUP_SECONDS),
    )? || !capture(session, run, started)?
    {
        return Ok(());
    }
    let deadline = started
        + Duration::from_secs_f64(run.samples[0].elapsed_seconds)
        + Duration::from_secs(crate::resource_plan::CPU_SECONDS);
    if !wait(session, run, started, deadline)? || !capture(session, run, started)? {
        return Ok(());
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

fn environment() -> Result<String, String> {
    let [pixel_width, pixel_height, point_width, point_height] = native::display()?;
    Ok(format!(
        "{{\"macos_version\":{},\"os_build\":{},\"model\":{},\"cpu\":{},\"ram_bytes\":{},\"architecture\":{},\"unix_seconds\":{},\"main_display\":{{\"pixel_width\":{},\"pixel_height\":{},\"point_width\":{},\"point_height\":{},\"scale_x\":{},\"scale_y\":{}}},\"release_build\":\"caller-supplied; not inferred from binary\",\"desktop_conditions\":\"caller must keep desktop unlocked, display on, and avoid interaction\"}}",
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
        super::measure(&mut session, &mut run, (1, 1)).unwrap();
        assert_eq!(run.status, "EXITED_EARLY");
        assert!(run.samples.is_empty() && run.metrics.is_none());
        assert!(run.exit_status.as_deref().unwrap().contains('1'));
        session.cleanup().unwrap();
    }
}
