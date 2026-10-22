//! Bounded native sampling while the observer performs the second launch.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn matches(kind: &str, state: manual_probe::State) -> bool {
    match kind {
        "hidden" => !state.visible,
        "minimized" => state.minimized,
        "docked" => state.visible && !state.minimized && state.collapsed,
        _ => false,
    }
}

struct Run {
    first: u64,
    last: u64,
    count: u64,
    state: char,
}
struct Samples {
    runs: Vec<Run>,
    timed_out: bool,
}
impl Samples {
    fn new(first_observed_ms: u64) -> Self {
        Self {
            runs: vec![Run {
                first: first_observed_ms,
                last: first_observed_ms,
                count: 1,
                state: 'p',
            }],
            timed_out: false,
        }
    }
    fn add(&mut self, time: u64, state: char) -> Result<(), String> {
        let last = self.runs.last_mut().ok_or("missing initial sample")?;
        if last.state == state {
            last.last = time;
            last.count += 1;
        } else {
            if self.runs.len() == 32 {
                return Err("wake sampling exceeded 32 state changes".to_owned());
            }
            self.runs.push(Run {
                first: time,
                last: time,
                count: 1,
                state,
            });
        }
        Ok(())
    }
    fn encoded(&self) -> String {
        self.runs
            .iter()
            .map(|run| format!("{}:{}:{}:{};", run.first, run.last, run.count, run.state))
            .collect()
    }
    fn activated(&self) -> bool {
        self.runs.iter().any(|run| run.state == 'a')
    }
}

struct Sampler {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<Samples, String>>>,
}
impl Sampler {
    fn start(
        hwnd: window::WindowHandle,
        monitors: Vec<manual_probe::Monitor>,
        kind: String,
        started: Instant,
        initial_ms: u64,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            window::enable_per_monitor_v2_dpi_awareness()?;
            // The original native sample and this thread share one monotonic
            // origin, including any delay before the worker actually starts.
            let mut samples = Samples::new(initial_ms);
            while !worker_stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(100));
                let elapsed = started.elapsed();
                if elapsed >= Duration::from_secs(120) {
                    samples.timed_out = true;
                    break;
                }
                let state = manual_probe::state(hwnd, &monitors)?;
                let code = if matches(&kind, state) {
                    'p'
                } else if state.expanded && state.activated {
                    'a'
                } else if state.expanded {
                    'r'
                } else {
                    'o'
                };
                samples.add(elapsed.as_millis() as u64, code)?;
            }
            Ok(samples)
        });
        Self {
            stop,
            worker: Some(worker),
        }
    }
    fn finish(&mut self) -> Result<Samples, String> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .ok_or("sampler already stopped")?
            .join()
            .map_err(|_| "wake sampler panicked".to_owned())?
    }
}
impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(super) fn run(
    process: &mut Launched,
    kind: &str,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<Preparation, String> {
    let hwnd = window::visible_window(process.instance.pid)?;
    ack(
        input,
        output,
        &format!(
            "Put this original window in the {kind} state. Leave it in that state and return here."
        ),
    )?;
    let monitors = manual_probe::monitors()?;
    let started = Instant::now();
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    if !matches(kind, manual_probe::state(hwnd, &monitors)?) {
        return Err(format!(
            "the original window is not in the required {kind} state"
        ));
    }
    let initial_ms = started.elapsed().as_millis() as u64;
    let marker = format!("MANUAL-WAKE-{}-{epoch}", process.instance.pid);
    let mut sampler = Sampler::start(hwnd, monitors.clone(), kind.to_owned(), started, initial_ms);
    ack(
        input,
        output,
        &format!(
            "Within 120 seconds, double-click {}. If the original window restores and activates, type {marker} into it without separately clicking or activating the window, then return here. Do not manually restore or restart the original instance.",
            process.program.join("StickyMD.exe").display()
        ),
    )?;
    let sampled = sampler.finish();
    if !confirm(
        input,
        output,
        "Did the required hidden/minimized/collapsed state remain continuously in effect until your double-click?",
    )? || !confirm(
        input,
        output,
        "Did that double-click occur before any restoration of the original window?",
    )? {
        return Err("the observer could not attest continuity and launch ordering".to_owned());
    }
    let mut prepared = Preparation::default();
    prepared
        .declared
        .insert("precondition".to_owned(), kind.to_owned());
    prepared
        .declared
        .insert("sample-origin-unix-ms".to_owned(), epoch.to_string());
    fact(&mut prepared, "continuous-until-launch", true);
    fact(&mut prepared, "launch-before-restore", true);
    if !process.child.is_running()? {
        prepared.pass_block =
            Some("the original process exited during the observed second launch".to_owned());
        return Ok(prepared);
    }
    let samples = sampled?;
    if samples.timed_out {
        return Err("wake sampling reached its 120-second limit".to_owned());
    }
    let encoded = samples.encoded();
    if encoded.len() > 1024 {
        return Err("wake sample record exceeds its bound".to_owned());
    }
    for (index, chunk) in encoded.as_bytes().chunks(128).enumerate() {
        prepared.declared.insert(
            format!("samples-{index:02}"),
            String::from_utf8(chunk.to_vec()).map_err(|error| error.to_string())?,
        );
    }
    if manual_probe::monitors()? != monitors {
        return Err("display topology changed during wake".to_owned());
    }
    let end = manual_probe::state(hwnd, &monitors)?;
    fact(&mut prepared, "restored", end.expanded);
    fact(&mut prepared, "activated", samples.activated());
    if !end.expanded {
        prepared.pass_block = Some(
            "the original window did not visibly restore and expand after the confirmed launch"
                .to_owned(),
        );
        return Ok(prepared);
    }
    if !samples.activated() {
        return Err(
            "restoration was observed but activation was denied or not sampled; record NOT TESTED"
                .to_owned(),
        );
    }
    let typed = confirm(
        input,
        output,
        "Did the typed marker enter the original window without a separate activation step?",
    )?;
    fact(&mut prepared, "typed-original", typed);
    let durable = crate::qualification::exact_desktop::wait_note(&process.program, |text| {
        text.contains(&marker)
    })
    .is_ok();
    fact(&mut prepared, "marker-durable", durable);
    if !typed || !durable {
        prepared.pass_block =
            Some("the typed marker was not verified in the original note".to_owned());
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampling_keeps_time_and_count_and_never_discards_state_changes() {
        let mut samples = Samples::new(0);
        samples.add(100, 'p').unwrap();
        samples.add(200, 'a').unwrap();
        samples.add(300, 'r').unwrap();
        assert_eq!(samples.encoded(), "0:100:2:p;200:200:1:a;300:300:1:r;");
        assert!(samples.activated());
        for time in 4..33 {
            samples
                .add(time * 100, if time % 2 == 0 { 'a' } else { 'r' })
                .unwrap();
        }
        assert!(samples.add(3300, 'r').is_err());
    }

    #[test]
    fn initial_native_sample_retains_its_time_before_worker_start() {
        let mut samples = Samples::new(7);
        samples.add(307, 'p').unwrap();
        samples.add(407, 'a').unwrap();
        assert_eq!(samples.encoded(), "7:307:2:p;407:407:1:a;");
    }
}
