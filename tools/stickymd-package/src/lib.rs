//! StickyMD release packaging: every release byte the repository's own tools produce. The Windows
//! and macOS package directories and ZIPs, the package README and third-party notices, the SBOMs
//! and checksum files, and the product input fingerprint `build-app.sh` writes into the macOS
//! bundle, with the shared code they need (JSON, hashing, atomic writes, temporary directories,
//! path identity and the output guard).
//!
//! The crate depends on the standard library only and never on `stickymd-smoke`, which depends on
//! it: the compiler then guarantees that no verification code reaches the bytes, which is why plan
//! 11 counts this crate, and not the verification tool, as a build input of both platforms.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app_inputs;
pub mod atomic;
pub mod checksums;
pub mod cli;
pub mod commands;
pub mod identity;
pub mod integrity;
pub mod json;
pub mod macos;
pub mod notices;
pub mod output_guard;
pub mod package_content;
pub mod package_inputs;
pub mod package_path;
pub mod package_publish;
pub mod package_staging;
pub mod package_workflow;
pub mod path_identity;
pub mod pe_dependencies;
pub mod platform;
pub mod repository;
pub mod sbom;
pub mod sbom_preparation;
pub mod sbom_workflow;
pub mod syft;
pub mod temporary;
pub mod windows;
