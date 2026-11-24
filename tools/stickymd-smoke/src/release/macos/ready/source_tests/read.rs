//! Reading the source-test receipt for the gate (plan 11 #macos-readiness-check): only under the
//! run lock, never without it (not even its bytes), and only the receipt of the run the run record
//! names, so a receipt left by an earlier run, or restored from a copy, does not count after a run
//! that was interrupted (review round 3, B5; review round 4, BLOCKER 2).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::super::judge::Read;
use super::{RECEIPT, RUN, Receipt, parse, refuse_link};
use std::io::ErrorKind;
use std::path::Path;

const RERUN: &str = "run release macos-source-tests on R again";

/// The receipt under `root`, read under the run lock when `held`.
pub(in super::super) fn read_receipt(root: &Path, held: bool) -> Result<Read<Receipt>, String> {
    let path = root.join(RECEIPT);
    if !refuse_link(&path)? {
        return Ok(Read::Missing(
            "no source-test receipt: run release macos-source-tests on R".to_owned(),
        ));
    }
    if !held {
        return Ok(Read::Invalid(format!(
            "the source-test receipt has no run lock beside it; {RERUN}"
        )));
    }
    let marker = root.join(RUN);
    if !refuse_link(&marker)? {
        return Ok(Read::Invalid(format!(
            "no record names the run that wrote the source-test receipt; {RERUN}"
        )));
    }
    let run = match std::fs::read_to_string(&marker) {
        Ok(text) => text.trim().to_owned(),
        Err(error) => {
            return Ok(Read::Invalid(format!(
                "cannot read the source-test run record: {error}"
            )));
        }
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(Read::Missing(
                "no source-test receipt: run release macos-source-tests on R".to_owned(),
            ));
        }
        Err(error) => {
            return Ok(Read::Invalid(format!(
                "cannot read the source-test receipt: {error}"
            )));
        }
    };
    Ok(match parse(&text) {
        Ok(receipt) if receipt.run == run => Read::Present(receipt),
        Ok(receipt) => Read::Invalid(format!(
            "the source-test receipt is of run {}, but the last run to start was {run} (it was interrupted, or the receipt was restored); {RERUN}",
            receipt.run
        )),
        Err(error) => Read::Invalid(format!("the source-test receipt is invalid: {error}")),
    })
}

#[cfg(test)]
mod tests {
    use super::super::{RECEIPT, RUN, Reading, fixture, lock_for_reading, lock_for_run, render};
    use super::{Read, read_receipt};

    const R: &str = "0123456789abcdef0123456789abcdef01234567";

    fn receipt_of(run: &str) -> String {
        let mut receipt = fixture(R, Vec::new());
        receipt.run = run.to_owned();
        render(&receipt)
    }

    /// Waits out a child another test forked while the lock was held: it shares the lock until
    /// its exec closes it (as in the ledger's writer-lock test).
    fn held(root: &std::path::Path) -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            match lock_for_reading(root) {
                Ok(Reading::Free { held, .. }) => return held,
                _ if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => panic!("the run lock was not released: {:?}", other.is_ok()),
            }
        }
    }

    /// Review round 3, B5: no lock file means no run ever held it, and then a receipt found
    /// there is not consumed; a lock another holder keeps is busy.
    #[test]
    fn a_receipt_is_read_only_under_its_lock() {
        let scratch = stickymd_package::temporary::TemporaryDirectory::new("receipt-lock").unwrap();
        let root = scratch.path();
        std::fs::create_dir_all(root.join(RECEIPT).parent().unwrap()).unwrap();
        assert!(matches!(read_receipt(root, false), Ok(Read::Missing(_))));
        std::fs::write(root.join(RECEIPT), receipt_of("3-3")).unwrap();
        assert!(
            matches!(read_receipt(root, false), Ok(Read::Invalid(reason)) if reason.contains("no run lock"))
        );
        assert!(!held(root));
        let run = lock_for_run(root).unwrap().unwrap();
        assert!(matches!(lock_for_reading(root), Ok(Reading::Busy)));
        drop(run);
        assert!(held(root));
    }

    /// Review round 4, BLOCKER 2: a receipt restored without its lock, then a rerun interrupted
    /// after it recorded itself but before it retired the old receipt: the lock now exists, and
    /// the old receipt still does not count. The run that finishes does.
    #[test]
    fn a_receipt_counts_only_for_the_run_the_record_names() {
        let scratch = stickymd_package::temporary::TemporaryDirectory::new("receipt-run").unwrap();
        let root = scratch.path();
        std::fs::create_dir_all(root.join(RECEIPT).parent().unwrap()).unwrap();
        std::fs::write(root.join(RECEIPT), receipt_of("1-1")).unwrap();
        // The interrupted rerun: it takes the lock and records itself, then stops.
        let run = lock_for_run(root).unwrap().unwrap();
        std::fs::write(root.join(RUN), "2-2\n").unwrap();
        drop(run);
        assert!(held(root));
        assert!(
            matches!(read_receipt(root, true), Ok(Read::Invalid(reason)) if reason.contains("interrupted"))
        );
        // No record at all beside a held lock.
        std::fs::remove_file(root.join(RUN)).unwrap();
        assert!(
            matches!(read_receipt(root, true), Ok(Read::Invalid(reason)) if reason.contains("no record"))
        );
        // The rerun that finishes writes its own receipt.
        std::fs::write(root.join(RUN), "2-2\n").unwrap();
        std::fs::write(root.join(RECEIPT), receipt_of("2-2")).unwrap();
        assert!(matches!(read_receipt(root, true), Ok(Read::Present(_))));
    }

    /// Fix round, BLOCKER 2: an old record and its matching receipt, found without a lock; a rerun
    /// creates the lock and stops before it records itself. The new lock does not make the old
    /// pair count: the record went before the lock came.
    #[test]
    fn a_new_lock_does_not_revive_an_old_pair() {
        let scratch =
            stickymd_package::temporary::TemporaryDirectory::new("receipt-revive").unwrap();
        let root = scratch.path();
        std::fs::create_dir_all(root.join(RECEIPT).parent().unwrap()).unwrap();
        std::fs::write(root.join(RUN), "1-1\n").unwrap();
        std::fs::write(root.join(RECEIPT), receipt_of("1-1")).unwrap();
        assert!(
            matches!(read_receipt(root, false), Ok(Read::Invalid(reason)) if reason.contains("no run lock"))
        );
        let run = lock_for_run(root).unwrap().unwrap();
        drop(run);
        assert!(held(root));
        assert!(
            matches!(read_receipt(root, true), Ok(Read::Invalid(reason)) if reason.contains("no record"))
        );
    }

    /// Review round 4: an error inspecting the path, other than its absence, stops the read.
    #[test]
    fn an_uninspectable_path_is_an_error() {
        let scratch = stickymd_package::temporary::TemporaryDirectory::new("receipt-path").unwrap();
        std::fs::write(scratch.path().join("file"), "").unwrap();
        let error =
            super::super::refuse_link(&scratch.path().join("file/receipt.json")).unwrap_err();
        assert!(error.contains("cannot inspect"), "{error}");
    }
}
