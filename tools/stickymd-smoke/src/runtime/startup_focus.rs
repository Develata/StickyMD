//! Observe initial activation before any harness input can hide a focus failure.
//! plan_ref: docs/plan/09_windows_shell.md#tool-window-identity

use super::*;
use crate::window_control::{self, WindowActivationFacts};

#[test]
#[ignore = "exclusive desktop and STICKYMD_SMOKE_PROBE_REPOSITORY; initial focus only, diagnostic"]
fn native_startup_focus_diagnostic() {
    native_diagnostics::diagnose("startup-focus", run);
}

fn run(repository: &Path, root: &Path) -> Result<RuntimeEvidence, String> {
    let source = crate::qualification::release_executable(repository)?;
    // Share the trace archive convention with the startup timing diagnostic.
    let traces = root.join("phase9-startup");
    fs::create_dir(&traces).map_err(|error| error.to_string())?;
    let mut output = RuntimeEvidence::passed(Vec::new());
    let result = (|| {
        for (view_index, view) in ["source", "preview", "split"].into_iter().enumerate() {
            for run in 1..=3 {
                let directory = root.join(format!("启动 {view}-{run}"));
                let executable = copy_executable(&source, &directory)?;
                prepare_resource_layout(&directory, view, 0, 0, ImageResourceFixture::None)?;
                let sequence = (view_index * 3 + run) as u64;
                observe(
                    &executable,
                    &directory,
                    &traces,
                    view,
                    run,
                    sequence,
                    &mut output,
                )?;
            }
        }
        Ok(())
    })();
    output.gate_failure = result.err();
    Ok(output)
}

fn observe(
    executable: &Path,
    directory: &Path,
    traces: &Path,
    view: &str,
    run: usize,
    sequence: u64,
    output: &mut RuntimeEvidence,
) -> Result<(), String> {
    let ready = ReadyEvent::create(sequence)?;
    let trace = traces.join(format!("startup-trace-{sequence}.txt"));
    let mut command = startup_command(executable, directory, ready.name(), &trace);
    command.env_remove("STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY");
    let mut child = ChildGuard::spawn(&mut command, "cannot start initial-focus probe")?;
    ready.wait(START_TIMEOUT)?;
    ensure_alive(&mut child, "initial-focus probe")?;
    let window = window_control::visible_window(child.id())?;
    // No focus helper, mouse, keyboard, clipboard or view-switch command may run
    // before this observation. The fixture selects the initial view on disk.
    let activation = window_control::activation_facts(window)?;
    output.samples.push(EvidenceSample {
        cohort: view.into(),
        run,
        shared_from: None,
        measurements: [
            ("foreground", activation.foreground),
            ("active", activation.active),
            ("focused", activation.focused),
            ("captured", activation.captured),
        ]
        .into_iter()
        .map(|(name, value)| EvidenceMeasurement {
            name: name.into(),
            unit: "bool".into(),
            value: f64::from(value),
        })
        .collect(),
    });
    eprintln!("startup focus view={view} run={run} {activation:?}");
    require_initial_focus(activation)?;
    wait_for_tool_window_style(window)?;
    wait_for_startup_trace(&trace)?;
    // ChildGuard owns this copied process on every success/error path.
    Ok(())
}

fn require_initial_focus(activation: WindowActivationFacts) -> Result<(), String> {
    if activation.foreground && activation.active && activation.focused && !activation.captured {
        Ok(())
    } else {
        Err(format!(
            "startup focus before harness input: {activation:?}"
        ))
    }
}

#[test]
fn ready_signal_does_not_substitute_for_observed_initial_focus() {
    let valid = WindowActivationFacts {
        foreground: true,
        active: true,
        focused: true,
        captured: false,
    };
    require_initial_focus(valid).unwrap();
    for invalid in [
        WindowActivationFacts {
            foreground: false,
            ..valid
        },
        WindowActivationFacts {
            active: false,
            ..valid
        },
        WindowActivationFacts {
            focused: false,
            ..valid
        },
        WindowActivationFacts {
            captured: true,
            ..valid
        },
    ] {
        assert!(require_initial_focus(invalid).is_err());
    }
}
