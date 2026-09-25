//! Shared observation intervals for startup measurements and receipt validation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::time::Duration;

pub(crate) const WARM_CACHE_START_IDLE: Duration = Duration::from_secs(1);
pub(crate) const RAPID_RESTART_DIAGNOSTIC_IDLE: Duration = Duration::from_millis(250);
