//! The coordinator-reserved success storage that diagnostics must never target.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::path::Path;

use crate::qualification::path_identity::is_within;
#[cfg(test)]
pub(super) use stickymd_package::output_guard::git_common_dir_from_files;

/// Both the retired per-worktree ledger and the clone-wide store are coordinator-owned.
pub(in crate::qualification) fn is_success_storage_path(root: &Path, path: &Path) -> bool {
    is_within(root, path, "dist/evidence/module-success")
        || stickymd_package::output_guard::is_ledger_store(root, path)
}
