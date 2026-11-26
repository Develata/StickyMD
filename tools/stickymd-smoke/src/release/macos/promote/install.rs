//! The swap that installs a prepared macOS candidate: the old receipt and record are retired,
//! the old candidate is set aside, the new one moved in and the receipt written last; a failure
//! before the receipt undoes every step.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use super::super::content;
use super::{RECEIPT, REUSE_RECORD, STAGING};
use crate::atomic_evidence;
use std::fs;
use std::path::Path;

/// One change `install` made, undone in reverse order if a later step fails.
enum Step {
    ReceiptRetired(Vec<u8>),
    RecordRetired(Vec<u8>),
    OldSetAside,
    NewInPlace,
    RecordWritten,
}

/// The bytes at `path`, `None` only when nothing is there; any other error stops the caller
/// before it changes anything.
fn existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// Swaps the prepared candidate in. The receipt is retired first, so from then on the resolver
/// refuses the staging directory; the old candidate is kept aside; the new one moves in, then
/// the reuse record, and the receipt last: writing it is the commit. A failure before that
/// undoes every step in reverse order (the old receipt last) and reports any step it could not
/// undo. After the commit, leftover cleanup is only reported: the new candidate stands.
pub(super) fn install(
    root: &Path,
    staged: &Path,
    record: Option<&str>,
    receipt: &str,
) -> Result<(), String> {
    let target = root.join(STAGING);
    let receipt_path = root.join(RECEIPT);
    let record_path = root.join(REUSE_RECORD);
    let aside = root.join(format!(
        "dist/.exact-candidate-macos-old-{}",
        std::process::id()
    ));
    let old_receipt = existing(&receipt_path)?;
    let old_record = existing(&record_path)?;
    let mut done = Vec::new();
    let io = |what: &str, error: std::io::Error| format!("cannot {what}: {error}");
    let outcome = (|| {
        if let Some(bytes) = old_receipt {
            fs::remove_file(&receipt_path).map_err(|error| io("retire the receipt", error))?;
            done.push(Step::ReceiptRetired(bytes));
        }
        if let Some(bytes) = old_record {
            fs::remove_file(&record_path).map_err(|error| io("retire the reuse record", error))?;
            done.push(Step::RecordRetired(bytes));
        }
        if target.exists() {
            fs::rename(&target, &aside)
                .map_err(|error| io("set the old candidate aside", error))?;
            done.push(Step::OldSetAside);
        }
        fs::rename(staged, &target).map_err(|error| io("move the candidate into place", error))?;
        done.push(Step::NewInPlace);
        if let Some(record) = record {
            atomic_evidence::write(&record_path, record.as_bytes())?;
            done.push(Step::RecordWritten);
        }
        atomic_evidence::write(&receipt_path, receipt.as_bytes())
    })();
    if let Err(error) = outcome {
        let mut failures = Vec::new();
        for step in done.into_iter().rev() {
            let undone = match step {
                Step::RecordWritten => fs::remove_file(&record_path).map_err(|e| e.to_string()),
                Step::NewInPlace => fs::remove_dir_all(&target).map_err(|e| e.to_string()),
                Step::OldSetAside => fs::rename(&aside, &target).map_err(|e| e.to_string()),
                Step::RecordRetired(bytes) => atomic_evidence::write(&record_path, &bytes),
                Step::ReceiptRetired(bytes) => atomic_evidence::write(&receipt_path, &bytes),
            };
            if let Err(failure) = undone {
                failures.push(failure);
            }
        }
        let _ = fs::remove_dir_all(staged);
        return Err(if failures.is_empty() {
            format!("{error}; the previous candidate is back in place")
        } else {
            format!(
                "{error}; the previous candidate could not be fully restored: {}",
                failures.join("; ")
            )
        });
    }
    if aside.exists() {
        // Only what a promotion writes is removed: a note written beside the staged `.app` is
        // the user's (M5 gate review B9; AGENTS.md: never delete user files).
        let kept = match foreign(&aside, "") {
            Ok(None) => fs::remove_dir_all(&aside)
                .err()
                .map(|error| error.to_string()),
            Ok(Some(path)) => Some(format!("it holds {path}, which no promotion writes")),
            Err(error) => Some(error),
        };
        if let Some(reason) = kept {
            eprintln!(
                "warning: promoted; the previous candidate stays at {}: {reason}",
                aside.display()
            );
        }
    }
    Ok(())
}

