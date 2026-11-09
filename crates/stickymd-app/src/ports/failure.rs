//! The failure half of an Execution Receipt.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::fmt;

/// Why a request did not succeed (plan 03 rule 4). A failure never commits a saved or hidden state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The execution ran and failed.
    Failed,
    /// The resource could not be observed now; a later attempt may succeed.
    Unavailable,
    /// This host does not have the capability.
    Unsupported,
}

/// A failed storage request. `detail` is the host's own message, shown in diagnostics verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageFailure {
    kind: FailureKind,
    detail: Box<str>,
}

impl StorageFailure {
    pub fn new(kind: FailureKind, detail: impl Into<Box<str>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    pub const fn kind(&self) -> FailureKind {
        self.kind
    }
}

impl fmt::Display for StorageFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for StorageFailure {}

/// A failed request to the desktop host, with the same shape: the status line shows `detail`.
pub type HostFailure = StorageFailure;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_shows_exactly_the_host_message() {
        let failure = StorageFailure::new(FailureKind::Unavailable, "cannot read note: cause");
        assert_eq!(failure.to_string(), "cannot read note: cause");
        assert_eq!(failure.kind(), FailureKind::Unavailable);
    }
}
