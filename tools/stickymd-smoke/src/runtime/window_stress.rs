//! Phase 14 hidden-window stress reduction and diagnostic reporting.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use crate::cli::{WindowStressOptions, WindowStressScenario};

/// Run the hidden-window stress reducer without collecting the long-duration
/// resource samples. Each run uses a fresh copied executable and portable
/// directory so failures cannot inherit another run's shell state.
pub(crate) fn run(repository: &Path, options: WindowStressOptions) -> Result<(), String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let root = create_smoke_root()?;
    let mut failures = Vec::new();
    runtime_report!(
        "window-stress begin scenario={} runs={} collapse_cycles={} tray_cycles={} control_cycles={} persistence_cycles={}",
        options.scenario.as_str(),
        options.runs,
        options.collapse_cycles,
        options.tray_cycles,
        options.control_cycles,
        options.persistence_cycles,
    );
    for run in 1..=options.runs {
        let directory = root.join(format!("window-stress-{run}"));
        let result = run_once(&source, &directory, options);
        match result {
            Ok(()) => runtime_report!(
                "window-stress result run={run} scenario={} status=PASS",
                options.scenario.as_str()
            ),
            Err(error) => {
                runtime_report!(
                    "window-stress result run={run} scenario={} status=FAIL detail={error}",
                    options.scenario.as_str()
                );
                failures.push(format!("run {run}: {error}"));
            }
        }
    }
    if let Err(error) = cleanup_root(&root) {
        failures.push(format!("diagnostic cleanup: {error}"));
    }
    if failures.is_empty() {
        runtime_report!(
            "window-stress summary scenario={} passed={} failed=0",
            options.scenario.as_str(),
            options.runs
        );
        Ok(())
    } else {
        Err(format!(
            "window-stress summary scenario={} passed={} failed={} failures=[{}]",
            options.scenario.as_str(),
            options.runs.saturating_sub(failures.len()),
            failures.len(),
            failures.join(" | ")
        ))
    }
}

fn run_once(source: &Path, directory: &Path, options: WindowStressOptions) -> Result<(), String> {
    let executable = copy_executable(source, directory)?;
    prepare_resource_layout(directory, "source", 0, 0, ImageResourceFixture::None)?;
    let mut child = start(&executable)?;
    let result = (|| {
        wait_for_layout(directory)?;
        let window = crate::window_control::visible_window(child.id())?;
        wait_for_shell_state(window, ShellStateExpectation::Visible, START_TIMEOUT)?;
        prepare_primary_left_expanded(window, directory)?;

        if matches!(
            options.scenario,
            WindowStressScenario::Collapse
                | WindowStressScenario::CollapseTray
                | WindowStressScenario::Combined
        ) {
            run_collapse(window, options.collapse_cycles)?;
        }
        if matches!(
            options.scenario,
            WindowStressScenario::Tray
                | WindowStressScenario::CollapseTray
                | WindowStressScenario::Combined
        ) {
            run_tray(&executable, window, options.tray_cycles)?;
        }
        if matches!(
            options.scenario,
            WindowStressScenario::Controls | WindowStressScenario::Combined
        ) && let Some(opacity) = run_controls(window, options.control_cycles)?
        {
            wait_for_config_field(directory, &format!("opacity = {opacity}"))?;
        }
        run_external_reload(directory, &mut child, window, options.persistence_cycles)?;
        ensure_alive(&mut child, "window-stress instance")
    })();
    stop_child(&mut child);
    result
}

fn prepare_primary_left_expanded(
    window: crate::window_control::WindowHandle,
    directory: &Path,
) -> Result<(), String> {
    crate::window_control::move_to_primary_left_edge(window)?;
    wait_for_config_field(directory, "dock_edge = \"left\"")?;
    crate::window_control::park_cursor_at_primary_right(window)?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Collapse)?;
    wait_for_shell_state(
        window,
        ShellStateExpectation::PrimaryEdgeCollapsed(crate::window_control::PrimaryDockEdge::Left),
        START_TIMEOUT,
    )?;
    reveal_primary_left_and_wait(window)
}

fn run_collapse(window: crate::window_control::WindowHandle, cycles: usize) -> Result<(), String> {
    for cycle in 0..cycles {
        crate::window_control::park_cursor_at_primary_right(window)?;
        crate::window_control::click_toolbar(
            window,
            crate::window_control::ToolbarControl::Collapse,
        )?;
        wait_for_shell_state(
            window,
            ShellStateExpectation::PrimaryEdgeCollapsed(
                crate::window_control::PrimaryDockEdge::Left,
            ),
            START_TIMEOUT,
        )
        .map_err(|error| format!("stage=collapse cycle={} {error}", cycle + 1))?;
        reveal_primary_left_and_wait(window)
            .map_err(|error| format!("stage=expand cycle={} {error}", cycle + 1))?;
    }
    Ok(())
}

