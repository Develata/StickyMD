//! Ordered portable startup bootstrap.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

mod bootstrap;

pub use bootstrap::{BootstrapOutcome, bootstrap};
