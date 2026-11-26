//! The candidate as the readiness check reads it (plan 11 #macos-readiness-check, steps 3 and
//! 5): the resolver's verdict on the staging and its receipt, the promotion-check record on R,
//! and the whole state of both, which must be the same when the verdicts are printed.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::identity::{self, Identity};
use super::judge::{self, Read};
use crate::release::macos::promote::PromotionLock;
use crate::release::macos::promotion_check::{self, Check};
use std::path::Path;

/// One reading of the candidate.
pub(super) struct Snapshot {
    pub candidate: Read<judge::Candidate>,
    pub promotion: Read<Check>,
    /// The whole state read (absent, not resolving, or resolving to which receipt; and the
    /// record's bytes), compared between the first and the last reading.
    pub state: String,
}

/// The last reading (step 5), under the promotion lock's shared lock: the candidate's whole
/// state again, then the source identity; either differing from the start fails the check, so
/// no verdict is printed. The lock is returned to be held until the verdicts are printed, so no
/// promotion can swap in another candidate between this reading and the output (M5 gate review
/// B1). A promotion holding the lock fails the check; with no lock file no promotion ever ran
/// here, which stands only when there is no candidate either.
pub(super) fn confirm(
    root: &Path,
    release: &str,
    first: &Snapshot,
    start: &Identity,
) -> Result<Option<PromotionLock>, String> {
    let lock = PromotionLock::share(root)?;
    let last = read(root, release);
    if lock.is_none() && !matches!(last.candidate, Read::Missing(_)) {
        return Err(format!(
            "no promotion lock ({}) beside a candidate receipt: promote the candidate again on R",
            crate::release::macos::promote::PROMOTION_LOCK
        ));
    }
    if last.state != first.state {
        return Err("the candidate changed while it was checked; check again".to_owned());
    }
    if identity::capture(root)? != *start {
        return Err("the checkout changed while it was checked; check again".to_owned());
    }
    Ok(lock)
}

pub(super) fn read(root: &Path, release: &str) -> Snapshot {
    let candidate = match crate::release::macos::resolve::receipt_present(root) {
        Ok(true) => match crate::release::macos::resolve::resolve(root, release) {
            Ok(resolved) => Read::Present(judge::Candidate {
                receipt_sha256: resolved.receipt_sha256,
                zip_sha256: resolved.zip_sha256,
                built: crate::mac_ledger::record::Built {
                    executable: resolved.executable_sha256,
                    bundle: resolved.bundle_sha256,
                },
                signing: resolved.signing,
            }),
            Err(error) => Read::Invalid(format!("the candidate does not resolve: {error}")),
        },
        Ok(false) => Read::Missing("no macOS candidate has been promoted".to_owned()),
        Err(error) => Read::Invalid(error),
    };
    // Read after the receipt the resolver checked: a promotion writes the record after it.
    let found = promotion_check::load(root);
    let record = match &found {
        Ok(None) => "no record".to_owned(),
        Ok(Some(bytes)) => crate::integrity::sha256_bytes(bytes).map_or_else(
            |_| "record unhashable".to_owned(),
            |d| format!("record {d}"),
        ),
        Err(_) => "record unreadable".to_owned(),
    };
    let state = match &candidate {
        Read::Present(candidate) => format!("present {}", candidate.receipt_sha256),
        Read::Missing(_) => "missing".to_owned(),
        Read::Invalid(_) => "invalid".to_owned(),
    };
    Snapshot {
        candidate,
        promotion: judge::read_promotion(found),
        state: format!("{state}; {record}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::macos::promote::tests::scratch_git;
    use crate::release::temporary::TemporaryDirectory;
    use std::fs;

    const R: &str = "1111111111111111111111111111111111111111";

    /// A clean checkout that ignores `dist/`, with no candidate and no promotion lock yet.
    fn checkout() -> TemporaryDirectory {
        let scratch = TemporaryDirectory::new("macos-ready-confirm").unwrap();
        let root = scratch.path();
        scratch_git(root, &["init", "-q", "-b", "main"]);
        fs::write(root.join(".gitignore"), "/dist/\n").unwrap();
        scratch_git(root, &["add", "-A"]);
        scratch_git(root, &["commit", "-q", "-m", "r"]);
        scratch
    }

    /// M5 gate review B1: while a promotion holds its lock, the last reading fails instead of
    /// printing verdicts for a candidate that may be swapped meanwhile.
    #[test]
    fn a_promotion_in_progress_fails_the_last_reading() {
        let scratch = checkout();
        let root = scratch.path();
        let start = identity::capture(root).unwrap();
        let first = read(root, R);
        let promotion = PromotionLock::take(root).unwrap();
        let error = confirm(root, R, &first, &start).unwrap_err();
        assert!(
            error.contains("a macOS promotion is in progress"),
            "{error}"
        );
        drop(promotion);
        // Other tests spawn processes in parallel; a child forked while the lock was held shares
        // it until it execs, so the release can take a moment to show.
        let confirmed = (0..200).any(|_| {
            confirm(root, R, &first, &start).is_ok() || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                false
            }
        });
        assert!(confirmed, "a free lock lets the reading stand");
        // The lock file is opened read-only, never created or changed.
        let lock = root.join(crate::release::macos::promote::PROMOTION_LOCK);
        assert_eq!(fs::read(&lock).unwrap(), b"");
    }

    /// Without a lock file no promotion ever ran, which holds only when there is no candidate
    /// either; a receipt without the lock fails the reading. The lock is not created.
    #[test]
    fn no_promotion_lock_stands_only_without_a_candidate() {
        let scratch = checkout();
        let root = scratch.path();
        let start = identity::capture(root).unwrap();
        let first = read(root, R);
        confirm(root, R, &first, &start).unwrap();
        let lock = root.join(crate::release::macos::promote::PROMOTION_LOCK);
        assert!(!lock.exists());
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(
            root.join(crate::release::macos::promote::RECEIPT),
            "not a receipt",
        )
        .unwrap();
        let first = read(root, R);
        let error = confirm(root, R, &first, &start).unwrap_err();
        assert!(error.contains("no promotion lock"), "{error}");
        assert!(!lock.exists());
    }

    /// The candidate is read again before the source identity, and a change in either fails the
    /// reading.
    #[test]
    fn a_changed_candidate_or_checkout_fails_the_last_reading() {
        let scratch = checkout();
        let root = scratch.path();
        PromotionLock::take(root).map(drop).unwrap();
        let start = identity::capture(root).unwrap();
        let first = read(root, R);
        fs::write(
            root.join(promotion_check::PATH),
            promotion_check::render(R, &"c".repeat(64)),
        )
        .unwrap();
        let error = confirm(root, R, &first, &start).unwrap_err();
        assert!(error.contains("the candidate changed"), "{error}");
        let first = read(root, R);
        fs::write(root.join("new.txt"), "x").unwrap();
        let error = confirm(root, R, &first, &start).unwrap_err();
        assert!(error.contains("the checkout changed"), "{error}");
    }
}
