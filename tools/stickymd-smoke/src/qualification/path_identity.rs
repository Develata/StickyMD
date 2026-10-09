//! Filesystem identity of evidence paths: alias-resolved, case-folded spellings, so a
//! junction, 8.3 name, verbatim prefix or `..` cannot make a reserved destination
//! look like an ordinary diagnostic path.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::{Path, PathBuf};

pub(super) fn matches_receipt(root: &Path, path: &Path, expected: &str) -> bool {
    normalize(&root.join(path)) == normalize(&root.join(expected))
}

pub(super) fn is_within(root: &Path, path: &Path, directory: &str) -> bool {
    let path = normalize(&root.join(path));
    let directory = normalize(&root.join(directory));
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) fn normalize(path: &Path) -> String {
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
pub(super) fn validate_output_spelling(path: &Path) -> Result<(), String> {
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
