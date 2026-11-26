//! The swap that installs a prepared macOS candidate: the old receipt and records are retired,
//! the old candidate is set aside, the new one moved in and the receipt written last; a failure
//! before the receipt undoes every step. The promotion-check record follows the receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use super::super::content;
use super::{PROMOTION_CHECK, RECEIPT, REUSE_RECORD, STAGING};
use crate::atomic_evidence;
use std::fs;
use std::path::Path;

/// One change `install` made, undone in reverse order if a later step fails.
enum Step {
    ReceiptRetired(Vec<u8>),
    RecordRetired(Vec<u8>),
    PromotionRetired(Vec<u8>),
    OldSetAside,
    NewInPlace,
    RecordWritten,
}

/// The bytes of the regular file at `path`, `None` only when nothing is there; a link (even a
/// dangling one) or anything but a regular file, and any other error, stops the caller before
/// it changes anything (fix-round review R3-5: the resolver refuses what this would skip).
fn existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!(
                "{} is not a regular file (a link or a directory); move it away",
                path.display()
            ));
        }
        Ok(_) => {}
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// What `install` writes: the reuse record (when the candidate was built on an ancestor), the
/// receipt, and the promotion-check record on R.
pub(super) struct Outputs<'a> {
    pub reuse: Option<&'a str>,
    pub receipt: &'a str,
    pub promotion: &'a str,
}

