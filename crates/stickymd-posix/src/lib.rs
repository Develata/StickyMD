//! The POSIX native layer a host builds its storage and single instance on: directory handles and
//! `*at` calls, file identity, atomic publish, the `flock` instance lock and socket wake. Like a
//! host's platform adapter directory, it holds `unsafe` and native bindings. Only the macOS
//! implementation exists; on other targets the crate is empty.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "macos")]
pub mod atomic_file;
#[cfg(target_os = "macos")]
pub mod file;
#[cfg(target_os = "macos")]
pub mod instance;
#[cfg(target_os = "macos")]
mod sys;
#[cfg(all(test, target_os = "macos"))]
mod test_support;
