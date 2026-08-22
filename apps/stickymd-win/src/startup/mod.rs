//! Ordered portable startup bootstrap.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

mod bootstrap;
mod diagnostics;

pub use bootstrap::{BootstrapMilestone, BootstrapOutcome, bootstrap_observed};
pub use diagnostics::StartupDiagnostics;
