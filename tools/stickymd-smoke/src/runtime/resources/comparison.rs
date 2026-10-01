//! Matched view/size/style cohorts; the normal-style control never becomes product configuration.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use super::*;
use crate::resource_plan::window_comparison::{self, CASES, WindowCase};
use crate::window_control::resource_comparison as native;

pub(super) fn run(
    repository: &Path,
    root: &Path,
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<(), String> {
    let source = crate::qualification::release_executable(repository)?;
    let mut cohorts: Vec<_> = CASES
        .iter()
        .map(|case| cohort::Cohort::new(case.fixture.label, WARMUP_SECONDS))
        .collect();
    let processors = thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut cohort_dpi = None;
    let outcome = (|| {
        for repetition in 0..RESOURCE_REPETITIONS {
            // Keep matched arms nearby, rotating their order across repetitions.
            for view in 0..3 {
                for offset in 0..3 {
                    let index = view * 3 + (offset + repetition) % 3;
                    let case = CASES[index];
                    let label = case.fixture.label;
                    observer.stage(label, repetition + 1, "starting", 0)?;
                    let directory = root.join(format!("{label}-{repetition}"));
                    let executable = copy_executable(&source, &directory)?;
                    prepare(&directory, case)?;
                    let note =
                        fs::read(directory.join("note/note.md")).map_err(|e| e.to_string())?;
                    let ready = ReadyEvent::create((repetition * CASES.len() + index) as u64)?;
                    let mut command = Command::new(&executable);
                    command
                        .current_dir(&directory)
                        .env("STICKYMD_DIAGNOSTIC_READY_EVENT", ready.name())
                        .env_remove("STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY")
                        .env_remove("STICKYMD_DIAGNOSTIC_STARTUP_TRACE")
                        .env_remove("STICKYMD_DIAGNOSTIC_STARTUP_DETAILS")
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null());
                    let mut child =
                        ChildGuard::spawn(&mut command, "start window resource comparison")?;
                    let result = (|| {
                        ready.wait(START_TIMEOUT)?;
                        let window = crate::window_control::visible_window(child.id())?;
                        if case.normal_style {
                            native::apply_normal_style(&child, window)?;
                        }
                        let (_, _, dpi) = check(&child, window, case, cohort_dpi)?;
                        cohort_dpi = Some(dpi);
                        crate::window_control::park_cursor_outside_window(window)?;
                        observer.stage(
                            label,
                            repetition + 1,
                            "warmup",
                            RESOURCE_WARMUP.as_secs(),
                        )?;
                        thread::sleep(RESOURCE_WARMUP);
                        observer.waited(RESOURCE_WARMUP.as_secs());
                        facts(output, &child, window, case, repetition + 1, "before", dpi)?;
                        let memory = process_metrics::memory(&child)?;
                        cohorts[index].memory(
                            output,
                            memory,
                            crate::resource_plan::baseline_private_working_set_limit(
                                case.fixture.view_mode,
                            ),
                        )?;
                        observer.stage(label, repetition + 1, "cpu", CPU_INTERVAL.as_secs())?;
                        let cpu = measure_idle_cpu_observed(
                            &mut child,
                            label,
                            processors,
                            window,
                            |child| check(child, window, case, Some(dpi)).map(|_| ()),
                        )?;
                        observer.waited(CPU_INTERVAL.as_secs());
                        cohorts[index].cpu(output, cpu)?;
                        facts(output, &child, window, case, repetition + 1, "after", dpi)?;
                        if fs::read(directory.join("note/note.md")).map_err(|e| e.to_string())?
                            != note
                        {
                            return Err(
                                "window comparison fixture changed during observation".into()
                            );
                        }
                        wait_for_view_mode(&directory, case.fixture.view_mode)?;
                        runtime_report!(
                            "window comparison cohort={label} run={} pws_bytes={} private_bytes={} cpu_percent={cpu:.6}",
                            repetition + 1,
                            memory.private_working_set_bytes,
                            memory.private_bytes
                        );
                        Ok::<_, String>(())
                    })();
                    stop_child(&mut child);
                    result?;
                    observer.stage(label, repetition + 1, "sample-finished", 0)?;
                    output.checkpoint(ResourceModule::Window, observer)?;
                }
            }
        }
        Ok(())
    })();
    let mut summary = Ok(());
    for cohort in cohorts {
        if let Err(error) = cohort.finish(output) {
            summary = Err(error);
        }
    }
    outcome.and(summary)
}

fn prepare(directory: &Path, case: WindowCase) -> Result<(), String> {
    let fixture = case.fixture;
    prepare_resource_layout(
        directory,
        fixture.view_mode,
        fixture.formula_count,
        fixture.image_count,
        fixture.image_fixture,
    )?;
    super::preflight_fixture(directory, fixture)?;
    let config = format!(
        "version = 1\nview_mode = \"{}\"\ncontent_zoom_percent = 100\n[window]\nwidth_dip = {}\nheight_dip = {}\n",
        fixture.view_mode, case.width_dip, case.height_dip
    );
    crate::atomic_evidence::write(&directory.join("note/config.toml"), config.as_bytes())
}

fn check(
    child: &Child,
    window: crate::window_control::WindowHandle,
    case: WindowCase,
    expected_dpi: Option<u32>,
) -> Result<(f64, f64, u32), String> {
    let (width, height, dpi) = native::facts(child, window, case.normal_style)?;
    window_comparison::validate_geometry(case, width, height, f64::from(dpi))?;
    if let Some(expected) = expected_dpi {
        window_comparison::validate_matching_dpi(f64::from(expected), f64::from(dpi))?;
    }
    Ok((width, height, dpi))
}

fn facts(
    output: &mut Output,
    child: &Child,
    window: crate::window_control::WindowHandle,
    case: WindowCase,
    run: usize,
    stage: &str,
    expected_dpi: u32,
) -> Result<(), String> {
    let (width, height, dpi) = check(child, window, case, Some(expected_dpi))?;
    let objects = process_metrics::objects(child)?;
    output.measurements.extend(
        [
            ("width", width, "dip"),
            ("height", height, "dip"),
            ("dpi", f64::from(dpi), "dpi"),
            ("style_verified", 1.0, "count"),
            ("handles", f64::from(objects.handles), "count"),
            ("gdi_objects", f64::from(objects.gdi_objects), "count"),
            ("user_objects", f64::from(objects.user_objects), "count"),
        ]
        .into_iter()
        .map(|(name, value, unit)| EvidenceMeasurement {
            name: format!("{}.run_{run}.{stage}.{name}", case.fixture.label),
            unit: unit.into(),
            value,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_arms_use_identical_note_bytes_and_only_configure_size_and_view() {
        let root = create_smoke_root().unwrap();
        let mut note = None;
        for (index, &case) in CASES.iter().enumerate() {
            let directory = root.join(index.to_string());
            fs::create_dir(&directory).unwrap();
            prepare(&directory, case).unwrap();
            let bytes = fs::read(directory.join("note/note.md")).unwrap();
            assert_eq!(&bytes, note.get_or_insert_with(|| bytes.clone()));
            let config = fs::read_to_string(directory.join("note/config.toml")).unwrap();
            assert!(!config.contains("normal") && !config.contains("tool"));
            assert!(config.contains("content_zoom_percent = 100"));
        }
        cleanup_root(&root).unwrap();
    }
}
