//! Manual acceptance as an append-only observation log bound to the current Promoted
//! Candidate: the case registry, the USER's target environments, role matching, the log
//! and its reduction to one verdict per case and role.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

pub(super) mod environment;
pub(super) mod log;
pub(super) mod reduce;
pub(super) mod registry;
pub(super) mod role;
pub(super) mod store;
pub(super) mod target;
pub(super) mod waiver;
