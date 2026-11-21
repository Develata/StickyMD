//! The checks of the macOS ZIP that need no macOS tool, for the promotion workflow on a Windows
//! runner: `tar` (bsdtar, also on Windows) lists the archive and prints its README, and the
//! listing must be exactly the inventory. The bundle's signature and architecture were checked
//! on macOS by the build workflow and by local promotion.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use super::content::{self, README};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

fn tar(arguments: &[&std::ffi::OsStr]) -> Result<Vec<u8>, String> {
    let output = Command::new("tar")
        .args(arguments)
        .output()
        .map_err(|error| format!("cannot run tar: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "tar {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Checks a `tar -tf` listing: every entry an inventory file or one of its parent directories.
pub(crate) fn check_listing(listing: &str) -> Result<(), String> {
    let mut files = BTreeSet::new();
    for entry in listing.lines().filter(|line| !line.is_empty()) {
        if entry.ends_with('/') {
            if !content::is_inventory_directory(entry) {
                return Err(format!(
                    "the macOS package holds an unexpected directory {entry}"
                ));
            }
        } else if !files.insert(entry.to_owned()) {
            return Err(format!("the macOS package lists {entry} twice"));
        }
    }
    content::check_files(&files)
}

/// Checks the archive's own member list (before anything is extracted, so a name stored twice
/// cannot hide behind one extracted file).
pub(crate) fn check_archive(zip: &Path) -> Result<(), String> {
    let listing = tar(&["-tf".as_ref(), zip.as_os_str()])?;
    check_listing(std::str::from_utf8(&listing).map_err(|_| "the ZIP listing is not UTF-8")?)
}

/// The archive's inventory and the source commit its README names.
pub(crate) fn verify_archive(zip: &Path) -> Result<String, String> {
    check_archive(zip)?;
    let readme = tar(&["-xOf".as_ref(), zip.as_os_str(), README.as_ref()])?;
    content::source_commit(std::str::from_utf8(&readme).map_err(|_| "the README is not UTF-8")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::macos::content::{APP_FILES, FILES};

    fn listing(extra: &[&str]) -> String {
        let mut lines = vec!["StickyMD/", "StickyMD/StickyMD.app/", "StickyMD/licenses/"];
        lines.extend(FILES.iter().map(|(name, _)| *name));
        lines.extend(APP_FILES.iter().copied());
        lines.extend(extra);
        lines.join("\n")
    }

    #[test]
    fn a_listing_of_exactly_the_inventory_passes() {
        check_listing(&listing(&[])).unwrap();
        for extra in [
            "StickyMD/note/",
            "StickyMD/note/note.md",
            "__MACOSX/",
            "StickyMD/README.txt",
        ] {
            assert!(check_listing(&listing(&[extra])).is_err(), "{extra}");
        }
    }
}