fn run_tray(
    executable: &Path,
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<(), String> {
    for cycle in 0..cycles {
        crate::window_control::request_close(window)?;
        wait_for_shell_state(window, ShellStateExpectation::Hidden, START_TIMEOUT)
            .map_err(|error| format!("stage=tray-hide cycle={} {error}", cycle + 1))?;
        let mut secondary = start(executable)?;
        let status = wait_for_exit(&mut secondary, EXIT_TIMEOUT)?;
        if !status.success() {
            return Err(format!(
                "stage=tray-wake cycle={} secondary_status={status}",
                cycle + 1
            ));
        }
        wait_for_shell_state(window, ShellStateExpectation::Visible, START_TIMEOUT)
            .map_err(|error| format!("stage=tray-show cycle={} {error}", cycle + 1))?;
    }
    Ok(())
}

fn run_controls(
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<Option<u8>, String> {
    for cycle in 0..cycles {
        crate::window_control::click_toolbar(
            window,
            crate::window_control::ToolbarControl::Topmost,
        )
        .map_err(|error| format!("stage=topmost cycle={} {error}", cycle + 1))?;
    }
    for cycle in 0..cycles.saturating_add(2) {
        crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)
            .map_err(|error| format!("stage=theme cycle={} {error}", cycle + 1))?;
    }
    if cycles == 0 {
        return Ok(None);
    }
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    for cycle in 0..cycles {
        crate::window_control::commit_opacity_slider(window, if cycle % 2 == 0 { 70 } else { 100 })
            .map_err(|error| format!("stage=opacity cycle={} {error}", cycle + 1))?;
    }
    Ok(Some(if cycles.is_multiple_of(2) { 100 } else { 70 }))
}

fn run_external_reload(
    directory: &Path,
    child: &mut Child,
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<(), String> {
    if cycles == 0 {
        return Ok(());
    }
    let note = directory.join("note/note.md");
    crate::window_control::switch_to_source(child.id())?;
    wait_for_config_field(directory, "view_mode = \"source\"")?;
    for cycle in 0..cycles {
        let external = format!("external reload cycle {cycle}\n").into_bytes();
        fs::write(&note, &external)
            .map_err(|error| format!("stage=external-write cycle={} {error}", cycle + 1))?;
        wait_for_source_projection(window, &external).map_err(|error| {
            format!(
                "stage=external-reload-projection cycle={} {error}",
                cycle + 1
            )
        })?;
        crate::window_control::press_enter(window)?;
        wait_for_note(&note, &external, cycle + 1, window)?;
        wait_for_window_title(window, |title| title == "StickyMD", "clean autosave")?;
        if cycle == 0 || cycle + 1 == cycles {
            let after = observe_shell(window)?;
            runtime_report!(
                "window-stress input-checkpoint cycle={} expected=editor-input-ready matched={} actual={} logical_visibility=DockedExpanded expected_deadline=none projected_animation_active={}",
                cycle + 1,
                shell_matches(&after, ShellStateExpectation::EditorInputReady),
                format_shell_observation(&after),
                !after.stable_geometry,
            );
        }
    }
    Ok(())
}

fn wait_for_note(
    path: &Path,
    expected_base: &[u8],
    cycle: usize,
    window: crate::window_control::WindowHandle,
) -> Result<(), String> {
    let deadline = Instant::now() + START_TIMEOUT;
    let mut observed = Vec::new();
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(path) {
            observed = bytes;
            if is_single_byte_insertion(&observed, expected_base, b'\n') {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    let shell = observe_shell(window)?;
    Err(format!(
        "stage=external-reload-input cycle={cycle} expected=base-plus-one-LF expected_bytes={} actual_bytes={} actual_hex={} shell={}",
        expected_base.len().saturating_add(1),
        observed.len(),
        short_hex(&observed),
        format_shell_observation(&shell),
    ))
}

fn short_hex(bytes: &[u8]) -> String {
    const LIMIT: usize = 96;
    let mut encoded = String::with_capacity(bytes.len().min(LIMIT) * 2 + 3);
    for byte in bytes.iter().take(LIMIT) {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    if bytes.len() > LIMIT {
        encoded.push_str("...");
    }
    encoded
}
