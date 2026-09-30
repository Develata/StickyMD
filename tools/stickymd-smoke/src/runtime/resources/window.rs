//! Complete window resource cohort and indivisible stress procedure.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification
use super::super::*;
use super::{Output, cohort::Cohort};
use crate::resource_plan::progress::Observer;

pub(super) fn run_window_resource_measurement(
    repository: &Path,
    root: &Path,
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<(), String> {
    let source = crate::qualification::release_executable(repository)?;
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let logical_processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    runtime_report!(
        "Phase 8 window resource contract: warmup={}s repetitions={} cpu_interval={}s logical_processors={logical_processors}",
        RESOURCE_WARMUP.as_secs(),
        RESOURCE_REPETITIONS,
        CPU_INTERVAL.as_secs(),
    );
    let mut visible_cohort = Cohort::new("visible-source", RESOURCE_WARMUP.as_secs());
    let mut collapsed_cohort = Cohort::new("docked-collapsed", RESOURCE_WARMUP.as_secs());
    let mut hidden_cohort = Cohort::new("hidden-to-tray", RESOURCE_WARMUP.as_secs());
    let mut startup_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let outcome: Result<(), String> = (|| {
        for repetition in 0..RESOURCE_REPETITIONS {
            observer.stage("visible-source", repetition + 1, "starting", 0)?;
            let directory = root.join(format!("window-resource-{repetition}"));
            let executable = copy_executable(&source, &directory)?;
            prepare_resource_layout(&directory, "source", 0, 0, ImageResourceFixture::None)?;
            super::preflight_fixture(&directory, crate::cli::ResourceModule::Images.cases()[0])?;
            let note = directory.join("note/note.md");
            let baseline_note = fs::read(&note)
                .map_err(|error| format!("cannot read window resource baseline: {error}"))?;
            let startup_started = Instant::now();
            let mut child = start(&executable)?;
            let result = (|| {
                wait_for_layout(&directory)?;
                let window = crate::window_control::visible_window(child.id())?;
                let startup = startup_started.elapsed();
                startup_samples.push(startup);
                output.measurements.push(EvidenceMeasurement {
                    name: format!("window.run_{}.startup_to_paper", repetition + 1),
                    unit: "ms".into(),
                    value: startup.as_secs_f64() * 1_000.0,
                });
                runtime_report!(
                    "window startup run={} elapsed_ms={:.3}",
                    repetition + 1,
                    startup.as_secs_f64() * 1_000.0
                );
                observer.stage(
                    "visible-source",
                    repetition + 1,
                    "warmup",
                    RESOURCE_WARMUP.as_secs(),
                )?;
                thread::sleep(RESOURCE_WARMUP);
                observer.waited(RESOURCE_WARMUP.as_secs());
                ensure_alive(&mut child, "visible window resource instance")?;
                let visible = process_metrics::memory(&child)?;
                runtime_report!(
                    "resource sample mode=visible-source run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                    repetition + 1,
                    visible.private_working_set_bytes,
                    visible.private_bytes,
                    visible.peak_working_set_bytes,
                    visible.peak_private_bytes,
                );
                visible_cohort.memory(output, visible, None)?;
                observer.stage(
                    "visible-source",
                    repetition + 1,
                    "cpu",
                    CPU_INTERVAL.as_secs(),
                )?;
                let cpu =
                    measure_idle_cpu(&mut child, "visible-source", logical_processors, window)?;
                observer.waited(CPU_INTERVAL.as_secs());
                visible_cohort.cpu(output, cpu)?;
                crate::window_control::move_to_primary_left_edge(window)?;
                wait_for_config_field(&directory, "dock_edge = \"left\"")?;
                crate::window_control::park_cursor_at_primary_right(window)?;
                crate::window_control::click_toolbar(
                    window,
                    crate::window_control::ToolbarControl::Collapse,
                )?;
                wait_for_primary_left_state(window, true)?;
                observer.stage(
                    "docked-collapsed",
                    repetition + 1,
                    "warmup",
                    RESOURCE_WARMUP.as_secs(),
                )?;
                thread::sleep(RESOURCE_WARMUP);
                observer.waited(RESOURCE_WARMUP.as_secs());
                ensure_alive(&mut child, "collapsed window resource instance")?;
                let collapsed = process_metrics::memory(&child)?;
                runtime_report!(
                    "resource sample mode=docked-collapsed run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                    repetition + 1,
                    collapsed.private_working_set_bytes,
                    collapsed.private_bytes,
                    collapsed.peak_working_set_bytes,
                    collapsed.peak_private_bytes,
                );
                collapsed_cohort.memory(output, collapsed, None)?;
                observer.stage(
                    "docked-collapsed",
                    repetition + 1,
                    "cpu",
                    CPU_INTERVAL.as_secs(),
                )?;
                let cpu =
                    measure_idle_cpu(&mut child, "docked-collapsed", logical_processors, window)?;
                observer.waited(CPU_INTERVAL.as_secs());
                collapsed_cohort.cpu(output, cpu)?;
                if repetition == 0 {
                    observer.stage("window-stress", repetition + 1, "stress", 0)?;
                    run_window_leak_cycles(&directory, &executable, &mut child, window)?;
                    // Persistence/image stress replaces the document. Restore the same
                    // baseline before hidden warmup so all five samples measure it.
                    crate::atomic_evidence::write(&note, &baseline_note)?;
                    wait_for_source_projection(window, &baseline_note)?;
                    wait_for_window_title(
                        window,
                        |title| title == "StickyMD",
                        "restored window resource baseline",
                    )?;
                    output.measurements.push(EvidenceMeasurement {
                        name: "window.stress_completed".into(),
                        unit: "count".into(),
                        value: 1.0,
                    });
                }
                crate::window_control::request_close(window)?;
                wait_for_window_visibility(window, false)?;
                observer.stage(
                    "hidden-to-tray",
                    repetition + 1,
                    "warmup",
                    RESOURCE_WARMUP.as_secs(),
                )?;
                thread::sleep(RESOURCE_WARMUP);
                observer.waited(RESOURCE_WARMUP.as_secs());
                ensure_alive(&mut child, "hidden-to-tray resource instance")?;
                let hidden = process_metrics::memory(&child)?;
                runtime_report!(
                    "resource sample mode=hidden-to-tray run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                    repetition + 1,
                    hidden.private_working_set_bytes,
                    hidden.private_bytes,
                    hidden.peak_working_set_bytes,
                    hidden.peak_private_bytes,
                );
                hidden_cohort.memory(output, hidden, Some(HIDDEN_PRIVATE_WORKING_SET_LIMIT))?;
                observer.stage(
                    "hidden-to-tray",
                    repetition + 1,
                    "cpu",
                    CPU_INTERVAL.as_secs(),
                )?;
                let cpu =
                    measure_idle_cpu(&mut child, "hidden-to-tray", logical_processors, window)?;
                observer.waited(CPU_INTERVAL.as_secs());
                hidden_cohort.cpu(output, cpu)?;
                let observed_note = fs::read(&note)
                    .map_err(|error| format!("cannot verify hidden resource baseline: {error}"))?;
                if observed_note != baseline_note {
                    return Err(format!(
                        "hidden resource run={} changed its baseline: expected_bytes={} actual_bytes={}",
                        repetition + 1,
                        baseline_note.len(),
                        observed_note.len(),
                    ));
                }
                runtime_report!(
                    "resource fixture mode=hidden-to-tray run={} bytes={} baseline_matches=true",
                    repetition + 1,
                    observed_note.len(),
                );
                output.measurements.push(EvidenceMeasurement {
                    name: format!("hidden-to-tray.run_{}.fixture_bytes", repetition + 1),
                    unit: "bytes".to_owned(),
                    value: baseline_note.len() as f64,
                });
                Ok::<_, String>(())
            })();
            stop_child(&mut child);
            result?;
            output.checkpoint(crate::cli::ResourceModule::Window, observer)?;
        }
        print_duration_summary("startup-to-paper", &mut startup_samples)?;
        output.measurements.extend(duration_measurements(
            "window.startup_to_paper",
            &startup_samples,
        ));
        Ok(())
    })();
    let summaries = [
        visible_cohort.finish(output),
        collapsed_cohort.finish(output),
        hidden_cohort.finish(output),
    ];
    outcome?;
    for summary in summaries {
        summary?;
    }
    Ok(())
}
