//! Opt-in Windows runtime smoke using copied Release executables.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::evidence::{EvidenceGate, EvidenceMeasurement, EvidenceSample};
use crate::process_metrics::{self, MemorySample};
use crate::ready_event::ReadyEvent;
use crate::runner::RuntimeScenario;

const START_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const RESOURCE_WARMUP: Duration = Duration::from_secs(30);
const CPU_INTERVAL: Duration = Duration::from_secs(60);
const RESOURCE_REPETITIONS: usize = 5;
const HIDDEN_PRIVATE_WORKING_SET_LIMIT: u64 = 36 * 1024 * 1024;
const IDLE_CPU_PERCENT_LIMIT: f64 = 0.1;
const COLD_STARTUP_SAMPLE_COUNT: usize = 30;
const WARM_STARTUP_SAMPLE_COUNT: usize = 50;
const COLD_START_IDLE: Duration = Duration::from_secs(10);
const WARM_START_IDLE: Duration = Duration::from_millis(250);
const STARTUP_PREFERRED_TARGET: Duration = Duration::from_millis(180);
const STARTUP_ENGINEERING_TARGET: Duration = Duration::from_millis(400);
const V0_1_0_STARTUP_RELEASE_BOUNDARY: Duration = Duration::from_millis(550);
const ZOOM_RESOURCE_WARMUP: Duration = Duration::from_secs(5);
const ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT: u64 = 8 * 1024 * 1024;
static QUIET_OUTPUT: AtomicBool = AtomicBool::new(false);

macro_rules! runtime_report {
    ($($argument:tt)*) => {
        if !QUIET_OUTPUT.load(Ordering::Relaxed) {
            println!($($argument)*);
        }
    };
}

mod window_stress;

pub(crate) use window_stress::run as run_window_stress_diagnostic;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellStateExpectation {
    Visible,
    Hidden,
    PrimaryLeftCollapsed,
    PrimaryLeftExpanded,
    EditorInputReady,
}

#[derive(Clone, Debug)]
struct ShellObservation {
    visible: bool,
    rect: crate::window_control::WindowRect,
    work: crate::window_control::WindowRect,
    activation: crate::window_control::WindowActivationFacts,
    cursor: crate::window_control::CursorFacts,
    style: crate::window_control::WindowStyleFacts,
    topmost: bool,
    alpha: crate::window_control::LayeredAlpha,
    title: String,
    stable_geometry: bool,
}

pub(crate) fn run(
    repository: &Path,
    scenario: RuntimeScenario,
    quiet: bool,
) -> Result<RuntimeEvidence, String> {
    QUIET_OUTPUT.store(quiet, Ordering::Relaxed);
    let root = create_smoke_root()?;
    let mut children = Vec::new();
    let result = if scenario == RuntimeScenario::Startup {
        run_startup_measurement(repository, &root)
    } else if scenario == RuntimeScenario::Resources {
        run_resource_measurement(repository, &root, false, false).map(RuntimeEvidence::passed)
    } else if scenario == RuntimeScenario::MathResources {
        run_resource_measurement(repository, &root, true, false).map(RuntimeEvidence::passed)
    } else if scenario == RuntimeScenario::ImageResources {
        run_resource_measurement(repository, &root, false, true).map(RuntimeEvidence::passed)
    } else if scenario == RuntimeScenario::WindowResources {
        run_window_resource_measurement(repository, &root).map(RuntimeEvidence::passed)
    } else if scenario == RuntimeScenario::ZoomResources {
        run_zoom_resource_measurement(repository, &root).map(RuntimeEvidence::passed)
    } else {
        run_inner(repository, &root, scenario, &mut children)
            .map(|()| RuntimeEvidence::passed(Vec::new()))
    };
    stop_children(&mut children);
    let cleanup = cleanup_root(&root);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(evidence), Ok(())) => Ok(evidence),
    }
}

pub(crate) struct RuntimeEvidence {
    pub(crate) measurements: Vec<EvidenceMeasurement>,
    pub(crate) gates: Vec<EvidenceGate>,
    pub(crate) samples: Vec<EvidenceSample>,
    pub(crate) gate_failure: Option<String>,
}

impl RuntimeEvidence {
    fn passed(measurements: Vec<EvidenceMeasurement>) -> Self {
        Self {
            measurements,
            gates: Vec::new(),
            samples: Vec::new(),
            gate_failure: None,
        }
    }
}

fn run_inner(
    repository: &Path,
    root: &Path,
    scenario: RuntimeScenario,
    children: &mut Vec<Child>,
) -> Result<(), String> {
    debug_assert!(!matches!(
        scenario,
        RuntimeScenario::Resources
            | RuntimeScenario::MathResources
            | RuntimeScenario::ImageResources
            | RuntimeScenario::WindowResources
            | RuntimeScenario::Startup
            | RuntimeScenario::ZoomResources
    ));
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}; run the planned Release build first",
            source.display()
        ));
    }
    let first_dir = root.join("first");
    let first_exe = copy_executable(&source, &first_dir)?;
    if scenario == RuntimeScenario::Preview {
        prepare_preview_layout(&first_dir, "preview")?;
    } else if scenario == RuntimeScenario::Math {
        prepare_math_layout(&first_dir, "preview")?;
    } else if scenario == RuntimeScenario::Assets {
        prepare_asset_layout(&first_dir, "preview", 12)?;
    } else if scenario == RuntimeScenario::Phase11B {
        prepare_phase11b_layout(&first_dir)?;
    }
    children.push(start(&first_exe)?);
    wait_for_layout(&first_dir)?;
    ensure_alive(&mut children[0], "first portable instance")?;

    if scenario == RuntimeScenario::WindowShell {
        return run_window_shell_lifecycle(&first_dir, &first_exe, &mut children[0]);
    }
    if scenario == RuntimeScenario::Phase10 {
        return run_phase10_lifecycle(&first_dir, &first_exe, &mut children[0]);
    }
    if scenario == RuntimeScenario::Phase11B {
        return run_phase11b_lifecycle(&first_dir, &mut children[0]);
    }

    if scenario == RuntimeScenario::Launch {
        return Ok(());
    }

    if scenario == RuntimeScenario::Assets {
        assert_asset_source_unchanged(&first_dir)?;
        return Ok(());
    }

    if matches!(scenario, RuntimeScenario::Preview | RuntimeScenario::Math) {
        let second_dir = root.join("split");
        let second_exe = copy_executable(&source, &second_dir)?;
        if scenario == RuntimeScenario::Math {
            prepare_math_layout(&second_dir, "split")?;
        } else {
            prepare_preview_layout(&second_dir, "split")?;
        }
        children.push(start(&second_exe)?);
        wait_for_layout(&second_dir)?;
        thread::sleep(Duration::from_secs(2));
        ensure_alive(&mut children[0], "Preview-mode portable instance")?;
        ensure_alive(&mut children[1], "Split-mode portable instance")?;
        if scenario == RuntimeScenario::Math {
            assert_math_source_unchanged(&first_dir)?;
            assert_math_source_unchanged(&second_dir)?;
        }
        return Ok(());
    }

    thread::sleep(Duration::from_millis(300));
    let note = first_dir.join("note/note.md");
    let config = first_dir.join("note/config.toml");
    let before = (file_state(&note)?, file_state(&config)?);
    let mut secondary = start(&first_exe)?;
    let secondary_status = wait_for_exit(&mut secondary, EXIT_TIMEOUT)?;
    if !secondary_status.success() {
        return Err(format!(
            "same-directory secondary exited unsuccessfully: {secondary_status}"
        ));
    }
    let after = (file_state(&note)?, file_state(&config)?);
    if before != after {
        return Err("same-directory secondary modified durable files".to_owned());
    }

    let second_dir = root.join("second");
    let second_exe = copy_executable(&source, &second_dir)?;
    children.push(start(&second_exe)?);
    wait_for_layout(&second_dir)?;
    ensure_alive(&mut children[0], "first portable instance")?;
    ensure_alive(&mut children[1], "different-directory portable instance")?;
    Ok(())
}

#[derive(Debug)]
struct StartupSample {
    external: Duration,
    milestones_us: Vec<(String, u128)>,
}

