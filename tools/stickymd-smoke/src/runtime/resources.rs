//! Isolated native resource execution; pure scenario identity lives in resource_plan.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

mod cohort;
mod probe;
mod window;
mod zoom;
use super::*;
use crate::cli::ResourceModule;
use crate::resource_plan::progress::Observer;
use crate::resource_plan::{ResourceCase, ScenarioCache, Transition, WARMUP_SECONDS};

#[derive(Default)]
pub(crate) struct Cache {
    cohorts: ScenarioCache<Output>,
    executable_sha256: Option<String>,
}
#[derive(Clone, Default)]
pub(crate) struct Output {
    pub(crate) measurements: Vec<EvidenceMeasurement>,
    pub(crate) gates: Vec<EvidenceGate>,
    pub(crate) samples: Vec<EvidenceSample>,
    pub(crate) shared: Vec<String>,
    pub(crate) failure: Option<String>,
}

impl Output {
    fn checkpoint(&self, group: ResourceModule, observer: &mut dyn Observer) -> Result<(), String> {
        observer.checkpoint(crate::evidence::EvidenceResult {
            id: group.task_label().into(),
            status: crate::evidence::EvidenceStatus::NotTested,
            detail: Some("INCOMPLETE: resource group has not finished".into()),
            measurements: self.measurements.clone(),
            gates: self.gates.clone(),
            samples: self.samples.clone(),
        })
    }
}

pub(crate) fn run(
    repository: &Path,
    group: ResourceModule,
    json: bool,
    cache: &mut Cache,
    observer: &mut dyn Observer,
) -> Result<Output, String> {
    JSON_OUTPUT.store(json, Ordering::Relaxed);
    let filter = std::env::var("STICKYMD_SMOKE_RESOURCE_CASE").unwrap_or_default();
    validate_filter(group, &filter)?;
    managed_process::ensure_no_stale_smoke_stickymd()?;
    observer.stage("desktop-probe", 0, "probe", 0)?;
    let root = create_smoke_root()?;
    let mut output = Output::default();
    let started = Instant::now();
    let probe_result = probe::run(repository, &root);
    output.measurements.push(EvidenceMeasurement {
        name: "desktop_probe.execution_seconds".into(),
        unit: "seconds".into(),
        value: started.elapsed().as_secs_f64(),
    });
    runtime_report!(
        "RESOURCE_DESKTOP_PROBE group={} status={} elapsed_seconds={:.3}",
        group.name(),
        if probe_result.is_ok() {
            "PASSED"
        } else {
            "FAILED"
        },
        started.elapsed().as_secs_f64()
    );
    let result = probe_result.and_then(|()| match group {
        ResourceModule::Window => {
            window::run_window_resource_measurement(repository, &root, &mut output, observer)
        }
        ResourceModule::Zoom => {
            zoom::run_zoom_resource_measurement(repository, &root, &mut output, observer)
        }
        _ => matrix(repository, &root, group, cache, &mut output, observer),
    });
    output.failure = result.err();
    if let Err(error) = cleanup_root(&root) {
        output.failure = Some(match output.failure {
            Some(original) => format!("{original}; resource cleanup also failed: {error}"),
            None => error,
        });
    }
    Ok(output)
}

fn validate_filter(group: ResourceModule, filter: &str) -> Result<(), String> {
    if !filter.is_empty() && !group.cases().iter().any(|case| case.label == filter) {
        return Err(format!(
            "unknown resource case filter {filter} for {}",
            group.name()
        ));
    }
    Ok(())
}

