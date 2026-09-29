//! Zoom resource cohort and cache-growth check.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification
use super::super::*;
use super::{
    Output,
    cohort::{Cohort, check_max},
};

pub(super) fn run_zoom_resource_measurement(
    repository: &Path,
    root: &Path,
    output: &mut Output,
) -> Result<(), String> {
    const SPLIT_PRIVATE_WORKING_SET_LIMIT: u64 =
        crate::resource_plan::ZOOM_PRIVATE_WORKING_SET_LIMIT;
    let source = crate::qualification::release_executable(repository)?;
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    runtime_report!(
        "Phase 10 zoom resource contract: zoom=50/100/300 warmup={}s repetitions={}",
        ZOOM_RESOURCE_WARMUP.as_secs(),
        RESOURCE_REPETITIONS,
    );
    for zoom in [50_u16, 100, 300] {
        let label = format!("split-zoom-{zoom}");
        let mut cohort = Cohort::new(&label, ZOOM_RESOURCE_WARMUP.as_secs());
        let outcome = (|| {
            for repetition in 0..RESOURCE_REPETITIONS {
                let directory = root.join(format!("{label}-{repetition}"));
                let executable = copy_executable(&source, &directory)?;
                prepare_resource_layout(&directory, "split", 20, 12, ImageResourceFixture::None)?;
                super::preflight_fixture(
                    &directory,
                    crate::resource_plan::ResourceCase {
                        label: "split-zoom",
                        view_mode: "split",
                        formula_count: 20,
                        image_count: 12,
                        ..crate::cli::ResourceModule::Images.cases()[0]
                    },
                )?;
                set_resource_zoom(&directory, zoom)?;
                let mut child = start(&executable)?;
                let result = (|| {
                    wait_for_layout(&directory)?;
                    let window = crate::window_control::visible_window(child.id())?;
                    if repetition == 0 {
                        verify_toolbar_view_clicks(&directory, &mut child, window, zoom, true)?;
                    }
                    crate::window_control::park_cursor_outside_window(window)?;
                    thread::sleep(ZOOM_RESOURCE_WARMUP);
                    ensure_alive(&mut child, "Phase 10 zoom resource instance")?;
                    if zoom == 100 && repetition == 0 {
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
                    "Phase 10 zoom resource sample zoom={zoom} run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                    repetition + 1,
                    sample.private_working_set_bytes,
                    sample.private_bytes,
                    sample.peak_working_set_bytes,
                    sample.peak_private_bytes,
                );
                cohort.memory(output, sample, Some(SPLIT_PRIVATE_WORKING_SET_LIMIT))?;
            }
            Ok::<_, String>(())
        })();
        let summary = cohort.finish(output);
        outcome.and(summary)?;
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