fn run_startup_measurement(repository: &Path, root: &Path) -> Result<RuntimeEvidence, String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let directory = root.join("phase9-startup");
    let executable = copy_executable(&source, &directory)?;
    prepare_resource_layout(&directory, "source", 0, 0, ImageResourceFixture::None)?;
    runtime_report!(
        "startup contract: fixture_bytes={} cold_samples={} cold_idle_seconds={} warm_samples={} warm_idle_ms={} preferred_ms={} engineering_ms={} release_boundary_ms={} ordering=interleaved",
        fs::metadata(directory.join("note/note.md"))
            .map_err(|error| format!("cannot inspect startup fixture: {error}"))?
            .len(),
        COLD_STARTUP_SAMPLE_COUNT,
        COLD_START_IDLE.as_secs(),
        WARM_STARTUP_SAMPLE_COUNT,
        WARM_START_IDLE.as_millis(),
        STARTUP_PREFERRED_TARGET.as_millis(),
        STARTUP_ENGINEERING_TARGET.as_millis(),
        V0_1_0_STARTUP_RELEASE_BOUNDARY.as_millis(),
    );

    let mut sequence = 0_u64;
    let mut cold = Vec::with_capacity(COLD_STARTUP_SAMPLE_COUNT);
    let mut warm = Vec::with_capacity(WARM_STARTUP_SAMPLE_COUNT);
    for run in 0..COLD_STARTUP_SAMPLE_COUNT {
        thread::sleep(COLD_START_IDLE);
        sequence = sequence.saturating_add(1);
        let sample = measure_editor_ready(&executable, &directory, sequence)?;
        print_startup_sample("cold", run + 1, &sample);
        cold.push(sample);

        thread::sleep(WARM_START_IDLE);
        sequence = sequence.saturating_add(1);
        let sample = measure_editor_ready(&executable, &directory, sequence)?;
        print_startup_sample("warm", run + 1, &sample);
        warm.push(sample);
    }

    for run in COLD_STARTUP_SAMPLE_COUNT..WARM_STARTUP_SAMPLE_COUNT {
        thread::sleep(WARM_START_IDLE);
        sequence = sequence.saturating_add(1);
        let sample = measure_editor_ready(&executable, &directory, sequence)?;
        print_startup_sample("warm", run + 1, &sample);
        warm.push(sample);
    }

    let cold_summary = print_startup_summary("cold", &cold, COLD_STARTUP_SAMPLE_COUNT)?;
    let warm_summary = print_startup_summary("warm", &warm, WARM_STARTUP_SAMPLE_COUNT)?;
    print_startup_thresholds("cold", cold_summary.p95);
    print_startup_thresholds("warm", warm_summary.p95);
    let gate_failure = if cold_summary.p95 > V0_1_0_STARTUP_RELEASE_BOUNDARY {
        Some(format!(
            "cold editor-ready p95 {:.3} ms exceeds the USER-approved v0.1.0 550 ms release boundary",
            cold_summary.p95.as_secs_f64() * 1_000.0
        ))
    } else if warm_summary.p95 > V0_1_0_STARTUP_RELEASE_BOUNDARY {
        Some(format!(
            "warm editor-ready p95 {:.3} ms exceeds the USER-approved v0.1.0 550 ms release boundary",
            warm_summary.p95.as_secs_f64() * 1_000.0
        ))
    } else {
        None
    };
    Ok(RuntimeEvidence {
        measurements: startup_measurements(cold_summary, warm_summary, &cold, &warm)?,
        gates: startup_gates(),
        samples: startup_samples(&cold, &warm),
        gate_failure,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StartupSummary {
    p50: Duration,
    p90: Duration,
    p95: Duration,
    p99: Duration,
    max: Duration,
    mean_us: u128,
    stddev_us: u128,
}

fn startup_measurements(
    cold: StartupSummary,
    warm: StartupSummary,
    cold_samples: &[StartupSample],
    warm_samples: &[StartupSample],
) -> Result<Vec<EvidenceMeasurement>, String> {
    let mut measurements = Vec::with_capacity(48);
    measurements.push(EvidenceMeasurement {
        name: "cold.samples".to_owned(),
        unit: "count".to_owned(),
        value: COLD_STARTUP_SAMPLE_COUNT as f64,
    });
    measurements.extend(startup_summary_measurements("cold", cold));
    measurements.push(EvidenceMeasurement {
        name: "warm.samples".to_owned(),
        unit: "count".to_owned(),
        value: WARM_STARTUP_SAMPLE_COUNT as f64,
    });
    measurements.extend(startup_summary_measurements("warm", warm));
    for (name, value) in [
        ("startup.preferred_target", STARTUP_PREFERRED_TARGET),
        ("startup.engineering_target", STARTUP_ENGINEERING_TARGET),
        (
            "startup.v0_1_0_release_boundary",
            V0_1_0_STARTUP_RELEASE_BOUNDARY,
        ),
    ] {
        measurements.push(EvidenceMeasurement {
            name: name.to_owned(),
            unit: "ms".to_owned(),
            value: value.as_secs_f64() * 1_000.0,
        });
    }
    measurements.extend(startup_category_measurements("cold", cold_samples)?);
    measurements.extend(startup_category_measurements("warm", warm_samples)?);
    Ok(measurements)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupThresholdClass {
    Preferred,
    Engineering,
    ReleaseOnly,
    Failed,
}

fn startup_threshold_class(p95: Duration) -> StartupThresholdClass {
    if p95 <= STARTUP_PREFERRED_TARGET {
        StartupThresholdClass::Preferred
    } else if p95 <= STARTUP_ENGINEERING_TARGET {
        StartupThresholdClass::Engineering
    } else if p95 <= V0_1_0_STARTUP_RELEASE_BOUNDARY {
        StartupThresholdClass::ReleaseOnly
    } else {
        StartupThresholdClass::Failed
    }
}

fn print_startup_thresholds(kind: &str, p95: Duration) {
    runtime_report!(
        "startup thresholds kind={kind} p95_ms={:.3} preferred_180={} engineering_400={} release_550={} class={:?}",
        p95.as_secs_f64() * 1_000.0,
        p95 <= STARTUP_PREFERRED_TARGET,
        p95 <= STARTUP_ENGINEERING_TARGET,
        p95 <= V0_1_0_STARTUP_RELEASE_BOUNDARY,
        startup_threshold_class(p95),
    );
}

const STARTUP_CATEGORIES: &[(&str, &str, &str)] = &[
    ("bootstrap", "main_enter", "event_loop_ready"),
    ("window_surface", "event_loop_ready", "font_system_begin"),
    ("font_discovery", "font_system_begin", "font_system_end"),
    (
        "source_layout",
        "font_system_end",
        "source_projection_ready",
    ),
    ("shell_setup", "source_projection_ready", "window_visible"),
    ("focus_guards", "window_visible", "editor_ready"),
];

fn startup_category_measurements(
    cohort: &str,
    samples: &[StartupSample],
) -> Result<Vec<EvidenceMeasurement>, String> {
    let mut measurements = Vec::with_capacity((STARTUP_CATEGORIES.len() + 1) * 2);
    let process_overhead = samples
        .iter()
        .map(|sample| {
            sample
                .external
                .saturating_sub(Duration::from_micros(
                    milestone_us(sample, "editor_ready") as u64
                ))
        })
        .collect::<Vec<_>>();
    extend_category_summary(
        &mut measurements,
        cohort,
        "process_overhead",
        process_overhead,
    )?;
    for (category, start, end) in STARTUP_CATEGORIES {
        let values = samples
            .iter()
            .map(|sample| {
                Duration::from_micros(
                    milestone_us(sample, end).saturating_sub(milestone_us(sample, start)) as u64,
                )
            })
            .collect::<Vec<_>>();
        extend_category_summary(&mut measurements, cohort, category, values)?;
    }
    Ok(measurements)
}

fn extend_category_summary(
    measurements: &mut Vec<EvidenceMeasurement>,
    cohort: &str,
    category: &str,
    mut values: Vec<Duration>,
) -> Result<(), String> {
    values.sort_unstable();
    for (statistic, value) in [
        ("p50", nearest_rank(&values, 50)?),
        ("p95", nearest_rank(&values, 95)?),
    ] {
        measurements.push(EvidenceMeasurement {
            name: format!("{cohort}.category.{category}.{statistic}"),
            unit: "ms".to_owned(),
            value: value.as_secs_f64() * 1_000.0,
        });
    }
    Ok(())
}

fn startup_summary_measurements(
    kind: &str,
    summary: StartupSummary,
) -> impl Iterator<Item = EvidenceMeasurement> {
    let durations = [
        ("p50", summary.p50),
        ("p90", summary.p90),
        ("p95", summary.p95),
        ("p99", summary.p99),
        ("max", summary.max),
    ];
    durations
        .into_iter()
        .map(move |(statistic, duration)| EvidenceMeasurement {
            name: format!("{kind}.{statistic}"),
            unit: "ms".to_owned(),
            value: duration.as_secs_f64() * 1_000.0,
        })
        .chain(
            [
                ("mean", summary.mean_us as f64 / 1_000.0),
                ("stddev", summary.stddev_us as f64 / 1_000.0),
            ]
            .into_iter()
            .map(move |(statistic, value)| EvidenceMeasurement {
                name: format!("{kind}.{statistic}"),
                unit: "ms".to_owned(),
                value,
            }),
        )
}

fn startup_gates() -> Vec<EvidenceGate> {
    let source = "docs/plan/10_performance_reliability.md#initial-engineering-targets";
    vec![
        EvidenceGate {
            metric: "cold.p95".to_owned(),
            comparator: "<=".to_owned(),
            value: V0_1_0_STARTUP_RELEASE_BOUNDARY.as_secs_f64() * 1_000.0,
            unit: "ms".to_owned(),
            source: source.to_owned(),
        },
        EvidenceGate {
            metric: "warm.p95".to_owned(),
            comparator: "<=".to_owned(),
            value: V0_1_0_STARTUP_RELEASE_BOUNDARY.as_secs_f64() * 1_000.0,
            unit: "ms".to_owned(),
            source: source.to_owned(),
        },
    ]
}

fn startup_samples(cold: &[StartupSample], warm: &[StartupSample]) -> Vec<EvidenceSample> {
    let mut samples = Vec::with_capacity(cold.len() + warm.len());
    for (cohort, cohort_samples) in [("cold", cold), ("warm", warm)] {
        for (index, sample) in cohort_samples.iter().enumerate() {
            let internal_us = milestone_us(sample, "editor_ready");
            let external_us = sample.external.as_micros();
            let mut measurements = Vec::with_capacity(sample.milestones_us.len() + 3);
            measurements.push(EvidenceMeasurement {
                name: "external".to_owned(),
                unit: "ms".to_owned(),
                value: external_us as f64 / 1_000.0,
            });
            measurements.push(EvidenceMeasurement {
                name: "internal".to_owned(),
                unit: "ms".to_owned(),
                value: internal_us as f64 / 1_000.0,
            });
            measurements.push(EvidenceMeasurement {
                name: "process_overhead".to_owned(),
                unit: "ms".to_owned(),
                value: external_us.saturating_sub(internal_us) as f64 / 1_000.0,
            });
            measurements.extend(sample.milestones_us.iter().map(|(name, value)| {
                EvidenceMeasurement {
                    name: format!("milestone.{name}"),
                    unit: "ms".to_owned(),
                    value: *value as f64 / 1_000.0,
                }
            }));
            samples.push(EvidenceSample {
                cohort: cohort.to_owned(),
                run: index + 1,
                measurements,
            });
        }
    }
    samples
}

fn measure_editor_ready(
    executable: &Path,
    directory: &Path,
    sequence: u64,
) -> Result<StartupSample, String> {
    let ready = ReadyEvent::create(sequence)?;
    let trace = directory.join(format!("startup-trace-{sequence}.txt"));
    let started = Instant::now();
    let mut child = Command::new(executable)
        .current_dir(directory)
        .env("STICKYMD_DIAGNOSTIC_READY_EVENT", ready.name())
        .env("STICKYMD_DIAGNOSTIC_STARTUP_TRACE", &trace)
        .env("STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", executable.display()))?;
    let result = (|| {
        ready.wait(START_TIMEOUT)?;
        let external = started.elapsed();
        ensure_alive(&mut child, "startup measurement instance")?;
        let milestones_us = wait_for_startup_trace(&trace)?;
        validate_startup_milestones(&milestones_us)?;
        let status = wait_for_exit(&mut child, EXIT_TIMEOUT)?;
        if !status.success() {
            return Err(format!(
                "diagnostic startup instance exited unsuccessfully: {status}"
            ));
        }
        Ok(StartupSample {
            external,
            milestones_us,
        })
    })();
    stop_child(&mut child);
    result
}

fn wait_for_startup_trace(path: &Path) -> Result<Vec<(String, u128)>, String> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match fs::read_to_string(path) {
            Ok(content) => {
                let mut milestones = Vec::new();
                for line in content.lines().skip(1) {
                    let (name, value) = line
                        .split_once('=')
                        .ok_or_else(|| format!("invalid startup trace line `{line}`"))?;
                    milestones.push((
                        name.to_owned(),
                        value.parse::<u128>().map_err(|error| {
                            format!("invalid startup duration `{line}`: {error}")
                        })?,
                    ));
                }
                return Ok(milestones);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "cannot read startup trace {}: {error}",
                    path.display()
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(format!("startup trace was not written: {}", path.display()));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn validate_startup_milestones(milestones: &[(String, u128)]) -> Result<(), String> {
    const EXPECTED: &[&str] = &[
        "process_start",
        "main_enter",
        "program_dir_ready",
        "single_instance_ready",
        "persistence_ready",
        "config_ready",
        "document_ready",
        "event_loop_ready",
        "window_created",
        "surface_ready",
        "display_ready",
        "font_system_begin",
        "source_layout_begin",
        "font_system_end",
        "source_buffer_ready",
        "source_layout_end",
        "source_projection_ready",
        "monitor_ready",
        "tray_ready",
        "window_visible",
        "opacity_ready",
        "topmost_ready",
        "focus_ready",
        "guards_ready",
        "shell_ready",
        "editor_ready",
    ];
    let names = milestones
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    if names != EXPECTED {
        return Err(format!("startup milestone order mismatch: {names:?}"));
    }
    if milestones.windows(2).any(|pair| pair[0].1 > pair[1].1) {
        return Err("startup milestone durations are not monotonic".to_owned());
    }
    Ok(())
}

fn print_startup_sample(kind: &str, run: usize, sample: &StartupSample) {
    let internal = sample
        .milestones_us
        .last()
        .map_or(0.0, |(_, value)| *value as f64 / 1_000.0);
    let font_begin = milestone_us(sample, "font_system_begin");
    let font_end = milestone_us(sample, "font_system_end");
    runtime_report!(
        "startup sample kind={kind} run={run} external_ms={:.3} internal_ms={internal:.3} process_overhead_ms={:.3} font_system_ms={:.3}",
        sample.external.as_secs_f64() * 1_000.0,
        (sample.external.as_secs_f64() * 1_000.0 - internal).max(0.0),
        font_end.saturating_sub(font_begin) as f64 / 1_000.0,
    );
}

fn milestone_us(sample: &StartupSample, name: &str) -> u128 {
    sample
        .milestones_us
        .iter()
        .find_map(|(observed, value)| (observed == name).then_some(*value))
        .unwrap_or_default()
}

fn print_startup_summary(
    kind: &str,
    samples: &[StartupSample],
    expected_count: usize,
) -> Result<StartupSummary, String> {
    if samples.len() != expected_count {
        return Err(format!("{kind} startup produced {} samples", samples.len()));
    }
    let mut external = samples
        .iter()
        .map(|sample| sample.external)
        .collect::<Vec<_>>();
    external.sort_unstable();
    let p50 = nearest_rank(&external, 50)?;
    let p90 = nearest_rank(&external, 90)?;
    let p95 = nearest_rank(&external, 95)?;
    let p99 = nearest_rank(&external, 99)?;
    let max = *external
        .last()
        .ok_or_else(|| "startup samples are empty".to_owned())?;
    let total_us = external
        .iter()
        .try_fold(0_u128, |total, sample| {
            total.checked_add(sample.as_micros())
        })
        .ok_or_else(|| "startup sample duration sum overflowed".to_owned())?;
    let mean_us = total_us / external.len() as u128;
    let variance = external.iter().fold(0.0_f64, |total, sample| {
        let delta = sample.as_micros() as f64 - mean_us as f64;
        total + delta * delta
    }) / external.len() as f64;
    let stddev_us = variance.sqrt().round() as u128;
    runtime_report!(
        "startup summary kind={kind} samples={} p50_ms={:.3} p90_ms={:.3} p95_ms={:.3} p99_ms={:.3} max_ms={:.3} mean_ms={:.3} stddev_ms={:.3}",
        external.len(),
        p50.as_secs_f64() * 1_000.0,
        p90.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        p99.as_secs_f64() * 1_000.0,
        max.as_secs_f64() * 1_000.0,
        mean_us as f64 / 1_000.0,
        stddev_us as f64 / 1_000.0,
    );
    for name in [
        "program_dir_ready",
        "single_instance_ready",
        "persistence_ready",
        "config_ready",
        "document_ready",
        "event_loop_ready",
        "window_created",
        "surface_ready",
        "display_ready",
        "font_system_end",
        "source_buffer_ready",
        "source_layout_end",
        "source_projection_ready",
        "monitor_ready",
        "tray_ready",
        "window_visible",
        "opacity_ready",
        "topmost_ready",
        "focus_ready",
        "guards_ready",
        "shell_ready",
        "editor_ready",
    ] {
        let mut values = samples
            .iter()
            .map(|sample| Duration::from_micros(milestone_us(sample, name) as u64))
            .collect::<Vec<_>>();
        values.sort_unstable();
        runtime_report!(
            "startup milestone kind={kind} name={name} p50_ms={:.3} p95_ms={:.3}",
            nearest_rank(&values, 50)?.as_secs_f64() * 1_000.0,
            nearest_rank(&values, 95)?.as_secs_f64() * 1_000.0,
        );
    }
    Ok(StartupSummary {
        p50,
        p90,
        p95,
        p99,
        max,
        mean_us,
        stddev_us,
    })
}

fn nearest_rank(samples: &[Duration], percentile: usize) -> Result<Duration, String> {
    if samples.is_empty() || !(1..=100).contains(&percentile) {
        return Err("nearest-rank percentile requires samples and 1..=100".to_owned());
    }
    let rank = samples.len().saturating_mul(percentile).div_ceil(100);
    samples
        .get(rank.saturating_sub(1))
        .copied()
        .ok_or_else(|| "percentile rank is outside startup samples".to_owned())
}

fn run_window_shell_lifecycle(
    program_directory: &Path,
    executable: &Path,
    primary: &mut Child,
) -> Result<(), String> {
    let window = crate::window_control::visible_window(primary.id())?;
    runtime_report!(
        "Phase 8 runtime paper rect={:?}",
        crate::window_control::window_rect(window)?
    );
    // The HWND becomes observable immediately after `ShowWindow`, while the
    // UI thread applies its configured layered alpha in the following native
    // projection step. Poll the native fact instead of racing those two calls.
    wait_for_layered_alpha(window, Some(245))?;

    // Drive one real native move-loop cycle through the copied executable.
    // Pure reducer tests cover every edge and exact DIP boundary; this receipt
    // proves the Win32/winit bridge can dock, collapse, reveal, and detach a
    // real HWND without a product-only test command channel.
    crate::window_control::move_to_primary_left_edge(window)?;
    wait_for_config_field(program_directory, "dock_edge = \"left\"")?;
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Collapse)?;
    wait_for_primary_left_state(window, true)?;
    reveal_primary_left_and_wait(window)?;
    crate::window_control::move_to_primary_inset(window, 32)?;
    wait_for_config_field(program_directory, "dock_edge = \"none\"")?;

    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Topmost)?;
    wait_for_config_field(program_directory, "always_on_top = true")?;
    if !crate::window_control::is_topmost(window)? {
        return Err("configured topmost did not reach the real HWND".to_owned());
    }
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Topmost)?;
    wait_for_config_field(program_directory, "always_on_top = false")?;
    if crate::window_control::is_topmost(window)? {
        return Err("cleared configured topmost remained on the real HWND".to_owned());
    }

    for theme in ["system", "dark", "light"] {
        crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)?;
        wait_for_config_field(program_directory, &format!("theme = \"{theme}\""))?;
    }

    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    crate::window_control::commit_opacity_slider(window, 70)?;
    wait_for_config_field(program_directory, "opacity = 70")?;
    wait_for_layered_alpha(window, Some(179))?;
    crate::window_control::commit_opacity_slider(window, 100)?;
    wait_for_config_field(program_directory, "opacity = 100")?;
    wait_for_layered_alpha(window, None)?;

    crate::window_control::request_close(window)?;
    if let Err(error) = wait_for_window_visibility(window, false) {
        let process_state = match primary
            .try_wait()
            .map_err(|inspect| format!("cannot inspect close-to-tray process: {inspect}"))?
        {
            Some(status) => format!("exited with {status}"),
            None => "still running".to_owned(),
        };
        return Err(format!("{error}; primary process is {process_state}"));
    }
    ensure_alive(primary, "close-to-tray primary instance")?;

    // Let the primary finish any close-triggered save before isolating the
    // secondary-instance wake effect.
    thread::sleep(Duration::from_millis(300));
    let note = program_directory.join("note/note.md");
    let config = program_directory.join("note/config.toml");
    let before = (file_state(&note)?, file_state(&config)?);
    let mut secondary = start(executable)?;
    let secondary_status = wait_for_exit(&mut secondary, EXIT_TIMEOUT)?;
    if !secondary_status.success() {
        return Err(format!(
            "same-directory wake instance exited unsuccessfully: {secondary_status}"
        ));
    }
    wait_for_window_visibility(window, true)?;
    ensure_alive(primary, "woken primary instance")?;
    let after = (file_state(&note)?, file_state(&config)?);
    if before != after {
        return Err("secondary-instance wake modified durable files".to_owned());
    }

    let smoke_root = program_directory
        .parent()
        .ok_or_else(|| "Phase 8 program directory has no smoke root".to_owned())?;
    let isolated_directory = smoke_root.join("phase8-independent");
    let isolated_executable = copy_executable(executable, &isolated_directory)?;
    prepare_isolation_layout(&isolated_directory)?;
    let mut isolated = start(&isolated_executable)?;
    let isolated_result = (|| {
        wait_for_layout(&isolated_directory)?;
        let isolated_window = crate::window_control::visible_window(isolated.id())?;
        if isolated_window == window {
            return Err("different-directory instance reused the first HWND".to_owned());
        }
        ensure_alive(primary, "first portable instance during isolation smoke")?;
        ensure_alive(&mut isolated, "different-directory portable instance")?;
        let first_note = fs::read(program_directory.join("note/note.md"))
            .map_err(|error| format!("cannot read first isolated note: {error}"))?;
        let second_note = fs::read(isolated_directory.join("note/note.md"))
            .map_err(|error| format!("cannot read second isolated note: {error}"))?;
        if first_note == second_note {
            return Err("different-directory notes were not independently seeded".to_owned());
        }
        Ok(())
    })();
    stop_child(&mut isolated);
    isolated_result?;
    Ok(())
}

