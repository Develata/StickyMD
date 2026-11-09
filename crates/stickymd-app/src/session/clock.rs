//! The injected monotonic time every shared schedule reads (plan 03, rule 7).
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::time::Instant;

pub trait MonotonicClock {
    /// Milliseconds since the session began.
    fn now_ms(&self) -> u64;
}

/// The process's monotonic clock.
pub struct InstantClock {
    started: Instant,
}

impl InstantClock {
    pub const fn starting_at(started: Instant) -> Self {
        Self { started }
    }
}

impl MonotonicClock for InstantClock {
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
}
