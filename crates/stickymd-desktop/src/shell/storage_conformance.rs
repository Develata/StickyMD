//! The rows of `phase15_receipt_tests` that read or write the note's files, as functions over any
//! storage. A Platform Host proves its storage behind the real shell by expanding
//! `stickymd_desktop::testing::storage_conformance!` with a factory for it in its own tests: one
//! `#[test]` per row, named as the row. The assertions are the receipt table's; only the storage
//! changes.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use stickymd_app::execution::{IoCompletion, PersistenceWorker};
use stickymd_app::ports::{NoteReceipt, PersistMode, PreserveOutcome, TemporaryCleanup};
use stickymd_core::{ExternalFileState, PreserveExpectation, hash_bytes};

use super::headless_tests::{
    HeadlessApp, external, offer_recovery, receipt_fixture_on, saves, type_letter,
};

/// Where a storage keeps the note.
pub struct StoragePaths {
    pub note_file: PathBuf,
    pub note_tmp: PathBuf,
    pub note_dir: PathBuf,
}

/// A worker over the storage under test, and where that storage keeps the note.
pub struct StorageSite {
    pub worker: PersistenceWorker,
    pub paths: StoragePaths,
}

/// What the worker calls with each receipt.
pub type Completion = Box<dyn Fn(IoCompletion) + Send + Sync>;

/// Builds the storage in `root`, an empty directory the row owns and removes afterwards, and
/// starts a worker over it.
pub type StorageFactory = fn(&Path, Completion) -> StorageSite;

/// One `#[test]` per row, each running the row on the storage `$factory` builds.
#[macro_export]
macro_rules! storage_conformance {
    ($factory:path) => {
        $crate::storage_conformance!(
            @rows $factory;
            restoring_publishes_the_temporary_and_then_removes_the_evidence,
            restoring_over_an_invalid_canonical_moves_it_aside_first,
            using_the_canonical_removes_the_temporary_without_a_save,
            using_a_missing_canonical_publishes_it_before_removing_the_temporary,
            keeping_local_forces_the_save_and_refuses_a_second_choice,
        );
    };
    (@rows $factory:path; $($row:ident,)*) => {
        $(
            #[test]
            fn $row() {
                $crate::shell::storage_conformance::$row($factory);
            }
        )*
    };
}

pub(super) const GUARDED: PersistMode = PersistMode::Guarded { expected: None };
pub(super) const FORCED: PersistMode = PersistMode::ForceOverwrite;
const RECOVERY_DONE: &str = "恢复选择已完成；正在核对受管图片后启用编辑。";
const RECEIPT_WAIT: Duration = Duration::from_secs(10);

/// What a row pins: the status line, the saves this step handed to the worker, and the facts the
/// title and the window guards read.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Outcome {
    pub(super) diagnostic: Option<String>,
    pub(super) saves: u64,
    pub(super) recovery_pending: bool,
    pub(super) operation_pending: bool,
    pub(super) conflict: bool,
    pub(super) dirty: bool,
    pub(super) keep_local: bool,
}

pub(super) fn settled(diagnostic: impl Into<String>) -> Outcome {
    Outcome {
        diagnostic: Some(diagnostic.into()),
        saves: 0,
        recovery_pending: false,
        operation_pending: false,
        conflict: false,
        dirty: false,
        keep_local: false,
    }
}

pub(super) fn saving(app: &HeadlessApp) -> String {
    format!(
        "正在保存 generation {}…",
        app.app.coordinator.view().generation.value()
    )
}

fn step(app: &mut HeadlessApp, action: impl FnOnce(&mut HeadlessApp)) -> Outcome {
    let before = saves(app);
    action(app);
    Outcome {
        diagnostic: app.diagnostic.clone(),
        saves: saves(app) - before,
        recovery_pending: app.app.recovery.is_pending(),
        operation_pending: app.app.recovery.operation_pending(),
        conflict: app.app.persistence.conflict().is_some(),
        dirty: app.app.coordinator.view().dirty,
        keep_local: app.app.persistence.keep_local_in_progress(),
    }
}

pub(super) fn deliver(app: &mut HeadlessApp, receipt: IoCompletion) -> Outcome {
    step(app, |app| app.handle_io_completion(receipt))
}

pub(super) fn press(app: &mut HeadlessApp, primary: bool) -> Outcome {
    step(app, |app| assert!(app.handle_resolution_key(primary)))
}

pub(super) fn observed(app: &HeadlessApp, state: ExternalFileState) -> IoCompletion {
    IoCompletion::Note(NoteReceipt::External {
        requested_base: app.app.coordinator.view().base_disk_hash,
        result: Ok(state),
    })
}

pub(super) fn enter_conflict(app: &mut HeadlessApp) {
    let receipt = observed(app, ExternalFileState::Present(external("outside")));
    app.handle_io_completion(receipt);
    assert!(app.app.persistence.conflict().is_some());
}

/// The next note receipt; asset convergence runs beside these flows and is not pinned here.
pub(super) fn next(receipts: &Receiver<IoCompletion>) -> IoCompletion {
    loop {
        match receipts
            .recv_timeout(RECEIPT_WAIT)
            .expect("a worker receipt")
        {
            IoCompletion::AssetSync { .. } => {}
            receipt => return receipt,
        }
    }
}

