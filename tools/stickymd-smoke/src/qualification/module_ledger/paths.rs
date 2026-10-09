//! Path identity for evidence destinations: alias-resolved spellings and the
//! coordinator-reserved success storage that diagnostics must never target.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

pub(in crate::qualification) fn matches_receipt(root: &Path, path: &Path, expected: &str) -> bool {
    normalize(&root.join(path)) == normalize(&root.join(expected))
}

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

pub(in crate::qualification) fn is_within(root: &Path, path: &Path, directory: &str) -> bool {
    let path = normalize(&root.join(path));
    let directory = normalize(&root.join(directory));
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalize(path: &Path) -> String {
    #[cfg(windows)]
    let win32_path = win32_spelling(path);
    #[cfg(windows)]
    let path = win32_path.as_path();
    // Resolve existing filesystem identity before lexical cleanup: Windows verbatim/8.3
    // paths and directory junctions can name the same reserved evidence destination.
    // New output files inherit the identity of their nearest existing ancestor.
    let resolved = path.ancestors().find_map(|ancestor| {
        ancestor
            .canonicalize()
            .ok()
            .map(|existing| existing.join(path.strip_prefix(ancestor).expect("ancestor prefix")))
    });
    let path = resolved.as_deref().unwrap_or(path);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(windows)]
fn win32_spelling(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    if is_verbatim(path) {
        return path.to_path_buf();
    }
    // MoveFileExW receives ordinary Win32 paths for ordinary CLI output paths.
    // Only the final component is folded; verbatim paths keep their literal name.
    let Some(name) = path.file_name() else {
        return path.to_path_buf();
    };
    let mut units = name.encode_wide().collect::<Vec<_>>();
    while units
        .last()
        .is_some_and(|unit| matches!(*unit, 0x20 | 0x2e))
    {
        units.pop();
    }
    path.with_file_name(OsString::from_wide(&units))
}

#[cfg(windows)]
fn is_verbatim(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, std::path::Component::Prefix(prefix) if prefix.kind().is_verbatim())
    })
}

#[cfg(windows)]
pub(in crate::qualification) fn validate_output_spelling(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    if !is_verbatim(path) {
        let mut components = path.components();
        components.next_back();
        if components.any(|component| {
            matches!(component, std::path::Component::Normal(name)
                if name.encode_wide().last().is_some_and(|unit| matches!(unit, 0x20 | 0x2e)))
        }) {
            // An ancestor becomes the final component while canonicalizing. Do not
            // let that fold an intermediate directory that Win32 would keep literal.
            return Err("ordinary Windows evidence paths cannot contain a directory ending in a dot or space; use a verbatim path or a diagnostic path without ambiguous directories".into());
        }
    }
    Ok(())
}
