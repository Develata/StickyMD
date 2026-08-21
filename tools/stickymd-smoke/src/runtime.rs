//! Opt-in Windows runtime smoke using copied Release executables.

use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::process_metrics::{self, MemorySample};
use crate::runner::RuntimeScenario;

const START_TIMEOUT: Duration = Duration::from_secs(10);
const EXIT_TIMEOUT: Duration = Duration::from_secs(5);
const RESOURCE_WARMUP: Duration = Duration::from_secs(30);
const CPU_INTERVAL: Duration = Duration::from_secs(60);
const RESOURCE_REPETITIONS: usize = 5;
const HIDDEN_PRIVATE_WORKING_SET_LIMIT: u64 = 36 * 1024 * 1024;
const IDLE_CPU_PERCENT_LIMIT: f64 = 0.1;

pub(crate) fn run(repository: &Path, scenario: RuntimeScenario) -> Result<(), String> {
    let root = create_smoke_root()?;
    let mut children = Vec::new();
    let result = run_inner(repository, &root, scenario, &mut children);
    stop_children(&mut children);
    let cleanup = cleanup_root(&root);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn run_inner(
    repository: &Path,
    root: &Path,
    scenario: RuntimeScenario,
    children: &mut Vec<Child>,
) -> Result<(), String> {
    if scenario == RuntimeScenario::Resources {
        return run_resource_measurement(repository, root, false, false);
    }
    if scenario == RuntimeScenario::MathResources {
        return run_resource_measurement(repository, root, true, false);
    }
    if scenario == RuntimeScenario::ImageResources {
        return run_resource_measurement(repository, root, false, true);
    }
    if scenario == RuntimeScenario::WindowResources {
        return run_window_resource_measurement(repository, root);
    }
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
    }
    children.push(start(&first_exe)?);
    wait_for_layout(&first_dir)?;
    ensure_alive(&mut children[0], "first portable instance")?;

    if scenario == RuntimeScenario::WindowShell {
        return run_window_shell_lifecycle(&first_dir, &first_exe, &mut children[0]);
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

fn run_window_shell_lifecycle(
    program_directory: &Path,
    executable: &Path,
    primary: &mut Child,
) -> Result<(), String> {
    let window = crate::window_control::visible_window(primary.id())?;
    println!(
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
) -> Result<(), String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let logical_processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    println!(
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
        println!("resource development filter: {filter}");
    }
    for case in cases {
        let mode = case.label;
        let mut memory_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
        let mut cpu_percent = None;
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
            thread::sleep(RESOURCE_WARMUP);
            ensure_alive(&mut child, "resource measurement instance")?;
            if mode == "source-after-preview-cache-release" {
                crate::window_control::switch_to_source(child.id())?;
                wait_for_view_mode(&directory, "source")?;
                thread::sleep(Duration::from_secs(5));
                ensure_alive(&mut child, "Source-after-Preview resource instance")?;
            }
            let sample = process_metrics::memory(&child)?;
            println!(
                "resource sample mode={mode} run={} private_working_set_bytes={} private_bytes={} \
                 peak_working_set_bytes={} peak_private_bytes={}",
                repetition + 1,
                sample.private_working_set_bytes,
                sample.private_bytes,
                sample.peak_working_set_bytes,
                sample.peak_private_bytes,
            );
            memory_samples.push(sample);
            if repetition == 0 && case.measure_cpu {
                let before = process_metrics::cpu_time(&child)?;
                let wall_started = Instant::now();
                thread::sleep(CPU_INTERVAL);
                ensure_alive(&mut child, "idle CPU measurement instance")?;
                let elapsed = wall_started.elapsed();
                let after = process_metrics::cpu_time(&child)?;
                let cpu = after.saturating_sub(before).as_secs_f64()
                    / elapsed.as_secs_f64()
                    / logical_processors as f64
                    * 100.0;
                println!(
                    "resource cpu mode={mode} interval_seconds={:.3} average_percent={cpu:.6}",
                    elapsed.as_secs_f64()
                );
                cpu_percent = Some(cpu);
            }
            stop_child(&mut child);
        }
        print_resource_summary(mode, &memory_samples, cpu_percent)?;
    }
    Ok(())
}

fn run_window_resource_measurement(repository: &Path, root: &Path) -> Result<(), String> {
    let source = repository.join("target/release/stickymd-win.exe");
    if !source.is_file() {
        return Err(format!(
            "Release executable is missing: {}",
            source.display()
        ));
    }
    let logical_processors = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    println!(
        "Phase 8 window resource contract: warmup={}s repetitions={} cpu_interval={}s logical_processors={logical_processors}",
        RESOURCE_WARMUP.as_secs(),
        RESOURCE_REPETITIONS,
        CPU_INTERVAL.as_secs(),
    );
    let mut visible_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut collapsed_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut hidden_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut startup_samples = Vec::with_capacity(RESOURCE_REPETITIONS);
    let mut visible_cpu = None;
    let mut collapsed_cpu = None;
    let mut hidden_cpu = None;
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
            println!(
                "window startup run={} elapsed_ms={:.3}",
                repetition + 1,
                startup.as_secs_f64() * 1_000.0
            );
            thread::sleep(RESOURCE_WARMUP);
            ensure_alive(&mut child, "visible window resource instance")?;
            let visible = process_metrics::memory(&child)?;
            println!(
                "resource sample mode=visible-source run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                repetition + 1,
                visible.private_working_set_bytes,
                visible.private_bytes,
                visible.peak_working_set_bytes,
                visible.peak_private_bytes,
            );
            if repetition == 0 {
                visible_cpu = Some(measure_idle_cpu(
                    &mut child,
                    "visible-source",
                    logical_processors,
                )?);
            }
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
            println!(
                "resource sample mode=docked-collapsed run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                repetition + 1,
                collapsed.private_working_set_bytes,
                collapsed.private_bytes,
                collapsed.peak_working_set_bytes,
                collapsed.peak_private_bytes,
            );
            if repetition == 0 {
                collapsed_cpu = Some(measure_idle_cpu(
                    &mut child,
                    "docked-collapsed",
                    logical_processors,
                )?);
                run_window_leak_cycles(&directory, &executable, &mut child, window)?;
            }
            crate::window_control::request_close(window)?;
            wait_for_window_visibility(window, false)?;
            thread::sleep(RESOURCE_WARMUP);
            ensure_alive(&mut child, "hidden-to-tray resource instance")?;
            let hidden = process_metrics::memory(&child)?;
            println!(
                "resource sample mode=hidden-to-tray run={} private_working_set_bytes={} private_bytes={} peak_working_set_bytes={} peak_private_bytes={}",
                repetition + 1,
                hidden.private_working_set_bytes,
                hidden.private_bytes,
                hidden.peak_working_set_bytes,
                hidden.peak_private_bytes,
            );
            if repetition == 0 {
                hidden_cpu = Some(measure_idle_cpu(
                    &mut child,
                    "hidden-to-tray",
                    logical_processors,
                )?);
            }
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
    print_resource_summary("visible-source", &visible_samples, visible_cpu)?;
    print_resource_summary("docked-collapsed", &collapsed_samples, collapsed_cpu)?;
    print_resource_summary("hidden-to-tray", &hidden_samples, hidden_cpu)?;
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
    for (mode, cpu) in [
        ("visible-source", visible_cpu),
        ("docked-collapsed", collapsed_cpu),
        ("hidden-to-tray", hidden_cpu),
    ] {
        if cpu.is_some_and(|value| value > IDLE_CPU_PERCENT_LIMIT) {
            return Err(format!(
                "{mode} idle CPU {:.6}% exceeds {:.3}%",
                cpu.unwrap_or_default(),
                IDLE_CPU_PERCENT_LIMIT
            ));
        }
    }
    Ok(())
}

fn measure_idle_cpu(
    child: &mut Child,
    mode: &str,
    logical_processors: usize,
) -> Result<f64, String> {
    let before = process_metrics::cpu_time(child)?;
    let wall_started = Instant::now();
    thread::sleep(CPU_INTERVAL);
    ensure_alive(child, &format!("{mode} idle CPU instance"))?;
    let elapsed = wall_started.elapsed();
    let after = process_metrics::cpu_time(child)?;
    let cpu = after.saturating_sub(before).as_secs_f64()
        / elapsed.as_secs_f64()
        / logical_processors as f64
        * 100.0;
    println!(
        "resource cpu mode={mode} interval_seconds={:.3} average_percent={cpu:.6}",
        elapsed.as_secs_f64()
    );
    Ok(cpu)
}

fn wait_for_primary_left_state(
    window: crate::window_control::WindowHandle,
    collapsed: bool,
) -> Result<(), String> {
    wait_for_primary_left_state_with_timeout(window, collapsed, START_TIMEOUT)
}

fn wait_for_primary_left_state_with_timeout(
    window: crate::window_control::WindowHandle,
    collapsed: bool,
    timeout: Duration,
) -> Result<(), String> {
    let work = crate::window_control::primary_work_area()?;
    let deadline = Instant::now() + timeout;
    let mut last_rect = None;
    while Instant::now() < deadline {
        let rect = crate::window_control::window_rect(window)?;
        last_rect = Some(rect);
        let sensor_right = i64::from(rect.x) + i64::from(rect.width) - i64::from(work.x);
        let observed_collapsed = rect.x < work.x && (1..=16).contains(&sensor_right);
        let observed_expanded = (rect.x - work.x).abs() <= 1;
        if (collapsed && observed_collapsed) || (!collapsed && observed_expanded) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(format!(
        "StickyMD did not become primary-left {} within {:.3} seconds; work={work:?} last_rect={last_rect:?}",
        if collapsed { "collapsed" } else { "expanded" },
        timeout.as_secs_f64()
    ))
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
            println!(
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
    println!(
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
    thread::sleep(Duration::from_secs(2));
    ensure_alive(child, "post-cycle Phase 8 resource instance")?;
    let after_memory = process_metrics::memory(child)?;
    let after_objects = process_metrics::objects(child)?;
    println!(
        "window cycle resources before_private_bytes={} after_private_bytes={} before_objects={before_objects:?} after_objects={after_objects:?}",
        before_memory.private_bytes, after_memory.private_bytes
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

fn print_duration_summary(mode: &str, samples: &mut [Duration]) -> Result<(), String> {
    if samples.len() != RESOURCE_REPETITIONS {
        return Err(format!("{mode} produced {} timing samples", samples.len()));
    }
    samples.sort_unstable();
    println!(
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
    const PLAIN: &str = "中文 baseline text with Latin words and stable native preview layout.\n\n";
    while fixture.len() < 20 * 1024 {
        fixture.push_str(PLAIN);
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
    cpu_percent: Option<f64>,
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
    let middle = samples.len() / 2;
    println!(
        "resource summary mode={mode} private_working_set_median_bytes={} private_working_set_max_bytes={} \
         private_bytes_median={} private_bytes_max={} peak_working_set_median_bytes={} \
         peak_working_set_max_bytes={} peak_private_bytes_median={} peak_private_bytes_max={} \
         idle_cpu_average_percent={}",
        private_working_set[middle],
        private_working_set[samples.len() - 1],
        private_bytes[middle],
        private_bytes[samples.len() - 1],
        peak_working_set[middle],
        peak_working_set[samples.len() - 1],
        peak_private_bytes[middle],
        peak_private_bytes[samples.len() - 1],
        cpu_percent.map_or_else(|| "not-measured".to_owned(), |value| format!("{value:.6}"))
    );
    Ok(())
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