/// The first entry under `directory` (an old staging directory; `relative` is its path inside
/// it) that a promotion does not write, if any: a promotion writes the ZIP, the checksum file
/// and the SBOM, and extracts the ZIP's inventory beside them (the files beside the app, and
/// anything under the app). Links count as foreign, so nothing outside is looked at.
fn foreign(directory: &Path, relative: &str) -> Result<Option<String>, String> {
    let platform = crate::release::platform::Platform::MacOsArm64;
    let archive = platform.archive_name("*");
    let (prefix, suffix) = archive
        .split_once('*')
        .ok_or("no version in the ZIP name")?;
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| format!("{name:?} is not UTF-8"))?;
        let path = if relative.is_empty() {
            name.to_owned()
        } else {
            format!("{relative}/{name}")
        };
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        // The bundle is the app's own; everything under it is what the ZIP held.
        if path == content::APP || path.starts_with(&format!("{}/", content::APP)) {
            continue;
        }
        let promoted = if kind.is_dir() {
            content::is_inventory_directory(&path)
        } else if !kind.is_file() {
            false
        } else if relative.is_empty() {
            [platform.checksum_file(), platform.sbom_file()].contains(&name)
                || name
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.strip_suffix(suffix))
                    .is_some_and(|version| !version.is_empty())
        } else {
            content::FILES.iter().any(|(file, _)| *file == path)
        };
        if !promoted {
            return Ok(Some(path));
        }
        if kind.is_dir()
            && let Some(found) = foreign(&entry.path(), &path)?
        {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::temporary::TemporaryDirectory;

    /// A file every promotion writes, which tells the old candidate from the new one.
    const MARKER: &str = super::super::content::README;

    fn tree(root: &Path, candidate: &str, receipt: &str, record: Option<&str>) {
        fs::create_dir_all(root.join(STAGING).join("StickyMD")).unwrap();
        fs::write(root.join(STAGING).join(MARKER), candidate).unwrap();
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        if let Some(record) = record {
            fs::write(root.join(REUSE_RECORD), record).unwrap();
        }
    }

    fn read(root: &Path) -> (String, String, Option<String>) {
        (
            fs::read_to_string(root.join(STAGING).join(MARKER)).unwrap(),
            fs::read_to_string(root.join(RECEIPT)).unwrap(),
            fs::read_to_string(root.join(REUSE_RECORD)).ok(),
        )
    }

    #[test]
    fn install_swaps_the_candidate_and_writes_the_receipt_last() {
        let scratch = TemporaryDirectory::new("macos-install").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        // The old staging holds just what a promotion writes (of another version), so it goes.
        let platform = crate::release::platform::Platform::MacOsArm64;
        let old = root.join(STAGING);
        for file in [
            platform.archive_name("0.2.0"),
            platform.checksum_file().to_owned(),
            platform.sbom_file().to_owned(),
        ]
        .into_iter()
        .chain(content::FILES.iter().map(|(file, _)| (*file).to_owned()))
        .chain(content::APP_FILES.iter().map(|file| (*file).to_owned()))
        {
            fs::create_dir_all(old.join(&file).parent().unwrap()).unwrap();
            fs::write(old.join(&file), "old").unwrap();
        }
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        install(root, &staged, None, "new receipt").unwrap();
        // Same-commit promotion: the old ancestor record must not survive beside the new receipt.
        assert_eq!(read(root), ("new".into(), "new receipt".into(), None));
        assert!(!staged.exists());
        let leftovers = fs::read_dir(root.join("dist"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(".exact-candidate-macos"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// The directories set aside in `dist/` by `install`.
    fn set_aside(root: &Path) -> Vec<std::path::PathBuf> {
        fs::read_dir(root.join("dist"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".exact-candidate-macos-old-")
            })
            .collect()
    }

    /// M5 gate review B9: a note written beside the staged `.app` (its portable Note Root) is
    /// the user's; the old staging that holds it is kept, not deleted, when a new candidate
    /// replaces it.
    #[test]
    fn an_old_staging_holding_a_note_is_kept() {
        let scratch = TemporaryDirectory::new("macos-install-note").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", None);
        let note = root.join(STAGING).join("StickyMD/note/note.md");
        fs::create_dir_all(note.parent().unwrap()).unwrap();
        fs::write(&note, "the user's words").unwrap();
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        install(root, &staged, None, "new receipt").unwrap();
        assert_eq!(read(root).0, "new");
        let [kept] = set_aside(root).try_into().unwrap();
        assert_eq!(
            fs::read_to_string(kept.join("StickyMD/note/note.md")).unwrap(),
            "the user's words"
        );
    }

    #[test]
    fn a_failed_install_puts_the_old_candidate_receipt_and_record_back() {
        let scratch = TemporaryDirectory::new("macos-install-failure").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        // The prepared directory vanished: moving it into place fails after the old one moved.
        let error = install(
            root,
            &root.join("dist/.gone"),
            Some("new record"),
            "new receipt",
        )
        .unwrap_err();
        assert!(error.contains("into place"), "{error}");
        assert_eq!(
            read(root),
            (
                "old".into(),
                "old receipt".into(),
                Some("old record".into())
            )
        );
    }

    #[test]
    fn an_unreadable_receipt_stops_install_before_any_change() {
        let scratch = TemporaryDirectory::new("macos-install-unreadable").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", None);
        fs::remove_file(root.join(RECEIPT)).unwrap();
        // A directory where the receipt belongs cannot be read as one, nor taken as absent.
        fs::create_dir(root.join(RECEIPT)).unwrap();
        let staged = root.join("dist/.new");
        fs::create_dir_all(&staged).unwrap();
        assert!(install(root, &staged, None, "new receipt").is_err());
        assert_eq!(
            fs::read_to_string(root.join(STAGING).join(MARKER)).unwrap(),
            "old"
        );
        assert!(staged.exists());
    }
}
