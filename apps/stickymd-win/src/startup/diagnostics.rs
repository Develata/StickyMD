//! Opt-in startup milestones for copied-Release performance verification.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

mod details;
pub use details::StartupDetail;

const READY_EVENT_ENV: &str = "STICKYMD_DIAGNOSTIC_READY_EVENT";
const TRACE_PATH_ENV: &str = "STICKYMD_DIAGNOSTIC_STARTUP_TRACE";
const EXIT_AFTER_READY_ENV: &str = "STICKYMD_DIAGNOSTIC_EXIT_AFTER_READY";

/// Disabled by default; records only monotonic durations and fixed milestone names.
/// It never records note text, paths, clipboard data, or other user content.
pub struct StartupDiagnostics {
    started: Instant,
    ready_event: Option<String>,
    trace_path: Option<PathBuf>,
    exit_after_ready: bool,
    milestones: Vec<(&'static str, u128)>,
    details: Option<details::Details>,
    finished: bool,
    failed: bool,
}

impl StartupDiagnostics {
    pub fn from_environment() -> Self {
        let ready_event = env::var(READY_EVENT_ENV)
            .ok()
            .filter(|value| !value.is_empty());
        let trace_path = env::var_os(TRACE_PATH_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let enabled = ready_event.is_some() || trace_path.is_some();
        let details = details::Details::from_environment(trace_path.is_some());
        let mut milestones = if enabled {
            Vec::with_capacity(26)
        } else {
            Vec::new()
        };
        if enabled {
            // This is the first Rust-side diagnostic epoch. OS process creation
            // remains visible as external elapsed minus this internal trace.
            milestones.push(("process_start", 0));
        }
        Self {
            started: Instant::now(),
            ready_event,
            trace_path,
            exit_after_ready: enabled
                && env::var(EXIT_AFTER_READY_ENV).is_ok_and(|value| value == "1"),
            milestones,
            details,
            finished: false,
            failed: false,
        }
    }

    pub fn record(&mut self, name: &'static str) {
        if self.finished || (self.ready_event.is_none() && self.trace_path.is_none()) {
            return;
        }
        if self
            .milestones
            .last()
            .is_some_and(|(last, _)| *last == name)
        {
            return;
        }
        self.milestones
            .push((name, self.started.elapsed().as_micros()));
    }

    pub fn record_detail(&mut self, milestone: StartupDetail) {
        if !self.finished
            && let Some(details) = &mut self.details
        {
            details.record(milestone, self.started.elapsed().as_micros());
        }
    }

    /// Completes the startup measurement after the first successful present.
    /// The event is signalled before the optional trace write, so the external
    /// duration excludes diagnostic file I/O.
    pub fn editor_ready(&mut self) -> Result<bool, String> {
        if self.finished {
            return Ok(false);
        }
        self.record("editor_ready");
        self.finished = true;
        let result = self.publish();
        self.failed = result.is_err();
        result.map(|()| self.exit_after_ready)
    }

    /// An explicitly requested diagnostic exit survives a capture failure.
    pub fn exit_requested(&self) -> bool {
        self.finished && self.exit_after_ready
    }

    /// Return from main normally so app/worker guards are dropped before exit.
    pub fn exit_code(&self) -> std::process::ExitCode {
        if self.exit_requested() && self.failed {
            std::process::ExitCode::FAILURE
        } else {
            std::process::ExitCode::SUCCESS
        }
    }

    fn publish(&self) -> Result<(), String> {
        if let Some(name) = self.ready_event.as_deref() {
            crate::platform::windows::diagnostic_event::signal_named_event(name)
                .map_err(|error| format!("cannot signal diagnostic ready event: {error}"))?;
        }
        if let Some(path) = &self.trace_path {
            let mut output = String::from("stickymd_startup_trace_v2\n");
            for (name, elapsed_us) in &self.milestones {
                let _ = writeln!(output, "{name}={elapsed_us}");
            }
            crate::platform::windows::diagnostic_event::write_startup_trace(
                path,
                output.as_bytes(),
            )
            .map_err(|error| format!("cannot write startup trace: {error}"))?;
            if let Some(details) = &self.details {
                details.write(&output)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
