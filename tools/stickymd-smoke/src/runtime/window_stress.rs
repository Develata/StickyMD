//! Phase 14 hidden-window stress reduction and diagnostic reporting.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;
use crate::cli::{WindowStressOptions, WindowStressScenario};
use crate::qualification::repetition::{self, RepetitionDisposition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StressFailureClass {
    DesktopJitter,
    Blocking,
}

struct StressFailure {
    detail: String,
    class: StressFailureClass,
}

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
        "window-stress begin scenario={} runs={} collapse_cycles={} tray_cycles={} control_cycles={} view_mode_cycles={} persistence_cycles={}",
        options.scenario.as_str(),
        options.runs,
        options.collapse_cycles,
        options.tray_cycles,
        options.control_cycles,
        options.view_mode_cycles,
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
                failures.push(StressFailure {
                    class: classify_stress_failure(&error),
                    detail: format!("run {run}: {error}"),
                });
            }
        }
    }
    let failed_runs = failures.len();
    if let Err(error) = cleanup_root(&root) {
        failures.push(StressFailure {
            detail: format!("diagnostic cleanup: {error}"),
            class: StressFailureClass::Blocking,
        });
    }
    if failures.is_empty() {
        runtime_report!(
            "window-stress summary scenario={} passed={} failed=0",
            options.scenario.as_str(),
            options.runs
        );
        Ok(())
    } else {
        let blocking = failures
            .iter()
            .filter(|failure| failure.class == StressFailureClass::Blocking)
            .count();
        let details = failures
            .iter()
            .map(|failure| failure.detail.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        let summary = format!(
            "window-stress summary scenario={} passed={} failed_runs={} evidence_failures={} blocking={} failures=[{}]",
            options.scenario.as_str(),
            options.runs.saturating_sub(failed_runs),
            failed_runs,
            failures.len(),
            blocking,
            details,
        );
        if blocking != 0 {
            return Err(summary);
        }
        match repetition::classify(options.runs, failed_runs) {
            RepetitionDisposition::Pass => {
                runtime_report!("{summary} disposition=PASS");
                Ok(())
            }
            RepetitionDisposition::Fail => Err(format!("{summary} disposition=FAIL")),
            RepetitionDisposition::InsufficientSamples => Err(format!(
                "{summary} disposition=FAIL_INSUFFICIENT_SAMPLES minimum_runs={}",
                repetition::MINIMUM_INDEPENDENT_RUNS,
            )),
        }
    }
}

fn classify_stress_failure(error: &str) -> StressFailureClass {
    const JITTER_MARKERS: [&str; 6] = [
        "physical mouse input",
        "physical input target is not ready",
        "without focused foreground StickyMD window",
        "did not become an uncaptured foreground input target",
        "cannot focus source projection probe",
        "cannot restore source projection focus",
    ];
    if JITTER_MARKERS.iter().any(|marker| error.contains(marker)) {
        StressFailureClass::DesktopJitter
    } else {
        StressFailureClass::Blocking
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
        let mut objects = process_metrics::objects(&child)?;
        runtime_report!("window-stress objects stage=baseline sample={objects:?}");

        if matches!(
            options.scenario,
            WindowStressScenario::Collapse
                | WindowStressScenario::CollapseTray
                | WindowStressScenario::Combined
        ) {
            run_collapse(window, options.collapse_cycles)?;
            objects = report_object_delta(&child, "collapse", objects)?;
        }
        if matches!(
            options.scenario,
            WindowStressScenario::Tray
                | WindowStressScenario::CollapseTray
                | WindowStressScenario::Combined
        ) {
            run_tray(&executable, window, options.tray_cycles)?;
            objects = report_object_delta(&child, "tray", objects)?;
        }
        if matches!(
            options.scenario,
            WindowStressScenario::Controls | WindowStressScenario::Combined
        ) && let Some(opacity) = run_controls(window, options.control_cycles)?
        {
            wait_for_config_field(directory, &format!("opacity = {opacity}"))?;
            objects = report_object_delta(&child, "controls", objects)?;
        }
        if matches!(
            options.scenario,
            WindowStressScenario::ViewMode | WindowStressScenario::Combined
        ) {
            run_view_modes(directory, child.id(), window, options.view_mode_cycles)?;
            objects = report_object_delta(&child, "view-mode", objects)?;
        }
        run_external_reload(directory, &mut child, window, options.persistence_cycles)?;
        let _ = report_object_delta(&child, "external-reload", objects)?;
        ensure_alive(&mut child, "window-stress instance")
    })();
    stop_child(&mut child);
    result
}