fn matrix(
    repository: &Path,
    root: &Path,
    group: ResourceModule,
    cache: &mut Cache,
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<(), String> {
    let source = crate::qualification::release_executable(repository)?;
    let executable_sha256 = crate::integrity::sha256(&source)?;
    if cache
        .executable_sha256
        .as_ref()
        .is_some_and(|previous| previous != &executable_sha256)
    {
        return Err(
            "resource executable changed during this command; refusing to share earlier cohorts"
                .into(),
        );
    }
    cache.executable_sha256 = Some(executable_sha256);
    let filter = std::env::var("STICKYMD_SMOKE_RESOURCE_CASE").unwrap_or_default();
    let cases: Vec<_> = group
        .cases()
        .iter()
        .copied()
        .filter(|case| filter.is_empty() || case.label == filter)
        .collect();
    if cases.is_empty() {
        return Err(format!("unknown resource case filter {filter}"));
    }
    for case in cases {
        observer.stage(case.label, 0, "case-start", 0)?;
        let started = Instant::now();
        let measured = cache.cohorts.measure(case, || {
            let mut observed = Output::default();
            let outcome = measure_case(&source, root, case, &mut observed, observer);
            match outcome {
                Ok(()) => Ok(observed),
                Err(error) => {
                    output.measurements.extend(observed.measurements);
                    output.samples.extend(observed.samples);
                    output.gates.extend(observed.gates);
                    Err(error)
                }
            }
        });
        let elapsed = started.elapsed().as_secs_f64();
        output.measurements.push(EvidenceMeasurement {
            name: format!("{}.execution_seconds", case.label),
            unit: "seconds".into(),
            value: elapsed,
        });
        let (mut observed, origin) = measured?;
        if let Some(origin) = origin {
            alias_cohort(&mut observed, origin, case.label);
            output.shared.push(format!(
                "{} <- {origin} (same five-sample cohort)",
                case.label
            ));
        }
        runtime_report!(
            "RESOURCE_CASE id={} shared_from={} elapsed_seconds={elapsed:.3}",
            case.label,
            origin.unwrap_or("none")
        );
        output.measurements.extend(observed.measurements);
        output.samples.extend(observed.samples);
        output.gates.extend(observed.gates);
        observer.stage(
            case.label,
            RESOURCE_REPETITIONS,
            if origin.is_some() {
                "shared"
            } else {
                "case-finished"
            },
            0,
        )?;
        output.checkpoint(group, observer)?;
    }
    Ok(())
}

fn alias_cohort(output: &mut Output, origin: &str, alias: &str) {
    let rename = |name: &mut String| {
        if let Some(suffix) = name.strip_prefix(&format!("{origin}.")) {
            *name = format!("{alias}.{suffix}");
        }
    };
    for measurement in &mut output.measurements {
        rename(&mut measurement.name);
    }
    for gate in &mut output.gates {
        rename(&mut gate.metric);
    }
    for sample in &mut output.samples {
        sample.cohort = alias.into();
        sample.shared_from = Some(origin.into());
    }
}

fn measure_case(
    source: &Path,
    root: &Path,
    case: ResourceCase,
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<(), String> {
    let mode = case.label;
    let logical_processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let mut cohort = cohort::Cohort::new(mode, WARMUP_SECONDS);
    let outcome = (|| {
        for repetition in 0..RESOURCE_REPETITIONS {
            observer.stage(mode, repetition + 1, "starting", 0)?;
            let directory = root.join(format!("{mode}-{repetition}"));
            let executable = copy_executable(source, &directory)?;
            prepare_resource_layout(
                &directory,
                case.view_mode,
                case.formula_count,
                case.image_count,
                case.image_fixture,
            )?;
            preflight_fixture(&directory, case)?;
            let mut child = start(&executable)?;
            wait_for_layout(&directory)?;
            // A resource baseline represents a truly idle window. Keep the
            // physical cursor outside the paper so incidental mouse jitter or
            // operator movement cannot turn preview hit-testing and title
            // updates into process CPU attributed to the idle sample.
            let window = crate::window_control::visible_window(child.id())?;
            crate::window_control::park_cursor_outside_window(window)?;
            observer.stage(mode, repetition + 1, "warmup", RESOURCE_WARMUP.as_secs())?;
            thread::sleep(RESOURCE_WARMUP);
            observer.waited(RESOURCE_WARMUP.as_secs());
            ensure_alive(&mut child, "resource measurement instance")?;
            if case.transition == Transition::PreviewToSource {
                observer.stage(mode, repetition + 1, "transition", 5)?;
                crate::window_control::switch_to_source(child.id())?;
                wait_for_view_mode(&directory, "source")?;
                thread::sleep(Duration::from_secs(5));
                observer.waited(5);
                ensure_alive(&mut child, "Source-after-Preview resource instance")?;
            }
            let sample = process_metrics::memory(&child)?;
            runtime_report!(
                "resource sample mode={mode} run={} private_working_set_bytes={} private_bytes={} \
             peak_working_set_bytes={} peak_private_bytes={}",
                repetition + 1,
                sample.private_working_set_bytes,
                sample.private_bytes,
                sample.peak_working_set_bytes,
                sample.peak_private_bytes,
            );
            cohort.memory(output, sample, None)?;
            if case.measure_cpu {
                observer.stage(mode, repetition + 1, "cpu", CPU_INTERVAL.as_secs())?;
                let cpu = measure_idle_cpu(&mut child, mode, logical_processors, window)?;
                observer.waited(CPU_INTERVAL.as_secs());
                cohort.cpu(output, cpu)?;
            }
            stop_child(&mut child);
        }
        Ok(())
    })();
    let summary = cohort.finish(output);
    outcome.and(summary)
}

fn preflight_fixture(directory: &Path, case: ResourceCase) -> Result<(), String> {
    let note = fs::read_to_string(directory.join("note/note.md"))
        .map_err(|e| format!("resource fixture: {e}"))?;
    let expected_images = if case.image_fixture == ImageResourceFixture::FourK {
        1
    } else if case.image_fixture == ImageResourceFixture::SaturatedCache {
        420
    } else {
        case.image_count
    };
    if note.len() < 20 * 1024 - 3
        || note.len() > 20 * 1024
        || note.matches('$').count() != case.formula_count * 2
        || note.matches("![").count() != expected_images
    {
        return Err(format!(
            "invalid resource fixture {}: bytes={} formulas={} images={}",
            case.label,
            note.len(),
            note.matches('$').count() / 2,
            note.matches("![").count()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_resource_filter_is_rejected_before_the_desktop_probe() {
        assert!(validate_filter(ResourceModule::Images, "typo").is_err());
        assert!(validate_filter(ResourceModule::Window, "source").is_err());
        assert!(validate_filter(ResourceModule::Images, "preview-1-image").is_ok());
        assert!(validate_filter(ResourceModule::Zoom, "").is_ok());
    }
    #[test]
    fn a_shared_cohort_keeps_raw_samples_and_gates_in_its_independent_group_receipt() {
        let mut cache = ScenarioCache::default();
        for group in [
            ResourceModule::SourcePreview,
            ResourceModule::Math,
            ResourceModule::Images,
        ] {
            let valid = crate::resource_plan::tests::valid_resource_result(group);
            let mut result = valid.clone();
            result.measurements.clear();
            result.gates.clear();
            result.samples.clear();
            for &case in group.cases() {
                let (mut output, origin) = cache
                    .measure(case, || {
                        Ok(Output {
                            measurements: valid
                                .measurements
                                .iter()
                                .filter(|m| m.name.starts_with(&format!("{}.", case.label)))
                                .cloned()
                                .collect(),
                            gates: valid
                                .gates
                                .iter()
                                .filter(|g| g.metric.starts_with(&format!("{}.", case.label)))
                                .cloned()
                                .collect(),
                            samples: valid
                                .samples
                                .iter()
                                .filter(|s| s.cohort == case.label)
                                .cloned()
                                .collect(),
                            ..Default::default()
                        })
                    })
                    .unwrap();
                if let Some(origin) = origin {
                    alias_cohort(&mut output, origin, case.label);
                }
                result.measurements.extend(output.measurements);
                result.gates.extend(output.gates);
                result.samples.extend(output.samples);
            }
            let document = crate::resource_plan::tests::document(group, &result);
            crate::resource_plan::validate_receipt(&document, group).unwrap();
            assert_eq!(
                result.samples.len(),
                group.cohorts().len() * RESOURCE_REPETITIONS
            );
            if group == ResourceModule::Math {
                assert_eq!(
                    result
                        .samples
                        .iter()
                        .filter(|s| s.shared_from.is_some())
                        .count(),
                    15
                );
                assert!(document.contains("\"shared_from\":\"source\""));
            }
        }
    }
    #[test]
    fn equivalent_scenarios_generate_identical_note_and_configuration_bytes() {
        let root = create_smoke_root().unwrap();
        let source = ResourceModule::SourcePreview.cases();
        let math = ResourceModule::Math.cases();
        let images = ResourceModule::Images.cases();
        for (left, right) in [
            (source[0], math[0]),
            (source[1], math[3]),
            (source[2], math[4]),
            (math[1], images[2]),
        ] {
            for case in [left, right] {
                let directory = root.join(case.label);
                fs::create_dir(&directory).unwrap();
                prepare_resource_layout(
                    &directory,
                    case.view_mode,
                    case.formula_count,
                    case.image_count,
                    case.image_fixture,
                )
                .unwrap();
                preflight_fixture(&directory, case).unwrap();
            }
            for file in ["note/note.md", "note/config.toml"] {
                assert_eq!(
                    fs::read(root.join(left.label).join(file)).unwrap(),
                    fs::read(root.join(right.label).join(file)).unwrap()
                );
            }
        }
        cleanup_root(&root).unwrap();
    }

    #[test]
    fn preflight_accepts_every_registered_matrix_fixture() {
        let root = create_smoke_root().unwrap();
        for group in crate::resource_plan::GROUPS {
            for &case in group.cases() {
                let directory = root.join(case.label);
                fs::create_dir(&directory).unwrap();
                prepare_resource_layout(
                    &directory,
                    case.view_mode,
                    case.formula_count,
                    case.image_count,
                    case.image_fixture,
                )
                .unwrap();
                preflight_fixture(&directory, case).unwrap();
            }
        }
        cleanup_root(&root).unwrap();
    }
    #[test]
    fn preflight_rejects_a_wrong_fixture_before_starting_a_process() {
        let root = create_smoke_root().unwrap();
        let case = ResourceModule::Math.cases()[3];
        prepare_resource_layout(
            &root,
            case.view_mode,
            case.formula_count,
            case.image_count,
            case.image_fixture,
        )
        .unwrap();
        preflight_fixture(&root, case).unwrap();
        crate::atomic_evidence::write(&root.join("note/note.md"), b"wrong fixture").unwrap();
        assert!(preflight_fixture(&root, case).is_err());
        cleanup_root(&root).unwrap();
    }
}