pub(super) fn saved_mode(receipt: &IoCompletion) -> PersistMode {
    match receipt {
        IoCompletion::Note(NoteReceipt::Save { mode, .. }) => *mode,
        other => panic!("expected a save receipt, got {other:?}"),
    }
}

fn is_recovery_cleanup(receipt: &IoCompletion, expected: u64) -> bool {
    matches!(
        receipt,
        IoCompletion::Note(NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation },
            result: Ok(()),
        }) if *operation == expected
    )
}

pub fn restoring_publishes_the_temporary_and_then_removes_the_evidence(storage: StorageFactory) {
    let (mut app, receipts) = receipt_fixture_on(storage, "disk");
    fs::write(&app.paths.note_file, "disk").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, Some("disk"), None);
    let outcome = press(&mut app, true);
    assert_eq!(app.app.coordinator.view().text, "temporary");
    assert_eq!(outcome.saves, 1);
    assert!(outcome.operation_pending);
    let operation = app.app.recovery.operation_id().unwrap();
    let save = next(&receipts);
    assert_eq!(
        saved_mode(&save),
        PersistMode::Guarded {
            expected: Some(hash_bytes(b"disk"))
        }
    );
    let diagnostic = app.diagnostic.clone();
    let outcome = deliver(&mut app, save);
    assert_eq!(outcome.diagnostic, diagnostic);
    assert!(outcome.recovery_pending);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"temporary");
    assert!(!app.paths.note_tmp.exists());
}

pub fn restoring_over_an_invalid_canonical_moves_it_aside_first(storage: StorageFactory) {
    let (mut app, receipts) = receipt_fixture_on(storage, "");
    fs::write(&app.paths.note_file, b"\xff").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    let expected = PreserveExpectation::Content(hash_bytes(b"\xff"));
    offer_recovery(&mut app, None, Some(expected));
    let outcome = press(&mut app, true);
    assert_eq!(outcome.saves, 0);
    let operation = app.app.recovery.operation_id().unwrap();
    let preserved = next(&receipts);
    assert!(
        matches!(
            &preserved,
            IoCompletion::Note(NoteReceipt::CanonicalPreserved {
                operation: seen,
                result: Ok(PreserveOutcome::Preserved),
            }) if *seen == operation
        ),
        "{preserved:?}"
    );
    assert_eq!(deliver(&mut app, preserved).saves, 1);
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), GUARDED);
    app.handle_io_completion(save);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"temporary");
    let moved: Vec<_> = fs::read_dir(&app.paths.note_dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("note.invalid-")
        })
        .collect();
    assert_eq!(moved.len(), 1);
    assert_eq!(fs::read(moved[0].path()).unwrap(), b"\xff");
}

pub fn using_the_canonical_removes_the_temporary_without_a_save(storage: StorageFactory) {
    let (mut app, receipts) = receipt_fixture_on(storage, "local");
    fs::write(&app.paths.note_file, "disk").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, Some("disk"), None);
    assert_eq!(
        press(&mut app, false),
        Outcome {
            recovery_pending: true,
            operation_pending: true,
            ..settled("正在确认使用当前 note.md…")
        }
    );
    assert_eq!(app.app.coordinator.view().text, "disk");
    let operation = app.app.recovery.operation_id().unwrap();
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"disk");
    assert!(!app.paths.note_tmp.exists());
}

/// The temporary is the only durable evidence until an empty canonical note exists.
pub fn using_a_missing_canonical_publishes_it_before_removing_the_temporary(
    storage: StorageFactory,
) {
    let (mut app, receipts) = receipt_fixture_on(storage, "local");
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, None, None);
    assert_eq!(press(&mut app, false).saves, 1);
    assert_eq!(app.app.coordinator.view().text, "");
    let operation = app.app.recovery.operation_id().unwrap();
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), GUARDED);
    app.handle_io_completion(save);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"");
    assert!(!app.paths.note_tmp.exists());
}

pub fn keeping_local_forces_the_save_and_refuses_a_second_choice(storage: StorageFactory) {
    let (mut app, receipts) = receipt_fixture_on(storage, "start");
    fs::write(&app.paths.note_file, "outside").unwrap();
    type_letter(&mut app);
    enter_conflict(&mut app);
    let expected = saving(&app);
    assert_eq!(
        press(&mut app, false),
        Outcome {
            saves: 1,
            conflict: true,
            dirty: true,
            keep_local: true,
            ..settled(expected)
        }
    );
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), FORCED);
    assert_eq!(
        press(&mut app, true),
        Outcome {
            conflict: true,
            dirty: true,
            keep_local: true,
            ..settled("正在保存本地内容，请稍候。")
        }
    );
    assert_eq!(
        deliver(&mut app, save),
        settled("已保留本地内容并覆盖外部文件。")
    );
    assert_eq!(
        fs::read(&app.paths.note_file).unwrap(),
        app.app.coordinator.view().text.as_bytes()
    );
    assert!(receipts.recv_timeout(Duration::from_millis(200)).is_err());
}
