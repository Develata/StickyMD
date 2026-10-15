//! The coordinator-reserved success storage that diagnostics must never target.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

use crate::qualification::path_identity::{is_within, normalize};

/// Both the retired per-worktree ledger and the clone-wide store are coordinator-owned.
pub(in crate::qualification) fn is_success_storage_path(root: &Path, path: &Path) -> bool {
    is_within(root, path, "dist/evidence/module-success") || is_shared_store_path(root, path)
}

/// The store always lives at `<git common dir>/stickymd/qualification-ledger`. Matching
/// that segment pair on the alias-resolved path needs no git query, so an unavailable
/// git cannot turn the protection off while an existing store is still on disk.
fn is_shared_store_path(root: &Path, path: &Path) -> bool {
    // Case-folded: on NTFS a differently cased spelling names the same directory.
    let target = normalize(&root.join(path));
    if format!("{target}/")
        .to_ascii_lowercase()
        .contains(STORE_SEGMENTS)
    {
        return true;
    }
    // The store directory itself may be a junction to another place, which removes the
    // segment from the resolved target; compare with the resolved store root as well.
    match git_common_dir_from_files(root) {
        Ok(None) => false,
        Ok(Some(common)) => {
            let store = normalize(&common.join("stickymd").join("qualification-ledger"));
            let (target, store) = (target.to_ascii_lowercase(), store.to_ascii_lowercase());
            target == store
                || target
                    .strip_prefix(&store)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }
        // A `.git` that cannot be interpreted leaves the store location unknown.
        Err(_) => true,
    }
}

const STORE_SEGMENTS: &str = "/stickymd/qualification-ledger/";

/// The git common directory from `.git` metadata files alone, so the reserved-path
/// check never depends on running git. `None` when the root has no `.git` at all,
/// which proves no clone-wide store belongs to it.
pub(super) fn git_common_dir_from_files(root: &Path) -> Result<Option<PathBuf>, String> {
    let dot_git = root.join(".git");
    let git_dir = match fs::metadata(&dot_git) {
        Ok(metadata) if metadata.is_dir() => dot_git,
        Ok(_) => {
            // A linked worktree: `gitdir: <path>` names its private git directory.
            let text = fs::read_to_string(&dot_git)
                .map_err(|error| format!("cannot read {}: {error}", dot_git.display()))?;
            let pointer = text
                .trim()
                .strip_prefix("gitdir:")
                .ok_or_else(|| format!("{} is not a gitdir file", dot_git.display()))?
                .trim();
            existing_directory(root, pointer, "gitdir")?
        }
        // Only a truly absent entry proves there is no store; a dangling link is not absence.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !entry_exists(&dot_git) => {
            return Ok(None);
        }
        Err(error) => return Err(format!("cannot inspect {}: {error}", dot_git.display())),
    };
    let commondir = git_dir.join("commondir");
    match fs::read_to_string(&commondir) {
        Ok(text) => Ok(Some(existing_directory(
            &git_dir,
            text.trim(),
            "commondir",
        )?)),
        // A gitfile may name the repository itself (`--separate-git-dir`).
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !entry_exists(&commondir) => {
            Ok(Some(git_dir))
        }
        Err(error) => Err(format!("cannot read git commondir: {error}")),
    }
}

/// Whether a directory entry exists at all, without following a link it may be.
fn entry_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// A non-empty pointer that resolves to an existing directory; anything else leaves
/// the store location unknown, which the reserved-path check treats as reserved.
fn existing_directory(base: &Path, pointer: &str, label: &str) -> Result<PathBuf, String> {
    if pointer.is_empty() {
        return Err(format!("git {label} pointer is empty"));
    }
    let pointer = Path::new(pointer);
    let directory = if pointer.is_absolute() {
        pointer.to_path_buf()
    } else {
        base.join(pointer)
    };
    if !directory.is_dir() {
        return Err(format!(
            "git {label} does not name a directory: {}",
            directory.display()
        ));
    }
    Ok(directory)
}
