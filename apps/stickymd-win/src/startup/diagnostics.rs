//! Opt-in startup milestones for copied-Release performance verification.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

const READY_EVENT_ENV: &str = "STICKYMD_DIAGNOSTIC_READY_EVENT";
const TRACE_PATH_ENV: &str = "STICKYMD_DIAGNOSTIC_STARTUP_TRACE";

/// Disabled by default; records only monotonic durations and fixed milestone names.
/// It never records note text, paths, clipboard data, or other user content.
pub struct StartupDiagnostics {
    started: Instant,
    ready_event: Option<String>,
    trace_path: Option<PathBuf>,
    milestones: Vec<(&'static str, u128)>,
    finished: bool,
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
        Self {
            started: Instant::now(),
            ready_event,
            trace_path,
            milestones: if enabled {
                Vec::with_capacity(16)
            } else {
                Vec::new()
            },
            finished: false,
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

    /// Completes the startup measurement after the first successful present.
    /// The event is signalled before the optional trace write, so the external
    /// duration excludes diagnostic file I/O.
    pub fn editor_ready(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.record("editor_ready");
        self.finished = true;
        if let Some(name) = self.ready_event.as_deref() {
            crate::platform::windows::diagnostic_event::signal_named_event(name)
                .map_err(|error| format!("cannot signal diagnostic ready event: {error}"))?;
        }
        if let Some(path) = &self.trace_path {
            let mut output = String::from("stickymd_startup_trace_v1\n");
            for (name, elapsed_us) in &self.milestones {
                let _ = writeln!(output, "{name}={elapsed_us}");
            }
            crate::platform::windows::diagnostic_event::write_startup_trace(
                path,
                output.as_bytes(),
            )
            .map_err(|error| format!("cannot write startup trace: {error}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::StartupDiagnostics;

    #[test]
    fn disabled_diagnostics_do_not_accumulate_milestones() {
        // The regular test process does not set the private smoke variables.
        let mut diagnostics = StartupDiagnostics::from_environment();
        if std::env::var_os("STICKYMD_DIAGNOSTIC_READY_EVENT").is_none()
            && std::env::var_os("STICKYMD_DIAGNOSTIC_STARTUP_TRACE").is_none()
        {
            diagnostics.record("main_enter");
            assert!(diagnostics.milestones.is_empty());
        }
    }
}
