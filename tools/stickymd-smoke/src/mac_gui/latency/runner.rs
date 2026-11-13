//! The VM run: isolated sessions, interleaved apps, a deterministic flush and the checks that
//! make a run count.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    Options,
    model::{self, Sample},
};
use crate::mac_common::{
    fixture::{bundle_hash, contained_executable, copy_fixture},
    session::Session,
};
use crate::mac_gui::{
    input::{CMD, Injection},
    model::{quote, require_vm},
    native::{accessibility as ax, events, events::Events, permissions, windows},
};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The product's opt-in recorder (apps/stickymd-mac/src/platform/macos/input_latency.rs).
const ENV: &str = "STICKYMD_DIAGNOSTIC_INPUT_LATENCY";
const CONFIG: &str = "version = 1\nview_mode = \"source\"\n";
const DOWN_ARROW: u16 = 125;
/// After the last key, before the flush: lets the last frames commit.
const SETTLE_MS: u64 = 1500;

#[derive(Clone, Copy, Debug)]
enum Pattern {
    Sustained,
    AfterIdle,
}

#[derive(Clone, Copy, Debug)]
struct Case {
    name: &'static str,
    bytes: usize,
    pattern: Pattern,
}

const CASES: [Case; 4] = [
    Case {
        name: "100KiB-sustained",
        bytes: 100 * 1024,
        pattern: Pattern::Sustained,
    },
    Case {
        name: "100KiB-after-idle",
        bytes: 100 * 1024,
        pattern: Pattern::AfterIdle,
    },
    Case {
        name: "1MiB-sustained",
        bytes: 1024 * 1024,
        pattern: Pattern::Sustained,
    },
    Case {
        name: "1MiB-after-idle",
        bytes: 1024 * 1024,
        pattern: Pattern::AfterIdle,
    },
];

struct App {
    path: PathBuf,
    executable: String,
    bundle: String,
}

struct Run {
    app: usize,
    case: Case,
    repetition: usize,
    reason: Option<String>,
    injected: usize,
    recorded: usize,
    added: Option<usize>,
    clock_offset_ns: Option<i64>,
    dropped: (u64, u64),
    uncommitted: usize,
    samples: Vec<Sample>,
}

use crate::mac_common::uptime_ns as now_ns;

/// A deterministic Source fixture of exactly `bytes` bytes, mixed CJK and Latin prose, ending in
/// a newline. It contains no `x`, so the typed suffix is unambiguous.
fn fixture(bytes: usize) -> String {
    const PARAGRAPH: &str = "中文草稿与 Latin prose for the latency sample: deterministic, plain, long enough to wrap twice in the default window.\n\n";
    let mut note = String::from("# Latency sample / 延迟测量\n\n");
    while note.len() + PARAGRAPH.len() < bytes {
        note.push_str(PARAGRAPH);
    }
    while note.len() + 1 < bytes {
        note.push('.');
    }
    note.push('\n');
    debug_assert!(!note.contains('x'));
    note
}

pub(super) fn execute(options: &Options) -> Result<(), String> {
    let vm = Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.hv_vmm_present"])
        .output()
        .map_err(|e| format!("VM detection failed (refused): {e}"))?;
    require_vm(&String::from_utf8_lossy(&vm.stdout), options.allow_host)?;
    if !cfg!(panic = "unwind") {
        return Err("mac-latency refuses panic=abort builds: input release requires unwinding; build with CARGO_PROFILE_RELEASE_PANIC=unwind".into());
    }
    let granted = permissions::check();
    if !granted.ax || !granted.post {
        return Err("mac-latency needs Accessibility and PostEvent for this binary".into());
    }
    let mut apps = Vec::new();
    for path in &options.apps {
        let path = path.canonicalize().map_err(|e| format!("app path: {e}"))?;
        if path.extension().is_none_or(|e| e != "app") || !path.is_dir() {
            return Err("--app must name a .app directory".into());
        }
        apps.push(App {
            executable: crate::integrity::sha256(&contained_executable(&path)?)?,
            bundle: bundle_hash(&path)?,
            path,
        });
    }
    let driver = crate::integrity::sha256(
        &std::env::current_exe().map_err(|e| format!("driver path: {e}"))?,
    )?;
    fs::create_dir_all(&options.out).map_err(|e| e.to_string())?;
    let output = options.out.join("mac-latency.json");
    let sw_vers = Command::new("/usr/bin/sw_vers")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    events::install_cancel()?;
    let mut input = Injection::new(Events::new());
    let mut runs = Vec::new();
    for repetition in 1..=options.repetitions {
        for (index, app) in apps.iter().enumerate() {
            for case in CASES {
                println!(
                    "rep {repetition} app {} {}: starting",
                    &app.executable[..8],
                    case.name
                );
                let run = one_run(app, index, case, repetition, options, &mut input);
                println!(
                    "rep {repetition} app {} {}: {}",
                    &app.executable[..8],
                    case.name,
                    run.reason.as_deref().unwrap_or("VALID")
                );
                runs.push(run);
                let json = evidence(options, &apps, &driver, &sw_vers, &runs);
                crate::atomic_evidence::write(&output, json.as_bytes())?;
                if input.stopped.is_some() {
                    return Err(format!("input stopped: {:?}", input.stopped));
                }
            }
        }
    }
    summary(&apps, &runs);
    println!("Evidence: {}", output.display());
    if runs.iter().all(|run| run.reason.is_none()) {
        Ok(())
    } else {
        Err("some runs are invalid; see mac-latency.json".into())
    }
}

