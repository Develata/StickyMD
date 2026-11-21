//! Portable package selection, without claiming package or candidate validity.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::path::{Path, PathBuf};

use crate::repository::{command_text, workspace_version};

pub(crate) fn parse(arguments: &[String]) -> Result<PathBuf, String> {
    match arguments {
        [flag, directory] if flag == "--directory" && !directory.is_empty() => {
            Ok(PathBuf::from(directory))
        }
        _ => Err("usage: stickymd-smoke package-path --directory <directory>".to_owned()),
    }
}

pub(crate) fn resolve(root: &Path, directory: &Path) -> Result<PathBuf, String> {
    let directory = std::path::absolute(directory).map_err(|error| {
        format!(
            "cannot resolve package directory {}: {error}",
            directory.display()
        )
    })?;
    let mut packages = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(|error| format!("cannot enumerate {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot inspect package entry: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let normalized = name.to_ascii_lowercase();
        if normalized.starts_with("stickymd-")
            && normalized.ends_with("-windows-x64-portable.zip")
            && entry.path().is_file()
        {
            packages.push(entry.path());
        }
    }
    // Preserve the old helper's single-package behavior. Actual checksum,
    // version and provenance checks belong to the package verifier.
    if packages.len() == 1 {
        return Ok(packages.remove(0));
    }
    let version = workspace_version(root)?;
    let commit = command_text(root, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_text(root, "git", &["status", "--porcelain"])?.is_empty();
    let expected = local_archive_name(&version, &commit, dirty)?;
    packages
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name == expected.as_str())
        })
        .cloned()
        .ok_or_else(|| {
            format!(
                "cannot select current portable ZIP in {}; expected {expected} among {} package(s)",
                directory.display(),
                packages.len(),
            )
        })
}

pub(crate) fn local_archive_name(
    version: &str,
    commit: &str,
    dirty: bool,
) -> Result<String, String> {
    crate::integrity::validate_hex(commit, 40, "portable package Git commit SHA")?;
    let short = commit[..12].to_ascii_lowercase();
    let qualifier = if dirty {
        format!("local-validation-{short}-dirty")
    } else {
        format!("local-rc-{short}")
    };
    Ok(format!(
        "StickyMD-{version}-{qualifier}-windows-x64-portable.zip"
    ))
}

#[cfg(test)]
mod tests;
