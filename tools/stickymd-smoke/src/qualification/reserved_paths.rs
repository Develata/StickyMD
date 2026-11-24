//! The manual observation store's files and its lock, and the macOS GUI ledger's directory.
//! Every public evidence writer, the
//! release outputs and Promote consult this module, so it is an input of every module that
//! writes evidence; the store's own logic (`manual_observation::store`) is not.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use std::path::Path;

use super::file_lock::{SidecarGuard, sidecar};
use super::path_identity;

pub(super) const OBSERVATION_LOG: &str = "dist/evidence/manual-observations.json";
pub(super) const TARGET_RECEIPT: &str = "dist/evidence/manual-targets.json";
pub(super) const MANUAL_LOCK: &str = "dist/evidence/manual.lock";
pub(super) const MANUAL_ARCHIVE: &str = "dist/evidence/manual-archive";
/// The macOS GUI ledger (`mac-ledger`); only that command appends to it.
pub(crate) const MAC_GUI_LEDGER: &str = "dist/evidence/mac-gui-ledger";
/// The macOS source-test receipt, its run lock and its run record (`release
/// macos-source-tests`); only that command writes them, a replaced lock would let two runs
/// overlap, and a replaced record would let another receipt count (plan 11
/// #macos-readiness-check).
pub(crate) const MACOS_SOURCE_TEST_RECEIPT: &str = "dist/evidence/release-source-tests-macos.json";
pub(crate) const MACOS_SOURCE_TEST_LOCK: &str = "dist/evidence/.release-source-tests-macos.lock";
pub(crate) const MACOS_SOURCE_TEST_RUN: &str = "dist/evidence/.release-source-tests-macos.run";

/// Exclusive for any change to the store or to the candidate it is bound to (Promote,
/// Source Freeze); shared for reading a consistent target and log. Taken before the
/// release decision lock whenever both are needed.
pub(super) fn lock_manual_store(root: &Path, exclusive: bool) -> Result<SidecarGuard, String> {
    sidecar(
        &root.join(MANUAL_LOCK),
        exclusive,
        "manual observation store",
    )
}

/// Whether `path` names a file of the manual store, its archive, or anything below them
/// (so an output cannot occupy a store file that does not exist yet as a directory).
/// Public writers refuse them: only the store, under its lock, may replace or archive them.
pub(super) fn is_manual_store_path(root: &Path, path: &Path) -> bool {
    [OBSERVATION_LOG, TARGET_RECEIPT, MANUAL_LOCK, MANUAL_ARCHIVE]
        .iter()
        .any(|reserved| path_identity::reaches(root, path, reserved))
}

/// Whether `path` names the macOS source-test receipt, its lock or its run record. Public
/// writers, and the ledger's configurable destination, refuse them. On macOS the spelling is also
/// compared without case: a case-insensitive volume (the default) names one file by either case,
/// even one not created yet (review round 4, BLOCKER 4); on a case-sensitive volume this refuses
/// a few more names than it must.
pub(crate) fn is_macos_source_test_path(root: &Path, path: &Path) -> bool {
    [
        MACOS_SOURCE_TEST_RECEIPT,
        MACOS_SOURCE_TEST_LOCK,
        MACOS_SOURCE_TEST_RUN,
    ]
    .iter()
    .any(|reserved| {
        let folded = path_identity::normalize(&root.join(reserved)).to_lowercase();
        path_identity::reaches(root, path, reserved)
            // The path itself or any of its ancestors: creating the parents of a path through a
            // reserved name would make that name a directory (fix round, SHOULD_FIX 7).
            || (cfg!(target_os = "macos")
                && root
                    .join(path)
                    .ancestors()
                    .any(|ancestor| path_identity::normalize(ancestor).to_lowercase() == folded))
    })
}

/// Whether `path` names the macOS GUI ledger directory or anything below it. Public writers
/// refuse it: a suite report written there would replace the ledger's history.
pub(super) fn is_mac_ledger_path(root: &Path, path: &Path) -> bool {
    path_identity::reaches(root, path, MAC_GUI_LEDGER)
}