/// Swaps the prepared candidate in. The receipt is retired first, so from then on the resolver
/// refuses the staging directory, and the old records with it; the old candidate is kept aside;
/// the new one moves in, then the reuse record, then `confirm` runs (the source identity is
/// still a clean R, M5 gate review B2), and the receipt is written last: writing it is the
/// commit. A failure before that undoes every step in reverse order (the old receipt last) and
/// reports any step it could not undo. After the commit the promotion-check record is written:
/// if it cannot be, the new candidate stands and the error says to promote again on R; leftover
/// cleanup is only reported.
pub(super) fn install(
    root: &Path,
    staged: &Path,
    outputs: Outputs<'_>,
    confirm: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let target = root.join(STAGING);
    let receipt_path = root.join(RECEIPT);
    let record_path = root.join(REUSE_RECORD);
    let promotion_path = root.join(PROMOTION_CHECK);
    let aside = free_aside(root)?;
    let old_receipt = existing(&receipt_path)?;
    // The one ZIP the old staging may hold, as its receipt names it (fix-round review R1-3).
    let old_zip = old_zip(old_receipt.as_deref());
    let old_record = existing(&record_path)?;
    let old_promotion = existing(&promotion_path)?;
    // Only a promotion-check record is ours to retire (fix-round review R1-9).
    if old_promotion
        .as_deref()
        .is_some_and(|bytes| !super::super::promotion_check::is_record(bytes))
    {
        return Err(format!(
            "{} is not a promotion-check record; it is kept: move it away and promote again",
            promotion_path.display()
        ));
    }
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
        if let Some(bytes) = old_promotion {
            fs::remove_file(&promotion_path)
                .map_err(|error| io("retire the promotion-check record", error))?;
            done.push(Step::PromotionRetired(bytes));
        }
        if target.exists() {
            fs::rename(&target, &aside)
                .map_err(|error| io("set the old candidate aside", error))?;
            done.push(Step::OldSetAside);
        }
        fs::rename(staged, &target).map_err(|error| io("move the candidate into place", error))?;
        done.push(Step::NewInPlace);
        if let Some(record) = outputs.reuse {
            atomic_evidence::write(&record_path, record.as_bytes())?;
            done.push(Step::RecordWritten);
        }
        confirm()?;
        atomic_evidence::write(&receipt_path, outputs.receipt.as_bytes())
    })();
    if let Err(error) = outcome {
        let mut failures = Vec::new();
        for step in done.into_iter().rev() {
            let undone = match step {
                Step::RecordWritten => fs::remove_file(&record_path).map_err(|e| e.to_string()),
                Step::NewInPlace => fs::remove_dir_all(&target).map_err(|e| e.to_string()),
                Step::OldSetAside => fs::rename(&aside, &target).map_err(|e| e.to_string()),
                Step::PromotionRetired(bytes) => atomic_evidence::write(&promotion_path, &bytes),
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
    let recorded = atomic_evidence::write(&promotion_path, outputs.promotion.as_bytes());
    // Only the directory this run set aside is cleaned up (fix-round review R1-11).
    if done.iter().any(|step| matches!(step, Step::OldSetAside)) {
        // Only what a promotion writes is removed: a note written beside the staged `.app` is
        // the user's (M5 gate review B9; AGENTS.md: never delete user files).
        let kept = match old_zip.and_then(|zip| foreign(&aside, "", &zip)) {
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
    recorded.map_err(|error| {
        format!(
            "the new candidate stands (its receipt is written), but the promotion-check record {PROMOTION_CHECK} is not: {error}; promote again on R before macos-ready"
        )
    })
}

/// A name in `dist/` for setting the old candidate aside that nothing holds yet: a staging kept
/// by an earlier run may hold the plain one, and a rename onto a directory could fail or merge
/// (fix-round review R1-11).
fn free_aside(root: &Path) -> Result<std::path::PathBuf, String> {
    let base = format!("dist/.exact-candidate-macos-old-{}", std::process::id());
    for attempt in 0..1000_u32 {
        let name = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}-{attempt}")
        };
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(path),
            Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
            Ok(_) => {}
        }
    }
    Err(format!(
        "every name {base}-N in dist/ is taken by kept stagings; move them away"
    ))
}

/// The ZIP name the old receipt records, or why the old staging's ZIP cannot be told apart.
fn old_zip(receipt: Option<&[u8]>) -> Result<String, String> {
    let receipt = receipt.ok_or("no old receipt names its ZIP")?;
    std::str::from_utf8(receipt)
        .ok()
        .and_then(|text| crate::release::json::parse(text).ok())
        .and_then(|document| {
            document
                .field("zip_name")
                .and_then(|name| name.string().map(str::to_owned))
                .ok()
        })
        .ok_or_else(|| "the old receipt names no ZIP".to_owned())
}

/// The first entry under `directory` (an old staging directory; `relative` is its path inside
/// it) that a promotion does not write, if any: a promotion writes the ZIP its receipt names,
/// the checksum file and the SBOM, and extracts the ZIP's inventory beside them (the files beside
/// the app, and the app directory with anything under it). Links count as foreign, so nothing
/// outside is looked at.
fn foreign(directory: &Path, relative: &str, zip: &str) -> Result<Option<String>, String> {
    let platform = crate::release::platform::Platform::MacOsArm64;
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
        // The bundle is the app's own, a directory; everything under it is what the ZIP held.
        if path == content::APP && !kind.is_dir() {
            return Ok(Some(path));
        }
        if path == content::APP || path.starts_with(&format!("{}/", content::APP)) {
            continue;
        }
        let promoted = if kind.is_dir() {
            content::is_inventory_directory(&path)
        } else if !kind.is_file() {
            false
        } else if relative.is_empty() {
            [platform.checksum_file(), platform.sbom_file(), zip].contains(&name)
        } else {
            content::FILES.iter().any(|(file, _)| *file == path)
        };
        if !promoted {
            return Ok(Some(path));
        }
        if kind.is_dir()
            && let Some(found) = foreign(&entry.path(), &path, zip)?
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

    /// The promotion-check record the new promotion writes in these samples.
    const NEW_CHECK: &str = "{\"schema_version\":1,\"kind\":\"macos-promotion-check\",\"release_source\":\"cccccccccccccccccccccccccccccccccccccccc\",\"candidate_receipt_sha256\":\"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\"}\n";

    /// A file every promotion writes, which tells the old candidate from the new one.
    const MARKER: &str = super::super::content::README;

    /// An old candidate, its receipt, its promotion-check record and maybe a reuse record.
    fn tree(root: &Path, candidate: &str, receipt: &str, record: Option<&str>) {
        fs::create_dir_all(root.join(STAGING).join("StickyMD")).unwrap();
        fs::write(root.join(STAGING).join(MARKER), candidate).unwrap();
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        fs::write(root.join(PROMOTION_CHECK), old_check()).unwrap();
        if let Some(record) = record {
            fs::write(root.join(REUSE_RECORD), record).unwrap();
        }
    }

    /// The candidate's marker, the receipt, the reuse record and the promotion-check record.
    fn read(root: &Path) -> (String, String, Option<String>, Option<String>) {
        (
            fs::read_to_string(root.join(STAGING).join(MARKER)).unwrap(),
            fs::read_to_string(root.join(RECEIPT)).unwrap(),
            fs::read_to_string(root.join(REUSE_RECORD)).ok(),
            fs::read_to_string(root.join(PROMOTION_CHECK)).ok(),
        )
    }

    fn outputs<'a>(reuse: Option<&'a str>) -> Outputs<'a> {
        Outputs {
            reuse,
            receipt: "new receipt",
            promotion: NEW_CHECK,
        }
    }

    /// A promotion-check record an earlier promotion wrote.
    fn old_check() -> String {
        crate::release::macos::promotion_check::render(&"a".repeat(40), &"b".repeat(64))
    }

    fn old() -> (String, String, Option<String>, Option<String>) {
        (
            "old".into(),
            "old receipt".into(),
            Some("old record".into()),
            Some(old_check()),
        )
    }

    #[test]
    fn install_swaps_the_candidate_and_writes_the_receipt_last() {
        let scratch = TemporaryDirectory::new("macos-install").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        // The old staging holds just what a promotion writes (of another version), so it goes.
        promoted(root, "0.2.0");
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        install(root, &staged, outputs(None), || Ok(())).unwrap();
        // Same-commit promotion: the old ancestor record must not survive beside the new receipt;
        // the promotion-check record is the new one.
        assert_eq!(
            read(root),
            (
                "new".into(),
                "new receipt".into(),
                None,
                Some(NEW_CHECK.into())
            )
        );
        assert!(!staged.exists());
        let leftovers = fs::read_dir(root.join("dist"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(".exact-candidate-macos"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    /// Fills the old staging with exactly what a promotion of `version` writes, and gives it a
    /// receipt naming that version's ZIP.
    fn promoted(root: &Path, version: &str) {
        let platform = crate::release::platform::Platform::MacOsArm64;
        let old = root.join(STAGING);
        for file in [
            platform.archive_name(version),
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
        fs::write(
            root.join(RECEIPT),
            format!(
                "{{\"schema_version\":1,\"zip_name\":\"{}\"}}\n",
                platform.archive_name(version)
            ),
        )
        .unwrap();
    }

    /// Fix-round review R1-3: only the one ZIP the old receipt names is the promotion's, and
    /// `StickyMD/StickyMD.app` must be a directory; another ZIP-like name, an app that is a
    /// plain file, or an old receipt that names no ZIP keeps the old staging.
    #[test]
    fn an_old_staging_is_kept_unless_the_old_receipt_names_its_zip() {
        let platform = crate::release::platform::Platform::MacOsArm64;
        for case in ["backup", "app-file", "no-receipt", "unreadable-receipt"] {
            let scratch = TemporaryDirectory::new("macos-install-own").unwrap();
            let root = scratch.path();
            tree(root, "old", "old receipt", None);
            promoted(root, "0.2.0");
            let old = root.join(STAGING);
            let kept = match case {
                "backup" => {
                    let backup = old.join("StickyMD-我的备份-macos-arm64-portable.zip");
                    fs::write(&backup, "mine").unwrap();
                    "StickyMD-我的备份-macos-arm64-portable.zip"
                }
                "app-file" => {
                    fs::remove_dir_all(old.join(content::APP)).unwrap();
                    fs::write(old.join(content::APP), "not a bundle").unwrap();
                    content::APP
                }
                "no-receipt" => {
                    fs::remove_file(root.join(RECEIPT)).unwrap();
                    ""
                }
                _ => {
                    fs::write(root.join(RECEIPT), "old receipt").unwrap();
                    ""
                }
            };
            let staged = root.join("dist/.new");
            fs::create_dir_all(staged.join("StickyMD")).unwrap();
            fs::write(staged.join(MARKER), "new").unwrap();
            install(root, &staged, outputs(None), || Ok(())).unwrap();
            assert_eq!(read(root).0, "new", "{case}");
            let [aside] = set_aside(root).try_into().unwrap_or_else(|left: Vec<_>| {
                panic!("{case}: the old staging is gone ({left:?})")
            });
            assert!(
                fs::symlink_metadata(aside.join(platform.archive_name("0.2.0"))).is_ok(),
                "{case}"
            );
            if !kept.is_empty() {
                assert!(fs::symlink_metadata(aside.join(kept)).is_ok(), "{case}");
            }
        }
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
        install(root, &staged, outputs(None), || Ok(())).unwrap();
        assert_eq!(read(root).0, "new");
        let [kept] = set_aside(root).try_into().unwrap();
        assert_eq!(
            fs::read_to_string(kept.join("StickyMD/note/note.md")).unwrap(),
            "the user's words"
        );
    }

    /// Fix-round review R1-11: a staging kept by an earlier run under this process's id does not
    /// stop a promotion, and only the directory this run set aside is cleaned up: the earlier
    /// one stays as it was, even when it holds only what a promotion writes.
    #[test]
    fn a_staging_kept_under_the_same_name_neither_blocks_nor_is_cleaned() {
        let scratch = TemporaryDirectory::new("macos-install-pid").unwrap();
        let root = scratch.path();
        let earlier = root.join(format!(
            "dist/.exact-candidate-macos-old-{}",
            std::process::id()
        ));
        fs::create_dir_all(earlier.join("StickyMD/note")).unwrap();
        fs::write(earlier.join("StickyMD/note/note.md"), "kept before").unwrap();
        tree(root, "old", "old receipt", None);
        promoted(root, "0.2.0");
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        install(root, &staged, outputs(None), || Ok(())).unwrap();
        assert_eq!(read(root).0, "new");
        // This run's old staging held only promoted files and is gone; the earlier one is not.
        assert_eq!(set_aside(root), vec![earlier.clone()]);
        assert_eq!(
            fs::read_to_string(earlier.join("StickyMD/note/note.md")).unwrap(),
            "kept before"
        );
        // With no old candidate to set aside, an earlier directory of promoted files is not this
        // run's either.
        fs::remove_dir_all(earlier.join("StickyMD/note")).unwrap();
        fs::remove_dir_all(root.join(STAGING)).unwrap();
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "newer").unwrap();
        install(root, &staged, outputs(None), || Ok(())).unwrap();
        assert_eq!(set_aside(root), vec![earlier]);
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
            outputs(Some("new record")),
            || Ok(()),
        )
        .unwrap_err();
        assert!(error.contains("into place"), "{error}");
        assert_eq!(read(root), old());
    }

    /// M5 gate review B2: the source identity is checked again just before the receipt is
    /// written; a checkout that stopped being a clean R leaves the old candidate, its receipt
    /// and both records as they were, and removes the prepared candidate.
    #[test]
    fn a_checkout_that_is_no_longer_r_stops_install_before_the_receipt() {
        let scratch = TemporaryDirectory::new("macos-install-identity").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        let error = install(root, &staged, outputs(Some("new record")), || {
            Err("macos-promote runs on a clean checkout of R".to_owned())
        })
        .unwrap_err();
        assert!(error.contains("clean checkout"), "{error}");
        assert_eq!(read(root), old());
        assert!(!staged.exists());
    }

    /// M5 gate review B2: the promotion-check record is written after the receipt; when it
    /// cannot be, the new candidate stands and the error says to promote again on R.
    #[test]
    fn a_promotion_check_that_cannot_be_written_leaves_the_new_candidate() {
        let scratch = TemporaryDirectory::new("macos-install-check").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", None);
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        // Something takes the record's place after the old record was retired.
        let error = install(root, &staged, outputs(None), || {
            fs::create_dir(root.join(PROMOTION_CHECK)).map_err(|error| error.to_string())
        })
        .unwrap_err();
        assert!(
            error.contains("the new candidate stands") && error.contains("promote again on R"),
            "{error}"
        );
        assert_eq!(read(root).0, "new");
        assert_eq!(read(root).1, "new receipt");
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
        assert!(install(root, &staged, outputs(None), || Ok(())).is_err());
        assert_eq!(
            fs::read_to_string(root.join(STAGING).join(MARKER)).unwrap(),
            "old"
        );
        assert!(staged.exists());
    }

    /// Fix-round review R1-9: a file at the promotion-check path that is not a promotion-check
    /// record (here a ledger written there by mistake) is kept, and the promotion fails before
    /// anything changes; a record that lost its final newline is still one and is retired.
    #[test]
    fn a_foreign_file_at_the_promotion_check_path_is_kept() {
        let scratch = TemporaryDirectory::new("macos-install-foreign-check").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", None);
        let ledger = "{\"kind\":\"automated\",\"row\":\"W02\"}\n";
        fs::write(root.join(PROMOTION_CHECK), ledger).unwrap();
        let staged = root.join("dist/.new");
        fs::create_dir_all(staged.join("StickyMD")).unwrap();
        fs::write(staged.join(MARKER), "new").unwrap();
        let error = install(root, &staged, outputs(None), || Ok(())).unwrap_err();
        assert!(
            error.contains(PROMOTION_CHECK) && error.contains("not a promotion-check record"),
            "{error}"
        );
        assert_eq!(read(root).0, "old");
        assert_eq!(read(root).3.as_deref(), Some(ledger));
        fs::write(root.join(PROMOTION_CHECK), old_check().trim_end()).unwrap();
        install(root, &staged, outputs(None), || Ok(())).unwrap();
        assert_eq!(read(root).3.as_deref(), Some(NEW_CHECK));
    }

    /// Fix-round review R3-5: a link at the receipt, reuse-record or promotion-check path, even
    /// a dangling one, or a directory there, is an error before anything changes; it is neither
    /// read through nor taken as absent.
    #[test]
    fn a_link_or_directory_at_an_evidence_path_stops_install_before_any_change() {
        for (path, kind) in [
            (REUSE_RECORD, "dangling"),
            (RECEIPT, "link"),
            (PROMOTION_CHECK, "link"),
            (REUSE_RECORD, "directory"),
        ] {
            let scratch = TemporaryDirectory::new("macos-install-link").unwrap();
            let root = scratch.path();
            tree(root, "old", "old receipt", None);
            let _ = fs::remove_file(root.join(path));
            match kind {
                "dangling" => {
                    std::os::unix::fs::symlink(root.join("gone.json"), root.join(path)).unwrap()
                }
                "link" => {
                    fs::write(root.join("elsewhere.json"), "old").unwrap();
                    std::os::unix::fs::symlink(root.join("elsewhere.json"), root.join(path))
                        .unwrap();
                }
                _ => fs::create_dir(root.join(path)).unwrap(),
            }
            let staged = root.join("dist/.new");
            fs::create_dir_all(staged.join("StickyMD")).unwrap();
            fs::write(staged.join(MARKER), "new").unwrap();
            let error = install(root, &staged, outputs(None), || Ok(())).unwrap_err();
            assert!(error.contains(path), "{path} {kind}: {error}");
            assert_eq!(read(root).0, "old", "{path} {kind}");
            assert!(
                fs::symlink_metadata(root.join(path)).is_ok(),
                "{path} {kind}"
            );
            assert!(staged.exists(), "{path} {kind}");
        }
    }
}
