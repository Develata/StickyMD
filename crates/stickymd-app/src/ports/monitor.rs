//! The displays of the desktop, as neutral facts.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use crate::flow::window::geometry::{MonitorGeometry, MonitorIdentity};

use super::FailureKind;

/// Every display, and the one the window is on when that is known.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorSnapshot {
    pub displays: Vec<MonitorGeometry>,
    pub current: Option<MonitorIdentity>,
}

pub trait MonitorSource {
    /// `Unavailable` while there is no window to ask.
    fn snapshot(&self) -> Result<MonitorSnapshot, FailureKind>;
}
