//! Opt-in shell substeps, separate from the stable startup v2 trace.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::{fmt::Write as _, path::PathBuf};

const DETAILS_PATH_ENV: &str = "STICKYMD_DIAGNOSTIC_STARTUP_DETAILS";

#[derive(Clone, Copy)]
pub enum StartupDetail {
    SplitModeBegin,
    SplitModeEnd,
    WindowShowBegin,
    WindowShowEnd,
    ToolIdentityBegin,
    ToolIdentityEnd,
}

const NAMES: [&str; 6] = [
    "split_mode_begin",
    "split_mode_end",
    "window_show_begin",
    "window_show_end",
    "tool_identity_begin",
    "tool_identity_end",
];

pub(super) struct Details {
    path: PathBuf,
    elapsed_us: [u128; 6],
    next: usize,
    invalid: bool,
}

impl Details {
    pub(super) fn from_environment(trace_enabled: bool) -> Option<Self> {
        // Details bind the exact legacy trace; without that input they stay off.
        trace_enabled
            .then(|| std::env::var_os(DETAILS_PATH_ENV))?
            .filter(|path| !path.is_empty())
            .map(|path| Self::new(path.into()))
    }

    fn new(path: PathBuf) -> Self {
        Self {
            path,
            elapsed_us: [0; 6],
            next: 0,
            invalid: false,
        }
    }

    pub(super) fn record(&mut self, milestone: StartupDetail, elapsed_us: u128) {
        if milestone as usize != self.next || self.next == NAMES.len() {
            self.invalid = true;
            return;
        }
        self.elapsed_us[self.next] = elapsed_us;
        self.next += 1;
    }

    pub(super) fn write(&self, legacy_trace: &str) -> Result<(), String> {
        let output = self.encode(legacy_trace)?;
        crate::platform::windows::diagnostic_event::write_startup_trace(
            &self.path,
            output.as_bytes(),
        )
        .map_err(|error| format!("cannot write startup details: {error}"))
    }

    fn encode(&self, legacy_trace: &str) -> Result<String, String> {
        if self.invalid || self.next != NAMES.len() {
            return Err("startup details are incomplete or out of order".into());
        }
        let mut output = format!("stickymd_startup_details_v1\npid={}\n", std::process::id());
        for (name, elapsed_us) in NAMES.iter().zip(self.elapsed_us) {
            let _ = writeln!(output, "{name}={elapsed_us}");
        }
        // Copying this small, content-free trace binds the pair byte-for-byte.
        // It needs no digest dependency or filesystem reread, and happens only
        // after EDITOR_READY. The original v2 file stays unchanged.
        output.push_str("legacy_trace_begin\n");
        output.push_str(legacy_trace);
        Ok(output)
    }
}

#[cfg(test)]
mod tests;