fn run_phase10_lifecycle(
    program_directory: &Path,
    executable: &Path,
    primary: &mut Child,
) -> Result<(), String> {
    let window = crate::window_control::visible_window(primary.id())?;
    let initial_style = wait_for_tool_window_style(window)?;

    run_window_shell_lifecycle(program_directory, executable, primary)?;
    let restored_style = wait_for_tool_window_style(window)?;

    crate::window_control::resize_to_dip(window, 220, 120)?;
    wait_for_config_field(program_directory, "width_dip = 220")?;
    wait_for_config_field(program_directory, "height_dip = 120")?;
    let compact = crate::window_control::window_rect(window)?;
    if compact.width == 0 || compact.height == 0 {
        return Err("Phase 10 compact HWND has an empty extent".to_owned());
    }

    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    crate::window_control::commit_opacity_slider(window, 40)?;
    wait_for_config_field(program_directory, "opacity = 40")?;
    wait_for_layered_alpha(window, Some(102))?;
    runtime_report!(
        "Phase 10 runtime initial_style={initial_style:?} restored_style={restored_style:?} compact_rect={compact:?} opacity=40"
    );
    Ok(())
}

fn run_phase11b_lifecycle(program_directory: &Path, primary: &mut Child) -> Result<(), String> {
    let window = crate::window_control::visible_window(primary.id())?;
    let note = program_directory.join("note/note.md");
    crate::window_control::click_math_conversion(window)?;
    wait_for_note(&note, |bytes| bytes == PHASE11B_CONVERTED.as_bytes())?;
    ensure_alive(primary, "post Phase 11-B toolbar-conversion lifecycle")?;
    runtime_report!(
        "Phase 11-B runtime toolbar conversion, literal safety, and autosave lifecycle PASS"
    );
    Ok(())
}

