//! The swap that installs a prepared macOS candidate: the old receipt and record are retired,
//! the old candidate is set aside, the new one moved in and the receipt written last; a failure
//! before the receipt undoes every step.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

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
    if aside.exists()
        && let Err(error) = fs::remove_dir_all(&aside)
    {
        eprintln!(
            "warning: promoted; the previous candidate stays at {}: {error}",
            aside.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::temporary::TemporaryDirectory;

    fn tree(root: &Path, candidate: &str, receipt: &str, record: Option<&str>) {
        fs::create_dir_all(root.join(STAGING)).unwrap();
        fs::write(root.join(STAGING).join("marker"), candidate).unwrap();
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        if let Some(record) = record {
            fs::write(root.join(REUSE_RECORD), record).unwrap();
        }
    }

    fn read(root: &Path) -> (String, String, Option<String>) {
        (
            fs::read_to_string(root.join(STAGING).join("marker")).unwrap(),
            fs::read_to_string(root.join(RECEIPT)).unwrap(),
            fs::read_to_string(root.join(REUSE_RECORD)).ok(),
        )
    }

    #[test]
    fn install_swaps_the_candidate_and_writes_the_receipt_last() {
        let scratch = TemporaryDirectory::new("macos-install").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        let staged = root.join("dist/.new");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("marker"), "new").unwrap();
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
            fs::read_to_string(root.join(STAGING).join("marker")).unwrap(),
            "old"
        );
        assert!(staged.exists());
    }
}
