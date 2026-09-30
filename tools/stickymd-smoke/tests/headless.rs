//! One Cargo-built integration target lets libtest overlap independent CLI cases.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cli_exit;
#[cfg(windows)]
mod package_path_wrapper;
#[cfg(windows)]
mod phase_entry;
#[cfg(windows)]
mod release_remote;
mod release_workflow;
#[cfg(windows)]
mod release_wrappers;
mod support;