fn wait_for_tool_window_style(
    window: crate::window_control::WindowHandle,
) -> Result<crate::window_control::WindowStyleFacts, String> {
    let deadline = Instant::now() + START_TIMEOUT;
    let mut observed = crate::window_control::style_facts(window)?;
    while Instant::now() < deadline {
        observed = crate::window_control::style_facts(window)?;
        if observed.tool_window
            && !observed.app_window
            && !observed.no_activate
            && !observed.transparent
        {
            return Ok(observed);
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "Phase 10 tool-window style invariant failed: {observed:?}"
    ))
}

fn wait_for_config_field(program_directory: &Path, expected: &str) -> Result<(), String> {
    let path = program_directory.join("note/config.toml");
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if fs::read_to_string(&path).is_ok_and(|source| source.contains(expected)) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "config did not contain `{expected}` within {} seconds: {}",
        START_TIMEOUT.as_secs(),
        path.display()
    ))
}

fn prepare_isolation_layout(program_directory: &Path) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory)
        .map_err(|error| format!("cannot create isolated note directory: {error}"))?;
    fs::write(
        note_directory.join("note.md"),
        "# Independent Phase 8 portable instance\n",
    )
    .map_err(|error| format!("cannot seed isolated note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        "version = 1\nview_mode = \"source\"\n",
    )
    .map_err(|error| format!("cannot seed isolated config: {error}"))
}

