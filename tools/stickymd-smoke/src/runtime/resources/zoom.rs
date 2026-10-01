//! Zoom resource cohort and cache-growth check.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification
use super::super::*;
use super::{
    Output,
    cohort::{Cohort, check_max},
};
use crate::resource_plan::progress::Observer;

pub(super) fn run_zoom_resource_measurement(
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
    runtime_report!(
        "Phase 10 zoom resource contract: views=source/preview/split zoom=50/100/300 warmup={}s repetitions={}",
        RESOURCE_WARMUP.as_secs(),
        RESOURCE_REPETITIONS,
    );
    for case in crate::resource_plan::zoom::CASES {
        let fixture = case.fixture;
        let label = fixture.label;
        let zoom = case.percent;
        let limit = crate::resource_plan::baseline_private_working_set_limit(fixture.view_mode)
            .ok_or("unknown zoom fixture view")?;
        let mut cohort = Cohort::new(label, RESOURCE_WARMUP.as_secs());
        let outcome = (|| {
            for repetition in 0..RESOURCE_REPETITIONS {
                observer.stage(label, repetition + 1, "starting", 0)?;
                let directory = root.join(format!("{label}-{repetition}"));
                let executable = copy_executable(&source, &directory)?;
                prepare_resource_layout(
                    &directory,
                    fixture.view_mode,
                    fixture.formula_count,
                    fixture.image_count,
                    fixture.image_fixture,
                )?;
                super::preflight_fixture(&directory, fixture)?;
                set_resource_zoom(&directory, zoom)?;
                let mut child = start(&executable)?;
                let result = (|| {
                    wait_for_layout(&directory)?;
                    let window = crate::window_control::visible_window(child.id())?;
                    // Keep Source-only and Preview-only cohorts free of view
                    // transitions that would preload another view's resources.
                    if fixture.view_mode == "split" && repetition == 0 {
                        verify_toolbar_view_clicks(&directory, &mut child, window, zoom, true)?;
                    }
                    crate::window_control::park_cursor_outside_window(window)?;
                    observer.stage(label, repetition + 1, "warmup", RESOURCE_WARMUP.as_secs())?;
                    thread::sleep(RESOURCE_WARMUP);
                    observer.waited(RESOURCE_WARMUP.as_secs());
                    ensure_alive(&mut child, "Phase 10 zoom resource instance")?;
                    if fixture.view_mode == "split" && zoom == 100 && repetition == 0 {
                        observer.stage(label, repetition + 1, "stress", 0)?;
                        let growth =
                            verify_zoom_relayout_does_not_leak(&directory, &mut child, window)?;
                        output.measurements.push(EvidenceMeasurement {
                            name: "zoom_cycles.private_growth".to_owned(),
                            unit: "bytes".to_owned(),
                            value: growth as f64,
                        });
                        check_max(
                            output,
                            "zoom_cycles.private_growth",
                            growth as f64,
                            ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT as f64,
                            "bytes",
                            "tools/stickymd-smoke/src/runtime.rs::ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT",
                        )?;
                    }
                    process_metrics::memory(&child)
                })();
                stop_child(&mut child);
                let sample = result?;
                runtime_report!(
                    "Phase 10 zoom resource sample cohort={label} zoom={zoom} run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                    repetition + 1,
                    sample.private_working_set_bytes,
                    sample.private_bytes,
                    sample.peak_working_set_bytes,
                    sample.peak_private_bytes,
                );
                cohort.memory(output, sample, Some(limit))?;
            }
            Ok::<_, String>(())
        })();
        let summary = cohort.finish(output);
        outcome.and(summary)?;
        observer.stage(label, RESOURCE_REPETITIONS, "case-finished", 0)?;
        output.checkpoint(crate::cli::ResourceModule::Zoom, observer)?;
    }
    Ok(())
}

fn verify_zoom_relayout_does_not_leak(
    program_directory: &Path,
    child: &mut Child,
    window: crate::window_control::WindowHandle,
) -> Result<i64, String> {
    const CYCLES: usize = 100;
    let before = process_metrics::memory(child)?;
    for _ in 0..CYCLES {
        crate::window_control::press_zoom_in(window)?;
        crate::window_control::press_zoom_out(window)?;
    }
    thread::sleep(Duration::from_secs(2));
    ensure_alive(child, "Phase 10 zoom-cycle instance")?;
    wait_for_config_field(program_directory, "content_zoom_percent = 100")?;
    let after = process_metrics::memory(child)?;
    runtime_report!(
        "Phase 10 zoom cycles={CYCLES} before_private_bytes={} after_private_bytes={}",
        before.private_bytes,
        after.private_bytes,
    );
    Ok(after.private_bytes as i64 - before.private_bytes as i64)
}