fn one_run(
    app: &App,
    index: usize,
    case: Case,
    repetition: usize,
    options: &Options,
    input: &mut Injection<Events>,
) -> Run {
    let mut run = Run {
        app: index,
        case,
        repetition,
        reason: None,
        injected: 0,
        recorded: 0,
        added: None,
        clock_offset_ns: None,
        dropped: (0, 0),
        uncommitted: 0,
        samples: Vec::new(),
    };
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(error) => {
            run.reason = Some(error);
            return run;
        }
    };
    let result = measure(&mut session, app, case, options, input, &mut run);
    let release = input.release();
    let cleanup = session.cleanup();
    let errors = [result, release, cleanup]
        .into_iter()
        .filter_map(Result::err)
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        run.reason = Some(errors.join("; "));
    }
    run
}

fn measure(
    session: &mut Session,
    app: &App,
    case: Case,
    options: &Options,
    input: &mut Injection<Events>,
    run: &mut Run,
) -> Result<(), String> {
    let note = fixture(case.bytes);
    let executable = copy_fixture(session, &app.path, &note, CONFIG, &app.executable)?;
    let recording = session.path().join("latency.txt");
    session.spawn(
        Command::new(&executable)
            .current_dir(session.path())
            .env(ENV, &recording)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )?;
    let pid = session.child.as_ref().ok_or("missing child")?.id();
    let started = Instant::now();
    loop {
        if let Some(status) = session.poll()? {
            return Err(format!("product exited before ready: {status}"));
        }
        if windows::window(pid, None).is_ok_and(|w| w.onscreen) {
            break;
        }
        if started.elapsed() > Duration::from_secs(30) {
            return Err("no on-screen product window within 30 s".into());
        }
        events::wait(100)?;
    }
    events::wait(1000)?;
    ax::activate(pid)?;
    events::wait(300)?;
    if ax::front()? != pid {
        return Err("product did not acquire focus".into());
    }
    input.chord(&[CMD, DOWN_ARROW])?;
    events::wait(500)?;

    let from = now_ns();
    let mut injected = Vec::new();
    match case.pattern {
        Pattern::Sustained => {
            let interval = Duration::from_millis(options.interval_ms);
            let begin = Instant::now();
            for key in 0..options.keys {
                let due = begin + interval * key as u32;
                if let Some(rest) = due.checked_duration_since(Instant::now()) {
                    std::thread::sleep(rest);
                }
                injected.push(now_ns());
                input.text("x")?;
            }
        }
        Pattern::AfterIdle => {
            for _ in 0..options.idle_keys {
                events::wait(options.idle_ms)?;
                injected.push(now_ns());
                input.text("x")?;
            }
        }
    }
    events::wait(SETTLE_MS)?;
    let to = now_ns();
    run.injected = injected.len();
    if ax::front()? != pid {
        return Err("focus left the product while typing".into());
    }

    session.signal_flush()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !recording.exists() {
        if Instant::now() >= deadline {
            return Err("the product wrote no latency recording within 5 s".into());
        }
        events::wait(50)?;
    }
    let text = fs::read_to_string(&recording).map_err(|e| e.to_string())?;
    let parsed = model::parse(&text)?;
    run.dropped = (parsed.dropped_keys, parsed.dropped_frames);
    let window_events = parsed
        .keys
        .iter()
        .filter(|key| (from..=to).contains(&key.event))
        .map(|key| key.event)
        .collect::<Vec<_>>();
    run.recorded = window_events.len();
    run.clock_offset_ns = model::clock_offset(&window_events, &injected);
    let paired = model::pair(&parsed, from, to);
    run.uncommitted = paired.uncommitted;
    run.samples = paired.samples;

    // Quit through the product's own SIGTERM path, which saves the note, then read it back.
    session.signal_terminate()?;
    let status = session
        .wait_until(Instant::now() + Duration::from_secs(15))?
        .ok_or("the product did not exit within 15 s of SIGTERM")?;
    let saved =
        fs::read_to_string(session.path().join("note/note.md")).map_err(|e| e.to_string())?;
    run.added = saved
        .strip_prefix(note.as_str())
        .filter(|rest| rest.bytes().all(|b| b == b'x'))
        .map(str::len);

    let mut problems = Vec::new();
    if !status.success() {
        problems.push(format!("exit status {status}"));
    }
    if run.recorded != run.injected {
        problems.push(format!(
            "recorded {} keys, injected {}",
            run.recorded, run.injected
        ));
    }
    if run.added != Some(run.injected) {
        problems.push(format!(
            "note gained {:?} characters, injected {}",
            run.added, run.injected
        ));
    }
    if paired.unpaired != 0 {
        problems.push(format!("{} keys without a frame", paired.unpaired));
    }
    if run.dropped != (0, 0) {
        problems.push(format!("recorder dropped {:?}", run.dropped));
    }
    match run.clock_offset_ns {
        Some(offset) if offset.abs() <= 100_000_000 => {}
        other => problems.push(format!("event clock offset {other:?} ns")),
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

fn evidence(options: &Options, apps: &[App], driver: &str, sw_vers: &str, runs: &[Run]) -> String {
    let apps_json = apps
        .iter()
        .map(|app| {
            format!(
                "{{\"path\":{},\"executable_sha256\":{},\"bundle_manifest_sha256\":{}}}",
                quote(&app.path.display().to_string()),
                quote(&app.executable),
                quote(&app.bundle)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let runs_json = runs
        .iter()
        .map(|run| {
            format!(
                "{{\"app\":{},\"case\":{},\"repetition\":{},\"status\":{},\"reason\":{},\"injected\":{},\"recorded\":{},\"added\":{},\"clock_offset_ns\":{},\"dropped\":[{},{}],\"uncommitted\":{},\"stages\":{}}}",
                run.app,
                quote(run.case.name),
                run.repetition,
                quote(if run.reason.is_none() { "VALID" } else { "INVALID" }),
                run.reason.as_deref().map_or("null".into(), quote),
                run.injected,
                run.recorded,
                run.added.map_or("null".into(), |n| n.to_string()),
                run.clock_offset_ns.map_or("null".into(), |n| n.to_string()),
                run.dropped.0,
                run.dropped.1,
                run.uncommitted,
                model::stages_json(&run.samples)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let pooled = (0..apps.len())
        .flat_map(|app| CASES.iter().map(move |case| (app, case)))
        .map(|(app, case)| {
            let samples = pooled(runs, app, case.name);
            format!(
                "{{\"app\":{app},\"case\":{},\"valid_runs\":{},\"stages\":{}}}",
                quote(case.name),
                valid_runs(runs, app, case.name),
                model::stages_json(&samples)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"diagnostic\",\"gate\":\"none; plan 10's edit benchmark is the input-latency gate\",\"system\":{},\"driver_sha256\":{},\"options\":{{\"repetitions\":{},\"keys\":{},\"interval_ms\":{},\"idle_keys\":{},\"idle_ms\":{}}},\"stages\":\"queue = event to main thread; wait = main thread to the start of drawing its frame; draw = drawing through present returned; commit = present to Core Animation commit; total = event to commit\",\"apps\":[{apps_json}],\"pooled\":[{pooled}],\"runs\":[{runs_json}]}}\n",
        quote(sw_vers.trim()),
        quote(driver),
        options.repetitions,
        options.keys,
        options.interval_ms,
        options.idle_keys,
        options.idle_ms,
    )
}

fn pooled(runs: &[Run], app: usize, case: &str) -> Vec<Sample> {
    runs.iter()
        .filter(|run| run.app == app && run.case.name == case && run.reason.is_none())
        .flat_map(|run| run.samples.iter().copied())
        .collect()
}

fn valid_runs(runs: &[Run], app: usize, case: &str) -> usize {
    runs.iter()
        .filter(|run| run.app == app && run.case.name == case && run.reason.is_none())
        .count()
}

fn summary(apps: &[App], runs: &[Run]) {
    for (index, app) in apps.iter().enumerate() {
        for case in CASES {
            let samples = pooled(runs, index, case.name);
            let line = |name: &str, values: Vec<u64>| match model::stats(values) {
                Some(s) => format!(
                    "{name} p50/p95/max {:.2}/{:.2}/{:.2} ms",
                    s.p50, s.p95, s.max
                ),
                None => format!("{name} -"),
            };
            println!(
                "{} {} ({} valid runs, {} keys): {}; {}; {}; {}",
                &app.executable[..8],
                case.name,
                valid_runs(runs, index, case.name),
                samples.len(),
                line("total", samples.iter().map(|s| s.total).collect()),
                line("wait", samples.iter().map(|s| s.wait).collect()),
                line("draw", samples.iter().map(|s| s.draw).collect()),
                line("commit", samples.iter().filter_map(|s| s.commit).collect()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixtures_have_the_exact_size_and_no_x() {
        for bytes in [100 * 1024, 1024 * 1024] {
            let note = super::fixture(bytes);
            assert_eq!(note.len(), bytes);
            assert!(note.ends_with('\n') && !note.contains('x'));
        }
    }

    #[test]
    fn the_injected_path_is_the_product_variable() {
        assert_eq!(
            super::ENV,
            "STICKYMD_DIAGNOSTIC_INPUT_LATENCY",
            "must match apps/stickymd-mac/src/platform/macos/input_latency.rs"
        );
    }
}
