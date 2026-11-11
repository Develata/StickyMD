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

/// Whether `path` names the macOS GUI ledger directory or anything below it. Public writers
/// refuse it: a suite report written there would replace the ledger's history.
pub(super) fn is_mac_ledger_path(root: &Path, path: &Path) -> bool {
    path_identity::reaches(root, path, MAC_GUI_LEDGER)
}