fn report_object_delta(
    child: &Child,
    stage: &str,
    before: process_metrics::ObjectSample,
) -> Result<process_metrics::ObjectSample, String> {
    let after = process_metrics::objects(child)?;
    runtime_report!(
        "window-stress objects stage={stage} before={before:?} after={after:?} delta_handles={} delta_gdi={} delta_user={}",
        i64::from(after.handles) - i64::from(before.handles),
        i64::from(after.gdi_objects) - i64::from(before.gdi_objects),
        i64::from(after.user_objects) - i64::from(before.user_objects),
    );
    Ok(after)
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

fn run_view_modes(
    directory: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<(), String> {
    let source = fs::read(directory.join("note/note.md"))
        .map_err(|error| format!("stage=view-mode-fixture-read {error}"))?;
    for cycle in 0..cycles {
        crate::window_control::switch_to_preview(window).map_err(|error| {
            format!("stage=view-mode-preview-click cycle={} {error}", cycle + 1)
        })?;
        if cycle == 0 {
            crate::window_control::focus_split_preview(window).map_err(|error| {
                format!("stage=view-mode-preview-focus cycle={} {error}", cycle + 1)
            })?;
            wait_for_preview_projection(window, &["StickyMD Resource Baseline"]).map_err(
                |error| {
                    format!(
                        "stage=view-mode-preview-projection cycle={} {error}",
                        cycle + 1
                    )
                },
            )?;
        }
        wait_for_config_field(directory, "view_mode = \"preview\"").map_err(|error| {
            format!("stage=view-mode-preview-config cycle={} {error}", cycle + 1)
        })?;

        crate::window_control::switch_to_source(process_id)
            .map_err(|error| format!("stage=view-mode-source-click cycle={} {error}", cycle + 1))?;
        if cycle + 1 == cycles {
            crate::window_control::focus_source_editor(window).map_err(|error| {
                format!("stage=view-mode-source-focus cycle={} {error}", cycle + 1)
            })?;
            wait_for_source_projection(window, &source).map_err(|error| {
                format!(
                    "stage=view-mode-source-projection cycle={} {error}",
                    cycle + 1
                )
            })?;
        }
        wait_for_config_field(directory, "view_mode = \"source\"").map_err(|error| {
            format!("stage=view-mode-source-config cycle={} {error}", cycle + 1)
        })?;
    }
    Ok(())
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
    run_conflict_cycles(directory, window, cycles)?;
    run_image_view_cycles(directory, child.id(), window, cycles)?;
    Ok(())
}

fn run_conflict_cycles(
    directory: &Path,
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<(), String> {
    let note = directory.join("note/note.md");
    for cycle in 0..cycles {
        let source = fs::read(&note)
            .map_err(|error| format!("stage=conflict-source-read cycle={} {error}", cycle + 1))?;
        wait_for_source_projection(window, &source)
            .map_err(|error| format!("stage=conflict-source-ready cycle={} {error}", cycle + 1))?;
        crate::window_control::press_enter(window)
            .map_err(|error| format!("stage=conflict-edit cycle={} {error}", cycle + 1))?;
        wait_for_window_title(window, |title| title == "StickyMD *", "dirty edit")
            .map_err(|error| format!("stage=conflict-dirty cycle={} {error}", cycle + 1))?;
        let external = format!("external conflict cycle {cycle}\n");
        fs::write(&note, external.as_bytes())
            .map_err(|error| format!("stage=conflict-write cycle={} {error}", cycle + 1))?;
        wait_for_window_title(
            window,
            |title| title.contains("外部修改冲突"),
            "external conflict",
        )
        .map_err(|error| format!("stage=conflict-detect cycle={} {error}", cycle + 1))?;
        crate::window_control::press_f6(window)
            .map_err(|error| format!("stage=conflict-resolve cycle={} {error}", cycle + 1))?;
        wait_for_window_title(window, |title| title == "StickyMD", "conflict resolution")
            .map_err(|error| format!("stage=conflict-clean cycle={} {error}", cycle + 1))?;
    }
    Ok(())
}

fn run_image_view_cycles(
    directory: &Path,
    process_id: u32,
    window: crate::window_control::WindowHandle,
    cycles: usize,
) -> Result<(), String> {
    let note = directory.join("note/note.md");
    let image_directory = directory.join("note/images");
    fs::create_dir_all(&image_directory)
        .map_err(|error| format!("stage=image-directory-create {error}"))?;
    for cycle in 0..cycles {
        let leaf = format!("stress-cycle-{cycle}.bmp");
        write_bmp(&image_directory.join(&leaf), 128, 128, cycle)
            .map_err(|error| format!("stage=image-write cycle={} {error}", cycle + 1))?;
        let external = format!("![cycle {cycle}](images/{leaf})\n");
        fs::write(&note, external.as_bytes())
            .map_err(|error| format!("stage=image-source-write cycle={} {error}", cycle + 1))?;
        wait_for_source_projection(window, external.as_bytes())
            .map_err(|error| format!("stage=image-source-reload cycle={} {error}", cycle + 1))?;

        crate::window_control::switch_to_preview(window)
            .map_err(|error| format!("stage=image-preview-click cycle={} {error}", cycle + 1))?;
        wait_for_config_field(directory, "view_mode = \"preview\"")
            .map_err(|error| format!("stage=image-preview-config cycle={} {error}", cycle + 1))?;
        thread::sleep(Duration::from_millis(150));

        crate::window_control::switch_to_source(process_id)
            .map_err(|error| format!("stage=image-source-click cycle={} {error}", cycle + 1))?;
        wait_for_source_projection(window, external.as_bytes()).map_err(|error| {
            format!("stage=image-source-projection cycle={} {error}", cycle + 1)
        })?;
        wait_for_config_field(directory, "view_mode = \"source\"")
            .map_err(|error| format!("stage=image-source-config cycle={} {error}", cycle + 1))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_input_route_failures_are_jitter_eligible() {
        assert_eq!(
            classify_stress_failure(
                "cannot activate StickyMD before physical input through physical mouse input"
            ),
            StressFailureClass::DesktopJitter
        );
        assert_eq!(
            classify_stress_failure(
                "refusing keyboard injection without focused foreground StickyMD window"
            ),
            StressFailureClass::DesktopJitter
        );
    }

    #[test]
    fn content_and_persistence_failures_remain_blocking() {
        assert_eq!(
            classify_stress_failure("stage=image-source-projection expected bytes changed"),
            StressFailureClass::Blocking
        );
        assert_eq!(
            classify_stress_failure("config did not contain view_mode = source"),
            StressFailureClass::Blocking
        );
    }
}