fn wait_for_window_visibility(
    window: crate::window_control::WindowHandle,
    expected: bool,
) -> Result<(), String> {
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if crate::window_control::is_visible(window)? == expected {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(format!(
        "StickyMD window did not become {} within {} seconds",
        if expected { "visible" } else { "hidden" },
        START_TIMEOUT.as_secs()
    ))
}

fn wait_for_layered_alpha(
    window: crate::window_control::WindowHandle,
    expected: Option<u8>,
) -> Result<(), String> {
    let deadline = Instant::now() + START_TIMEOUT;
    let mut observed = None;
    while Instant::now() < deadline {
        let alpha = crate::window_control::layered_alpha(window)?;
        observed = Some(alpha);
        let matches = match expected {
            Some(value) => alpha.layered && alpha.alpha == Some(value),
            None => !alpha.layered && alpha.alpha.is_none(),
        };
        if matches {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "whole-window alpha did not become {expected:?} within {} seconds; last observation={observed:?}",
        START_TIMEOUT.as_secs()
    ))
}

struct ResourceCase {
    label: &'static str,
    view_mode: &'static str,
    formula_count: usize,
    image_count: usize,
    image_fixture: ImageResourceFixture,
    measure_cpu: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageResourceFixture {
    None,
    FourK,
    SaturatedCache,
}

fn run_resource_measurement(
    repository: &Path,
    root: &Path,
    math_matrix: bool,
    image_matrix: bool,
) -> Result<Vec<EvidenceMeasurement>, String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let logical_processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    runtime_report!(
        "resource contract: warmup={}s repetitions={} cpu_interval={}s logical_processors={logical_processors}",
        RESOURCE_WARMUP.as_secs(),
        RESOURCE_REPETITIONS,
        CPU_INTERVAL.as_secs(),
    );
    let mut cases = if image_matrix {
        vec![
            ResourceCase {
                label: "source-no-images",
                view_mode: "source",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
            ResourceCase {
                label: "source-12-images-lazy",
                view_mode: "source",
                formula_count: 0,
                image_count: 12,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "preview-no-images",
                view_mode: "preview",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
            ResourceCase {
                label: "preview-1-image",
                view_mode: "preview",
                formula_count: 0,
                image_count: 1,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
            ResourceCase {
                label: "preview-12-images",
                view_mode: "preview",
                formula_count: 0,
                image_count: 12,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "split-12-images",
                view_mode: "split",
                formula_count: 0,
                image_count: 12,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "preview-4k-image",
                view_mode: "preview",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::FourK,
                measure_cpu: false,
            },
            ResourceCase {
                label: "preview-image-cache-saturated",
                view_mode: "preview",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::SaturatedCache,
                measure_cpu: true,
            },
            ResourceCase {
                label: "split-image-cache-saturated",
                view_mode: "split",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::SaturatedCache,
                measure_cpu: true,
            },
            ResourceCase {
                label: "source-after-preview-cache-release",
                view_mode: "preview",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::SaturatedCache,
                measure_cpu: true,
            },
        ]
    } else if math_matrix {
        vec![
            ResourceCase {
                label: "source-20-math-lazy",
                view_mode: "source",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "preview-no-math",
                view_mode: "preview",
                formula_count: 0,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
            ResourceCase {
                label: "preview-1-math",
                view_mode: "preview",
                formula_count: 1,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
            ResourceCase {
                label: "preview-20-math",
                view_mode: "preview",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "split-20-math",
                view_mode: "split",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "preview-200-unique",
                view_mode: "preview",
                formula_count: 200,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: false,
            },
        ]
    } else {
        vec![
            ResourceCase {
                label: "source",
                view_mode: "source",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "preview",
                view_mode: "preview",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
            ResourceCase {
                label: "split",
                view_mode: "split",
                formula_count: 20,
                image_count: 0,
                image_fixture: ImageResourceFixture::None,
                measure_cpu: true,
            },
        ]
    };
    if let Ok(filter) = std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
        && !filter.is_empty()
    {
        cases.retain(|case| case.label == filter);
        if cases.len() != 1 {
            return Err(format!("unknown resource case filter `{filter}`"));
        }
        runtime_report!("resource development filter: {filter}");
    }
    let mut evidence = Vec::new();
    for case in cases {
        let mode = case.label;
        let mut memory_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
        let mut cpu_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
        for repetition in 0..RESOURCE_REPETITIONS {
            let directory = root.join(format!("{mode}-{repetition}"));
            let executable = copy_executable(&source, &directory)?;
            prepare_resource_layout(
                &directory,
                case.view_mode,
                case.formula_count,
                case.image_count,
                case.image_fixture,
            )?;
            let mut child = start(&executable)?;
            wait_for_layout(&directory)?;
            // A resource baseline represents a truly idle window. Keep the
            // physical cursor outside the paper so incidental mouse jitter or
            // operator movement cannot turn preview hit-testing and title
            // updates into process CPU attributed to the idle sample.
            let window = crate::window_control::visible_window(child.id())?;
            crate::window_control::park_cursor_outside_window(window)?;
            thread::sleep(RESOURCE_WARMUP);
            ensure_alive(&mut child, "resource measurement instance")?;
            if mode == "source-after-preview-cache-release" {
                crate::window_control::switch_to_source(child.id())?;
                wait_for_view_mode(&directory, "source")?;
                thread::sleep(Duration::from_secs(5));
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
            memory_samples.push(sample);
            if case.measure_cpu {
                cpu_samples.push(measure_idle_cpu(
                    &mut child,
                    mode,
                    logical_processors,
                    window,
                )?);
            }
            stop_child(&mut child);
        }
        print_resource_summary(mode, &memory_samples, &cpu_samples)?;
        evidence.extend(memory_measurements(mode, &memory_samples));
        evidence.extend(cpu_measurements(mode, &cpu_samples));
        if cpu_samples
            .iter()
            .any(|sample| *sample > IDLE_CPU_PERCENT_LIMIT)
        {
            return Err(format!(
                "{mode} idle CPU sample exceeds {:.3}%: {cpu_samples:?}",
                IDLE_CPU_PERCENT_LIMIT
            ));
        }
    }
    Ok(evidence)
}

fn run_zoom_resource_measurement(
    repository: &Path,
    root: &Path,
) -> Result<Vec<EvidenceMeasurement>, String> {
    const SPLIT_PRIVATE_WORKING_SET_LIMIT: u64 = 64 * 1024 * 1024;
    let source = repository.join("target/release/stickymd-win.exe");
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
    let mut evidence = Vec::new();
    for zoom in [50_u16, 100, 300] {
        let label = format!("split-zoom-{zoom}");
        let mut samples = Vec::with_capacity(RESOURCE_REPETITIONS);
        for repetition in 0..RESOURCE_REPETITIONS {
            let directory = root.join(format!("{label}-{repetition}"));
            let executable = copy_executable(&source, &directory)?;
            prepare_resource_layout(&directory, "split", 20, 12, ImageResourceFixture::None)?;
            set_resource_zoom(&directory, zoom)?;
            let mut child = start(&executable)?;
            let result = (|| {
                wait_for_layout(&directory)?;
                let window = crate::window_control::visible_window(child.id())?;
                crate::window_control::park_cursor_outside_window(window)?;
                thread::sleep(ZOOM_RESOURCE_WARMUP);
                ensure_alive(&mut child, "Phase 10 zoom resource instance")?;
                if zoom == 100 && repetition == 0 {
                    let growth =
                        verify_zoom_relayout_does_not_leak(&directory, &mut child, window)?;
                    evidence.push(EvidenceMeasurement {
                        name: "zoom_cycles.private_growth".to_owned(),
                        unit: "bytes".to_owned(),
                        value: growth as f64,
                    });
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
            samples.push(sample);
        }
        print_resource_summary(&label, &samples, &[])?;
        evidence.extend(memory_measurements(&label, &samples));
        let observed_max = samples
            .iter()
            .map(|sample| sample.private_working_set_bytes)
            .max()
            .unwrap_or_default();
        if observed_max > SPLIT_PRIVATE_WORKING_SET_LIMIT {
            return Err(format!(
                "{label} private working set max {observed_max} exceeds {SPLIT_PRIVATE_WORKING_SET_LIMIT} bytes"
            ));
        }
    }
    Ok(evidence)
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
    if after.private_bytes
        > before
            .private_bytes
            .saturating_add(ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT)
    {
        return Err(format!(
            "Phase 10 repeated zoom relayout grew private bytes by more than {} bytes",
            ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT
        ));
    }
    Ok(after.private_bytes as i64 - before.private_bytes as i64)
}

fn memory_measurements(mode: &str, samples: &[MemorySample]) -> Vec<EvidenceMeasurement> {
    let mut private_working_set = samples
        .iter()
        .map(|sample| sample.private_working_set_bytes)
        .collect::<Vec<_>>();
    let mut private_bytes = samples
        .iter()
        .map(|sample| sample.private_bytes)
        .collect::<Vec<_>>();
    private_working_set.sort_unstable();
    private_bytes.sort_unstable();
    let middle = samples.len() / 2;
    [
        ("private_working_set_median", private_working_set[middle]),
        (
            "private_working_set_max",
            private_working_set[samples.len() - 1],
        ),
        ("private_bytes_median", private_bytes[middle]),
        ("private_bytes_max", private_bytes[samples.len() - 1]),
    ]
    .into_iter()
    .map(|(statistic, value)| EvidenceMeasurement {
        name: format!("{mode}.{statistic}"),
        unit: "bytes".to_owned(),
        value: value as f64,
    })
    .collect()
}

fn cpu_measurements(mode: &str, samples: &[f64]) -> Vec<EvidenceMeasurement> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    [
        ("idle_cpu_median", sorted[middle]),
        ("idle_cpu_p95", sorted[nearest_rank_index(sorted.len(), 95)]),
        ("idle_cpu_max", sorted[sorted.len() - 1]),
    ]
    .into_iter()
    .map(|(statistic, value)| EvidenceMeasurement {
        name: format!("{mode}.{statistic}"),
        unit: "percent".to_owned(),
        value,
    })
    .collect()
}

fn duration_measurements(mode: &str, samples: &[Duration]) -> Vec<EvidenceMeasurement> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let middle = sorted.len() / 2;
    [
        ("median", sorted[middle]),
        ("max", sorted[sorted.len() - 1]),
    ]
    .into_iter()
    .map(|(statistic, value)| EvidenceMeasurement {
        name: format!("{mode}.{statistic}"),
        unit: "ms".to_owned(),
        value: value.as_secs_f64() * 1_000.0,
    })
    .collect()
}

fn run_window_resource_measurement(
    repository: &Path,
    root: &Path,
) -> Result<Vec<EvidenceMeasurement>, String> {
    let source = repository.join("target/release/stickymd-win.exe");
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
    let mut visible_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut collapsed_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut hidden_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut startup_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut visible_cpu = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut collapsed_cpu = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut hidden_cpu = Vec::with_capacity(RESOURCE_REPETITIONS);
    for repetition in 0..RESOURCE_REPETITIONS {
        let directory = root.join(format!("window-resource-{repetition}"));
        let executable = copy_executable(&source, &directory)?;
        prepare_resource_layout(&directory, "source", 0, 0, ImageResourceFixture::None)?;
        let startup_started = Instant::now();
        let mut child = start(&executable)?;
        let result = (|| {
            wait_for_layout(&directory)?;
            let window = crate::window_control::visible_window(child.id())?;
            let startup = startup_started.elapsed();
            runtime_report!(
                "window startup run={} elapsed_ms={:.3}",
                repetition + 1,
                startup.as_secs_f64() * 1_000.0
            );
            thread::sleep(RESOURCE_WARMUP);
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
            visible_cpu.push(measure_idle_cpu(
                &mut child,
                "visible-source",
                logical_processors,
                window,
            )?);
            crate::window_control::move_to_primary_left_edge(window)?;
            wait_for_config_field(&directory, "dock_edge = \"left\"")?;
            crate::window_control::park_cursor_at_primary_right(window)?;
            crate::window_control::click_toolbar(
                window,
                crate::window_control::ToolbarControl::Collapse,
            )?;
            wait_for_primary_left_state(window, true)?;
            thread::sleep(RESOURCE_WARMUP);
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
            collapsed_cpu.push(measure_idle_cpu(
                &mut child,
                "docked-collapsed",
                logical_processors,
                window,
            )?);
            if repetition == 0 {
                run_window_leak_cycles(&directory, &executable, &mut child, window)?;
            }
            crate::window_control::request_close(window)?;
            wait_for_window_visibility(window, false)?;
            thread::sleep(RESOURCE_WARMUP);
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
            hidden_cpu.push(measure_idle_cpu(
                &mut child,
                "hidden-to-tray",
                logical_processors,
                window,
            )?);
            Ok::<_, String>((startup, visible, collapsed, hidden))
        })();
        stop_child(&mut child);
        let (startup, visible, collapsed, hidden) = result?;
        startup_samples.push(startup);
        visible_samples.push(visible);
        collapsed_samples.push(collapsed);
        hidden_samples.push(hidden);
    }
    print_duration_summary("startup-to-paper", &mut startup_samples)?;
    print_resource_summary("visible-source", &visible_samples, &visible_cpu)?;
    print_resource_summary("docked-collapsed", &collapsed_samples, &collapsed_cpu)?;
    print_resource_summary("hidden-to-tray", &hidden_samples, &hidden_cpu)?;
    let mut evidence = duration_measurements("window.startup_to_paper", &startup_samples);
    for (mode, memory, cpu) in [
        ("visible-source", &visible_samples, &visible_cpu),
        ("docked-collapsed", &collapsed_samples, &collapsed_cpu),
        ("hidden-to-tray", &hidden_samples, &hidden_cpu),
    ] {
        evidence.extend(memory_measurements(mode, memory));
        evidence.extend(cpu_measurements(mode, cpu));
    }
    let observed_max = hidden_samples
        .iter()
        .map(|sample| sample.private_working_set_bytes)
        .max()
        .unwrap_or_default();
    if observed_max > HIDDEN_PRIVATE_WORKING_SET_LIMIT {
        return Err(format!(
            "hidden-to-tray private working set max {observed_max} exceeds {} bytes",
            HIDDEN_PRIVATE_WORKING_SET_LIMIT
        ));
    }
    for (mode, cpu_samples) in [
        ("visible-source", &visible_cpu),
        ("docked-collapsed", &collapsed_cpu),
        ("hidden-to-tray", &hidden_cpu),
    ] {
        if cpu_samples
            .iter()
            .any(|sample| *sample > IDLE_CPU_PERCENT_LIMIT)
        {
            return Err(format!(
                "{mode} idle CPU sample exceeds {:.3}%: {cpu_samples:?}",
                IDLE_CPU_PERCENT_LIMIT,
            ));
        }
    }
    Ok(evidence)
}

fn measure_idle_cpu(
    child: &mut Child,
    mode: &str,
    logical_processors: usize,
    window: crate::window_control::WindowHandle,
) -> Result<f64, String> {
    const BUCKETS: u32 = 6;
    let before = process_metrics::cpu_time(child)?;
    let wall_started = Instant::now();
    let bucket_interval = CPU_INTERVAL / BUCKETS;
    let mut previous_cpu = before;
    let mut previous_wall = wall_started;
    for bucket in 0..BUCKETS {
        thread::sleep(bucket_interval);
        ensure_alive(child, &format!("{mode} idle CPU instance"))?;
        let current_wall = Instant::now();
        let current_cpu = process_metrics::cpu_time(child)?;
        let bucket_cpu = current_cpu.saturating_sub(previous_cpu).as_secs_f64()
            / current_wall.duration_since(previous_wall).as_secs_f64()
            / logical_processors as f64
            * 100.0;
        let memory = process_metrics::memory(child)?;
        let rect = crate::window_control::window_rect(window)?;
        runtime_report!(
            "resource cpu bucket mode={mode} bucket={}/{} average_percent={bucket_cpu:.6} \
             private_working_set_bytes={} private_bytes={} window_x={} window_y={} \
             window_width={} window_height={}",
            bucket + 1,
            BUCKETS,
            memory.private_working_set_bytes,
            memory.private_bytes,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        previous_cpu = current_cpu;
        previous_wall = current_wall;
    }
    let elapsed = wall_started.elapsed();
    let after = process_metrics::cpu_time(child)?;
    let cpu = after.saturating_sub(before).as_secs_f64()
        / elapsed.as_secs_f64()
        / logical_processors as f64
        * 100.0;
    runtime_report!(
        "resource cpu mode={mode} interval_seconds={:.3} average_percent={cpu:.6}",
        elapsed.as_secs_f64()
    );
    Ok(cpu)
}

fn wait_for_primary_left_state(
    window: crate::window_control::WindowHandle,
    collapsed: bool,
) -> Result<(), String> {
    let expectation = if collapsed {
        ShellStateExpectation::PrimaryLeftCollapsed
    } else {
        ShellStateExpectation::PrimaryLeftExpanded
    };
    wait_for_shell_state(window, expectation, START_TIMEOUT).map(|_| ())
}

fn wait_for_primary_left_state_with_timeout(
    window: crate::window_control::WindowHandle,
    collapsed: bool,
    timeout: Duration,
) -> Result<(), String> {
    let expectation = if collapsed {
        ShellStateExpectation::PrimaryLeftCollapsed
    } else {
        ShellStateExpectation::PrimaryLeftExpanded
    };
    wait_for_shell_state(window, expectation, timeout).map(|_| ())
}

fn wait_for_shell_state(
    window: crate::window_control::WindowHandle,
    expectation: ShellStateExpectation,
    timeout: Duration,
) -> Result<ShellObservation, String> {
    let deadline = Instant::now() + timeout;
    let mut last = None;
    while Instant::now() < deadline {
        let observed = observe_shell(window)?;
        if shell_matches(&observed, expectation) {
            return Ok(observed);
        }
        last = Some(observed);
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "expected={expectation:?} timeout_seconds={:.3} actual={}",
        timeout.as_secs_f64(),
        last.as_ref()
            .map_or_else(|| "unobserved".to_owned(), format_shell_observation)
    ))
}

fn observe_shell(window: crate::window_control::WindowHandle) -> Result<ShellObservation, String> {
    let rect = crate::window_control::window_rect(window)?;
    thread::sleep(Duration::from_millis(20));
    let final_rect = crate::window_control::window_rect(window)?;
    Ok(ShellObservation {
        visible: crate::window_control::is_visible(window)?,
        rect: final_rect,
        work: crate::window_control::primary_work_area()?,
        activation: crate::window_control::activation_facts(window)?,
        cursor: crate::window_control::cursor_facts(window)?,
        style: crate::window_control::style_facts(window)?,
        topmost: crate::window_control::is_topmost(window)?,
        alpha: crate::window_control::layered_alpha(window)?,
        title: crate::window_control::title(window)?,
        stable_geometry: rect == final_rect,
    })
}

fn shell_matches(observed: &ShellObservation, expectation: ShellStateExpectation) -> bool {
    let sensor_right =
        i64::from(observed.rect.x) + i64::from(observed.rect.width) - i64::from(observed.work.x);
    let collapsed = observed.rect.x < observed.work.x && (1..=16).contains(&sensor_right);
    let expanded = (observed.rect.x - observed.work.x).abs() <= 1;
    match expectation {
        ShellStateExpectation::Visible => observed.visible && observed.stable_geometry,
        ShellStateExpectation::Hidden => !observed.visible,
        ShellStateExpectation::PrimaryLeftCollapsed => {
            observed.visible && observed.stable_geometry && collapsed
        }
        ShellStateExpectation::PrimaryLeftExpanded => {
            observed.visible && observed.stable_geometry && expanded
        }
        ShellStateExpectation::EditorInputReady => {
            observed.visible
                && observed.stable_geometry
                && observed.activation.foreground
                && observed.activation.active
                && observed.activation.focused
        }
    }
}

fn format_shell_observation(observed: &ShellObservation) -> String {
    format!(
        "visible={} stable_geometry={} rect={:?} work={:?} foreground={} active={} focused={} captured={} cursor=({},{} inside={}) title={:?} topmost={} alpha={:?} style={:?}",
        observed.visible,
        observed.stable_geometry,
        observed.rect,
        observed.work,
        observed.activation.foreground,
        observed.activation.active,
        observed.activation.focused,
        observed.activation.captured,
        observed.cursor.x,
        observed.cursor.y,
        observed.cursor.inside_window,
        observed.title,
        observed.topmost,
        observed.alpha,
        observed.style,
    )
}

fn reveal_primary_left_and_wait(window: crate::window_control::WindowHandle) -> Result<(), String> {
    const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(1);
    let deadline = Instant::now() + START_TIMEOUT;
    let mut last_error = None;
    while Instant::now() < deadline {
        crate::window_control::reveal_primary_left_sensor(window)?;
        match wait_for_primary_left_state_with_timeout(window, false, ATTEMPT_TIMEOUT) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| "StickyMD sensor reveal produced no observation".into()))
}

fn run_window_leak_cycles(
    program_directory: &Path,
    executable: &Path,
    child: &mut Child,
    window: crate::window_control::WindowHandle,
) -> Result<(), String> {
    const ANIMATION_CYCLES: usize = 1_000;
    const TRAY_CYCLES: usize = 100;
    const CONTROL_CYCLES: usize = 100;
    reveal_primary_left_and_wait(window)?;
    let before_memory = process_metrics::memory(child)?;
    let before_objects = process_metrics::objects(child)?;
    let before_cpu = process_metrics::cpu_time(child)?;
    let cycle_started = Instant::now();
    for cycle in 0..ANIMATION_CYCLES {
        crate::window_control::park_cursor_at_primary_right(window)?;
        crate::window_control::click_toolbar(
            window,
            crate::window_control::ToolbarControl::Collapse,
        )?;
        wait_for_primary_left_state(window, true)?;
        reveal_primary_left_and_wait(window)?;
        if cycle % 250 == 249 {
            let checkpoint = process_metrics::memory(child)?;
            runtime_report!(
                "window cycle checkpoint expand_collapse={} private_bytes={}",
                cycle + 1,
                checkpoint.private_bytes
            );
        }
    }
    let cycle_elapsed = cycle_started.elapsed();
    let after_cpu = process_metrics::cpu_time(child)?;
    let animation_cpu =
        after_cpu.saturating_sub(before_cpu).as_secs_f64() / cycle_elapsed.as_secs_f64() * 100.0;
    runtime_report!(
        "window animation cycles={} elapsed_seconds={:.3} single_core_cpu_percent={animation_cpu:.3}",
        ANIMATION_CYCLES,
        cycle_elapsed.as_secs_f64()
    );

    for _ in 0..TRAY_CYCLES {
        crate::window_control::request_close(window)?;
        wait_for_window_visibility(window, false)?;
        let mut secondary = start(executable)?;
        let status = wait_for_exit(&mut secondary, EXIT_TIMEOUT)?;
        if !status.success() {
            return Err(format!("tray-cycle wake instance failed: {status}"));
        }
        wait_for_window_visibility(window, true)?;
    }

    for _ in 0..CONTROL_CYCLES {
        crate::window_control::click_toolbar(
            window,
            crate::window_control::ToolbarControl::Topmost,
        )?;
    }
    for _ in 0..102 {
        crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Theme)?;
    }
    crate::window_control::click_toolbar(window, crate::window_control::ToolbarControl::Opacity)?;
    for cycle in 0..CONTROL_CYCLES {
        crate::window_control::commit_opacity_slider(
            window,
            if cycle % 2 == 0 { 70 } else { 100 },
        )?;
    }
    wait_for_config_field(program_directory, "opacity = 100")?;
    run_persistence_and_image_leak_cycles(program_directory, child, window)?;
    thread::sleep(Duration::from_secs(2));
    ensure_alive(child, "post-cycle Phase 8 resource instance")?;
    let after_memory = process_metrics::memory(child)?;
    let after_objects = process_metrics::objects(child)?;
    runtime_report!(
        "window cycle resources before_private_bytes={} after_private_bytes={} before_objects={before_objects:?} after_objects={after_objects:?}",
        before_memory.private_bytes,
        after_memory.private_bytes
    );
    const PRIVATE_GROWTH_LIMIT: u64 = 8 * 1024 * 1024;
    if after_memory.private_bytes
        > before_memory
            .private_bytes
            .saturating_add(PRIVATE_GROWTH_LIMIT)
    {
        return Err("Phase 8 repeated shell cycles grew private bytes by more than 8 MiB".into());
    }
    if after_objects.handles > before_objects.handles.saturating_add(16)
        || after_objects.gdi_objects > before_objects.gdi_objects.saturating_add(8)
        || after_objects.user_objects > before_objects.user_objects.saturating_add(8)
    {
        return Err(format!(
            "Phase 8 repeated shell cycles leaked observable objects: before={before_objects:?} after={after_objects:?}"
        ));
    }
    Ok(())
}

fn run_persistence_and_image_leak_cycles(
    program_directory: &Path,
    child: &mut Child,
    window: crate::window_control::WindowHandle,
) -> Result<(), String> {
    const CYCLES: usize = 100;
    let note = program_directory.join("note/note.md");
    crate::window_control::switch_to_source(child.id())?;
    wait_for_config_field(program_directory, "view_mode = \"source\"")?;

    for cycle in 0..CYCLES {
        let external = format!("external reload cycle {cycle}\n").into_bytes();
        fs::write(&note, &external)
            .map_err(|error| format!("cannot simulate external reload: {error}"))?;
        wait_for_source_projection(window, &external)?;
        crate::window_control::press_enter(window)?;
        wait_for_note(&note, |bytes| {
            is_single_byte_insertion(bytes, &external, b'\n')
        })?;
        wait_for_window_title(window, |title| title == "StickyMD", "clean autosave")?;
        if cycle % 25 == 24 {
            print_cycle_checkpoint(child, "autosave_external_reload", cycle + 1)?;
        }
    }

    for cycle in 0..CYCLES {
        crate::window_control::press_enter(window)?;
        wait_for_window_title(window, |title| title == "StickyMD *", "dirty edit")?;
        let external = format!("external conflict cycle {cycle}\n");
        fs::write(&note, external.as_bytes())
            .map_err(|error| format!("cannot simulate external conflict: {error}"))?;
        wait_for_window_title(
            window,
            |title| title.contains("外部修改冲突"),
            "external conflict",
        )?;
        crate::window_control::press_f6(window)?;
        wait_for_window_title(window, |title| title == "StickyMD", "conflict resolution")?;
        if cycle % 25 == 24 {
            print_cycle_checkpoint(child, "conflict", cycle + 1)?;
        }
    }

    let image_directory = program_directory.join("note/images");
    fs::create_dir_all(&image_directory)
        .map_err(|error| format!("cannot create image-cycle directory: {error}"))?;
    for cycle in 0..CYCLES {
        let leaf = format!("leak-cycle-{cycle}.bmp");
        write_bmp(&image_directory.join(&leaf), 128, 128, cycle)?;
        let external = format!("![cycle {cycle}](images/{leaf})\n");
        fs::write(&note, external.as_bytes())
            .map_err(|error| format!("cannot simulate image source reload: {error}"))?;
        wait_for_source_projection(window, external.as_bytes())?;
        crate::window_control::switch_to_preview(window)?;
        wait_for_config_field(program_directory, "view_mode = \"preview\"")?;
        thread::sleep(Duration::from_millis(150));
        crate::window_control::switch_to_source(child.id())?;
        wait_for_config_field(program_directory, "view_mode = \"source\"")?;
        if cycle % 25 == 24 {
            print_cycle_checkpoint(child, "image_decode", cycle + 1)?;
        }
    }
    ensure_alive(child, "post persistence/image leak cycles")
}

fn wait_for_source_projection(
    window: crate::window_control::WindowHandle,
    expected: &[u8],
) -> Result<(), String> {
    let expected = std::str::from_utf8(expected)
        .map_err(|error| format!("source projection fixture is invalid UTF-8: {error}"))?;
    wait_for_shell_state(
        window,
        ShellStateExpectation::EditorInputReady,
        START_TIMEOUT,
    )?;
    crate::window_control::clear_clipboard()?;
    let deadline = Instant::now() + START_TIMEOUT;
    let mut last = None;
    while Instant::now() < deadline {
        crate::window_control::press_select_all(window)?;
        crate::window_control::press_copy(window)?;
        last = crate::window_control::clipboard_text()?;
        if last
            .as_deref()
            .is_some_and(|text| normalize_clipboard_newlines(text).as_ref() == expected)
        {
            crate::window_control::press_document_end(window)?;
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = crate::window_control::press_document_end(window);
    let shell = observe_shell(window)?;
    Err(format!(
        "source projection did not reach expected text; expected_bytes={} actual_clipboard_bytes={} shell={}",
        expected.len(),
        last.as_ref().map_or(0, String::len),
        format_shell_observation(&shell),
    ))
}

fn normalize_clipboard_newlines(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains("\r\n") {
        std::borrow::Cow::Owned(text.replace("\r\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

fn is_single_byte_insertion(observed: &[u8], original: &[u8], inserted: u8) -> bool {
    if observed.len() != original.len().saturating_add(1) {
        return false;
    }
    observed.iter().enumerate().any(|(index, byte)| {
        *byte == inserted
            && observed[..index] == original[..index]
            && observed[index + 1..] == original[index..]
    })
}

fn wait_for_note(path: &Path, accepted: impl Fn(&[u8]) -> bool) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + START_TIMEOUT;
    let mut observed = Vec::new();
    while Instant::now() < deadline {
        if let Ok(bytes) = fs::read(path) {
            observed = bytes;
            if accepted(&observed) {
                return Ok(observed);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "note did not reach the expected state; path={} bytes={}",
        path.display(),
        observed.len()
    ))
}

fn wait_for_window_title(
    window: crate::window_control::WindowHandle,
    accepted: impl Fn(&str) -> bool,
    label: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + START_TIMEOUT;
    let mut observed = String::new();
    while Instant::now() < deadline {
        observed = crate::window_control::title(window)?;
        if accepted(&observed) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "window title did not reach {label}; last title={observed:?}"
    ))
}

fn print_cycle_checkpoint(child: &mut Child, label: &str, completed: usize) -> Result<(), String> {
    let memory = process_metrics::memory(child)?;
    runtime_report!(
        "lifecycle cycle checkpoint kind={label} completed={completed} private_bytes={}",
        memory.private_bytes
    );
    Ok(())
}

fn print_duration_summary(mode: &str, samples: &mut [Duration]) -> Result<(), String> {
    if samples.len() != RESOURCE_REPETITIONS {
        return Err(format!("{mode} produced {} timing samples", samples.len()));
    }
    samples.sort_unstable();
    runtime_report!(
        "timing summary mode={mode} median_ms={:.3} max_ms={:.3}",
        samples[samples.len() / 2].as_secs_f64() * 1_000.0,
        samples[samples.len() - 1].as_secs_f64() * 1_000.0
    );
    Ok(())
}

fn wait_for_view_mode(program_directory: &Path, expected: &str) -> Result<(), String> {
    let config = program_directory.join("note/config.toml");
    let deadline = Instant::now() + START_TIMEOUT;
    let needle = format!("view_mode = \"{expected}\"");
    while Instant::now() < deadline {
        if fs::read_to_string(&config).is_ok_and(|content| content.contains(&needle)) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "StickyMD did not acknowledge view mode `{expected}` in {}",
        config.display()
    ))
}

fn prepare_resource_layout(
    program_directory: &Path,
    view_mode: &str,
    formula_count: usize,
    image_count: usize,
    image_fixture: ImageResourceFixture,
) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory)
        .map_err(|error| format!("cannot create resource note directory: {error}"))?;
    let mut fixture = String::from("# StickyMD Resource Baseline\n\n");
    for index in 0..formula_count {
        fixture.push_str(&format!(
            "Formula {index}: $x_{index}^2+y_{index}^2=1$.\n\n"
        ));
    }
    if image_count > 0 {
        write_tiny_png(&note_directory.join("images/local.png"))?;
        for index in 0..image_count {
            fixture.push_str(&format!("Image {index}:\n\n![local](images/local.png)\n\n"));
        }
    }
    if image_fixture == ImageResourceFixture::FourK {
        write_4k_bmp(&note_directory.join("images/large.bmp"))?;
        fixture.push_str("4K image:\n\n![large](images/large.bmp)\n\n");
    }
    if image_fixture == ImageResourceFixture::SaturatedCache {
        const IMAGE_COUNT: usize = 420;
        fs::create_dir_all(note_directory.join("images"))
            .map_err(|error| format!("cannot create saturated-cache fixture directory: {error}"))?;
        fixture.push_str("Cache saturation: ");
        for index in 0..IMAGE_COUNT {
            let leaf = format!("cache-{index}.bmp");
            write_bmp(&note_directory.join("images").join(&leaf), 128, 128, index)?;
            fixture.push_str(&format!("![cache-{index}](images/{leaf})"));
        }
        fixture.push_str("\n\n");
    }
    const TYPICAL_NOTE_SEED: &str =
        include_str!("../../../tests/fixtures/performance/typical-note-seed.md");
    if fixture.len() < 20 * 1024 {
        while fixture.len() < 20 * 1024 {
            fixture.push_str(TYPICAL_NOTE_SEED);
        }
        while fixture.len() > 20 * 1024 {
            fixture.pop();
        }
    }
    fs::write(note_directory.join("note.md"), fixture)
        .map_err(|error| format!("cannot seed resource note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        format!("version = 1\nview_mode = \"{view_mode}\"\n"),
    )
    .map_err(|error| format!("cannot seed resource config: {error}"))?;
    Ok(())
}

fn set_resource_zoom(program_directory: &Path, zoom: u16) -> Result<(), String> {
    let path = program_directory.join("note/config.toml");
    let mut config = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read resource config: {error}"))?;
    config.push_str(&format!("content_zoom_percent = {zoom}\n"));
    fs::write(&path, config).map_err(|error| format!("cannot seed resource zoom: {error}"))
}

fn write_4k_bmp(path: &Path) -> Result<(), String> {
    write_bmp(path, 3_840, 2_160, 0)
}

fn write_bmp(path: &Path, width: u32, height: u32, seed: usize) -> Result<(), String> {
    const HEADER_BYTES: u32 = 54;
    let pixel_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "4K BMP fixture size overflowed".to_owned())?;
    let file_bytes = HEADER_BYTES
        .checked_add(pixel_bytes)
        .ok_or_else(|| "4K BMP file size overflowed".to_owned())?;
    let parent = path
        .parent()
        .ok_or_else(|| "4K BMP fixture path has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create 4K image fixture directory: {error}"))?;
    let file =
        fs::File::create(path).map_err(|error| format!("cannot create 4K BMP fixture: {error}"))?;
    let mut output = BufWriter::new(file);
    let mut header = [0_u8; HEADER_BYTES as usize];
    header[0..2].copy_from_slice(b"BM");
    header[2..6].copy_from_slice(&file_bytes.to_le_bytes());
    header[10..14].copy_from_slice(&HEADER_BYTES.to_le_bytes());
    header[14..18].copy_from_slice(&40_u32.to_le_bytes());
    header[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    header[22..26].copy_from_slice(&(height as i32).to_le_bytes());
    header[26..28].copy_from_slice(&1_u16.to_le_bytes());
    header[28..30].copy_from_slice(&32_u16.to_le_bytes());
    header[34..38].copy_from_slice(&pixel_bytes.to_le_bytes());
    output
        .write_all(&header)
        .map_err(|error| format!("cannot write 4K BMP header: {error}"))?;
    let mut row = vec![0_u8; (width * 4) as usize];
    for (index, pixel) in row.chunks_exact_mut(4).enumerate() {
        let value = ((index + seed) % 256) as u8;
        pixel.copy_from_slice(&[value, seed as u8, 192, 255]);
    }
    for _ in 0..height {
        output
            .write_all(&row)
            .map_err(|error| format!("cannot write 4K BMP pixels: {error}"))?;
    }
    output
        .flush()
        .map_err(|error| format!("cannot flush 4K BMP fixture: {error}"))
}

fn prepare_asset_layout(
    program_directory: &Path,
    view_mode: &str,
    image_count: usize,
) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory)
        .map_err(|error| format!("cannot create asset smoke note directory: {error}"))?;
    write_tiny_png(&note_directory.join("images/local.png"))?;
    let mut fixture = String::from("# StickyMD Asset Smoke\n\n");
    for index in 0..image_count {
        fixture.push_str(&format!("![local-{index}](images/local.png)\n\n"));
    }
    fixture.push_str("![remote](https://example.invalid/no-fetch.png)\n");
    fs::write(note_directory.join("note.md"), fixture)
        .map_err(|error| format!("cannot seed asset smoke note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        format!("version = 1\nview_mode = \"{view_mode}\"\n"),
    )
    .map_err(|error| format!("cannot seed asset smoke config: {error}"))
}

fn write_tiny_png(path: &Path) -> Result<(), String> {
    const PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 4,
        0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15, 0, 1, 5,
        1, 1, 39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    let parent = path
        .parent()
        .ok_or_else(|| "asset fixture path has no parent".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create asset fixture directory: {error}"))?;
    fs::write(path, PNG).map_err(|error| format!("cannot seed tiny PNG: {error}"))
}

fn assert_asset_source_unchanged(program_directory: &Path) -> Result<(), String> {
    let note = program_directory.join("note/note.md");
    let content = fs::read_to_string(&note)
        .map_err(|error| format!("cannot inspect asset smoke source: {error}"))?;
    if !content.contains("images/local.png") || !content.contains("https://example.invalid") {
        return Err("asset Preview runtime changed canonical Markdown source".to_owned());
    }
    if !program_directory.join("note/images/local.png").is_file() {
        return Err("asset Preview runtime removed user-supplied local image".to_owned());
    }
    Ok(())
}

fn print_resource_summary(
    mode: &str,
    samples: &[MemorySample],
    cpu_samples: &[f64],
) -> Result<(), String> {
    if samples.len() != RESOURCE_REPETITIONS {
        return Err(format!("{mode} produced {} samples", samples.len()));
    }
    let mut private_working_set: Vec<_> = samples
        .iter()
        .map(|sample| sample.private_working_set_bytes)
        .collect();
    let mut private_bytes: Vec<_> = samples.iter().map(|sample| sample.private_bytes).collect();
    let mut peak_working_set: Vec<_> = samples
        .iter()
        .map(|sample| sample.peak_working_set_bytes)
        .collect();
    let mut peak_private_bytes: Vec<_> = samples
        .iter()
        .map(|sample| sample.peak_private_bytes)
        .collect();
    private_working_set.sort_unstable();
    private_bytes.sort_unstable();
    peak_working_set.sort_unstable();
    peak_private_bytes.sort_unstable();
    let mut cpu_sorted = cpu_samples.to_vec();
    cpu_sorted.sort_by(f64::total_cmp);
    if !cpu_sorted.is_empty() && cpu_sorted.len() != RESOURCE_REPETITIONS {
        return Err(format!(
            "{mode} produced {} idle CPU samples",
            cpu_sorted.len()
        ));
    }
    let middle = samples.len() / 2;
    let cpu_summary = if cpu_sorted.is_empty() {
        "samples=0 median=not-measured p95=not-measured max=not-measured".to_owned()
    } else {
        format!(
            "samples={} median={:.6} p95={:.6} max={:.6}",
            cpu_sorted.len(),
            cpu_sorted[middle],
            cpu_sorted[nearest_rank_index(cpu_sorted.len(), 95)],
            cpu_sorted[cpu_sorted.len() - 1],
        )
    };
    runtime_report!(
        "resource summary mode={mode} private_working_set_median_bytes={} private_working_set_max_bytes={} \
         private_bytes_median={} private_bytes_max={} peak_working_set_median_bytes={} \
         peak_working_set_max_bytes={} peak_private_bytes_median={} peak_private_bytes_max={} \
         idle_cpu_percent={cpu_summary}",
        private_working_set[middle],
        private_working_set[samples.len() - 1],
        private_bytes[middle],
        private_bytes[samples.len() - 1],
        peak_working_set[middle],
        peak_working_set[samples.len() - 1],
        peak_private_bytes[middle],
        peak_private_bytes[samples.len() - 1],
    );
    Ok(())
}

fn nearest_rank_index(sample_count: usize, percentile: usize) -> usize {
    sample_count
        .saturating_mul(percentile)
        .div_ceil(100)
        .saturating_sub(1)
}

fn prepare_preview_layout(program_directory: &Path, view_mode: &str) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory).map_err(|error| {
        format!(
            "cannot create preview smoke note directory {}: {error}",
            note_directory.display()
        )
    })?;
    let fixture = concat!(
        "# StickyMD Preview Smoke\n\n",
        "中文 **粗体** and *italic* with [safe link](https://example.com).\n\n",
        "> quote\n\n- [x] task\n\n",
        "| left | right |\n| :--- | ---: |\n| A | B |\n\n",
        "`inline` and $x^2$\n\n",
        "![remote placeholder](https://example.invalid/no-fetch.png)\n\n",
        "<script>throw new Error('must remain literal')</script>\n\n",
        "<iframe src=\"https://example.invalid/must-not-load\"></iframe>\n"
    );
    fs::write(note_directory.join("note.md"), fixture)
        .map_err(|error| format!("cannot seed preview smoke note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        format!("version = 1\nview_mode = \"{view_mode}\"\n"),
    )
    .map_err(|error| format!("cannot seed preview smoke config: {error}"))?;
    Ok(())
}

const PHASE11B_SOURCE: &str = concat!(
    "# Phase 11-B\n\n",
    "Inline: \\(x^2+中\\)\n\n",
    "Display: \\[\\frac{a}{b}\\]\n\n",
    "Literal: `\\(example\\)` and $already$.\n",
);

const PHASE11B_CONVERTED: &str = concat!(
    "# Phase 11-B\n\n",
    "Inline: $x^2+中$\n\n",
    "Display: $$\\frac{a}{b}$$\n\n",
    "Literal: `\\(example\\)` and $already$.\n",
);

fn prepare_phase11b_layout(program_directory: &Path) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory)
        .map_err(|error| format!("cannot create Phase 11-B smoke note directory: {error}"))?;
    fs::write(note_directory.join("note.md"), PHASE11B_SOURCE)
        .map_err(|error| format!("cannot seed Phase 11-B smoke note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        "version = 1\nview_mode = \"source\"\n",
    )
    .map_err(|error| format!("cannot seed Phase 11-B smoke config: {error}"))
}

const MATH_RUNTIME_FIXTURE: &str = concat!(
    "# 数学测试\n\n",
    "这是一个行内公式 $x^2+y^2=1$ and this is English.\n\n",
    "Euler: $e^{i\\pi}+1=0$\n\n",
    "\\[\\int_0^1 x^2\\,dx=\\frac13\\]\n\n",
    "\\[A=\\begin{pmatrix}a&b\\\\c&d\\end{pmatrix}\\]\n\n",
    "\\[f(x)=\\begin{cases}x^2,&x\\ge0\\\\-x,&x<0\\end{cases}\\]\n\n",
    "坏公式：\\[\\frac{\\]\n",
);

fn prepare_math_layout(program_directory: &Path, view_mode: &str) -> Result<(), String> {
    let note_directory = program_directory.join("note");
    fs::create_dir(&note_directory)
        .map_err(|error| format!("cannot create math smoke note directory: {error}"))?;
    fs::write(note_directory.join("note.md"), MATH_RUNTIME_FIXTURE)
        .map_err(|error| format!("cannot seed math smoke note: {error}"))?;
    fs::write(
        note_directory.join("config.toml"),
        format!("version = 1\nview_mode = \"{view_mode}\"\n"),
    )
    .map_err(|error| format!("cannot seed math smoke config: {error}"))?;
    Ok(())
}

fn assert_math_source_unchanged(program_directory: &Path) -> Result<(), String> {
    let actual = fs::read_to_string(program_directory.join("note/note.md"))
        .map_err(|error| format!("cannot read math smoke note: {error}"))?;
    if actual == MATH_RUNTIME_FIXTURE {
        Ok(())
    } else {
        Err("native math preview changed canonical Markdown source".to_owned())
    }
}

fn create_smoke_root() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stickymd-smoke-{}-{nonce}", std::process::id()));
    fs::create_dir(&root)
        .map_err(|error| format!("cannot create smoke root {}: {error}", root.display()))?;
    Ok(root)
}

fn copy_executable(source: &Path, directory: &Path) -> Result<PathBuf, String> {
    fs::create_dir(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let destination = directory.join("StickyMD.exe");
    fs::copy(source, &destination).map_err(|error| {
        format!(
            "cannot copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
}

fn start(executable: &Path) -> Result<Child, String> {
    Command::new(executable)
        .current_dir(
            executable
                .parent()
                .ok_or_else(|| format!("{} has no parent", executable.display()))?,
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot start {}: {error}", executable.display()))
}

fn wait_for_layout(program_directory: &Path) -> Result<(), String> {
    let note = program_directory.join("note/note.md");
    let config = program_directory.join("note/config.toml");
    let deadline = Instant::now() + START_TIMEOUT;
    while Instant::now() < deadline {
        if note.is_file() && config.is_file() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(format!(
        "portable layout was not created within {:?}: {}",
        START_TIMEOUT,
        program_directory.display()
    ))
}

fn ensure_alive(child: &mut Child, label: &str) -> Result<(), String> {
    match child
        .try_wait()
        .map_err(|error| format!("cannot inspect {label}: {error}"))?
    {
        None => Ok(()),
        Some(status) => Err(format!("{label} exited early with {status}")),
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("cannot inspect secondary process: {error}"))?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("secondary process did not exit within {timeout:?}"));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn file_state(path: &Path) -> Result<(Vec<u8>, SystemTime), String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    Ok((bytes, modified))
}

fn stop_children(children: &mut [Child]) {
    for child in children {
        match child.try_wait() {
            Ok(Some(_)) => {}
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn stop_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) | Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn cleanup_root(root: &Path) -> Result<(), String> {
    let temporary = std::env::temp_dir();
    let safe_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("stickymd-smoke-"));
    if !root.starts_with(&temporary) || !safe_name {
        return Err(format!(
            "refusing to remove unverified smoke directory {}",
            root.display()
        ));
    }
    let retry_delays = [0, 50, 100, 200, 400, 800];
    let mut last_error = None;
    for delay_ms in retry_delays {
        if delay_ms != 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }
        match fs::remove_dir_all(root) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(format!(
        "cannot remove smoke directory {} after bounded retries: {}",
        root.display(),
        last_error.map_or_else(|| "unknown error".to_owned(), |error| error.to_string())
    ))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        StartupThresholdClass, cpu_measurements, duration_measurements, is_single_byte_insertion,
        nearest_rank_index, normalize_clipboard_newlines, startup_threshold_class,
    };

    #[test]
    fn five_sample_p95_is_the_observed_maximum() {
        assert_eq!(nearest_rank_index(5, 50), 2);
        assert_eq!(nearest_rank_index(5, 95), 4);
    }

    #[test]
    fn autosave_receipt_accepts_exactly_one_requested_byte() {
        assert!(is_single_byte_insertion(b"ab\ncd", b"abcd", b'\n'));
        assert!(is_single_byte_insertion(b"abcd\n", b"abcd", b'\n'));
        assert!(!is_single_byte_insertion(b"abcd", b"abcd", b'\n'));
        assert!(!is_single_byte_insertion(b"abycd", b"abcd", b'\n'));
    }

    #[test]
    fn source_projection_probe_normalizes_only_windows_newlines() {
        assert_eq!(normalize_clipboard_newlines("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize_clipboard_newlines("a\rb\n"), "a\rb\n");
    }

    #[test]
    fn resource_summaries_project_structured_machine_measurements() {
        let cpu = cpu_measurements("source", &[0.01, 0.03, 0.02, 0.04, 0.05]);
        assert_eq!(cpu.len(), 3);
        assert_eq!(cpu[0].name, "source.idle_cpu_median");
        assert_eq!(cpu[0].value, 0.03);
        assert_eq!(cpu[1].value, 0.05);
        assert_eq!(cpu[2].value, 0.05);

        let duration = duration_measurements(
            "window.startup",
            &[
                Duration::from_millis(1),
                Duration::from_millis(3),
                Duration::from_millis(2),
            ],
        );
        assert_eq!(duration.len(), 2);
        assert_eq!(duration[0].value, 2.0);
        assert_eq!(duration[1].value, 3.0);
    }

    #[test]
    fn startup_thresholds_keep_targets_diagnostic_and_550_ms_release_blocking() {
        assert_eq!(
            startup_threshold_class(Duration::from_millis(180)),
            StartupThresholdClass::Preferred
        );
        assert_eq!(
            startup_threshold_class(Duration::from_millis(400)),
            StartupThresholdClass::Engineering
        );
        assert_eq!(
            startup_threshold_class(Duration::from_millis(550)),
            StartupThresholdClass::ReleaseOnly
        );
        assert_eq!(
            startup_threshold_class(Duration::from_millis(551)),
            StartupThresholdClass::Failed
        );
    }
}
