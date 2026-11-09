//! The shared I/O worker over the real Windows adapters.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use std::fs;
use std::path::Path;
use std::sync::{Arc, Barrier, mpsc};
use std::time::Duration;

use std::collections::HashMap;
use stickymd_app::execution::{IoCompletion, IoPorts, PersistenceWorker};
use stickymd_app::ports::{
    AssetReconcileMode, AssetSyncRequest, NoteReceipt, PersistMode, PersistRequest,
};
use stickymd_core::{AssetEffect, Generation, LineEnding, ManagedAssetLocation, hash_bytes};

use super::{WinAssetStore, WinConfigStorage, WinExportSink, WinNoteStorage};
use crate::assets::AssetStorage;
use crate::test_support::unique_temp_path;

type WinPorts = IoPorts<WinNoteStorage, WinAssetStore, WinExportSink, WinConfigStorage>;

fn ports_in(note_dir: &Path) -> WinPorts {
    IoPorts {
        note: WinNoteStorage {
            note_file: note_dir.join("note.md"),
            note_tmp: note_dir.join("note.md.tmp"),
            note_dir: note_dir.to_path_buf(),
        },
        assets: WinAssetStore {
            images: note_dir.join("images"),
            trash: note_dir.join(".trash"),
            note_file: note_dir.join("note.md"),
        },
        export: WinExportSink {
            note_dir: note_dir.to_path_buf(),
        },
        config: WinConfigStorage {
            target: note_dir.join("config.toml"),
            temporary: note_dir.join("config.toml.tmp"),
        },
    }
}

#[test]
fn next_note_waits_for_ack_and_stale_base_request_is_discarded() {
    let root = unique_temp_path("worker-barrier");
    fs::create_dir(&root).unwrap();
    let target = root.join("note.md");
    fs::write(&target, b"base").unwrap();

    let (sender, receiver) = mpsc::channel();
    let worker = PersistenceWorker::start(ports_in(&root), move |completion| {
        sender.send(completion).unwrap();
    })
    .unwrap();
    let request = |text: &'static str| PersistRequest {
        generation: Generation::initial(),
        text: text.into(),
        line_ending: LineEnding::Lf,
        mode: PersistMode::Guarded {
            expected: Some(hash_bytes(b"base")),
        },
    };
    worker.submit_note(request("first"));
    let IoCompletion::Note(NoteReceipt::Save { mode, .. }) =
        receiver.recv_timeout(Duration::from_secs(3)).unwrap()
    else {
        panic!("expected a note completion");
    };
    assert_eq!(
        mode,
        PersistMode::Guarded {
            expected: Some(hash_bytes(b"base"))
        }
    );

    worker.submit_note(request("stale-second"));
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    worker.acknowledge_note_completion();
    assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"first");

    drop(worker);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn newer_asset_convergence_runs_after_an_older_sync_has_left_the_mailbox() {
    use stickymd_render::image::prepare_rgba_image;

    let root = unique_temp_path("worker-asset-order");
    let images = root.join("images");
    let trash = root.join(".trash");
    fs::create_dir_all(&images).unwrap();
    fs::create_dir(&trash).unwrap();
    let storage = AssetStorage::open(&images, &trash).unwrap();
    let asset = storage
        .store(&prepare_rgba_image(2, 2, vec![23; 16]).unwrap())
        .unwrap();

    let (sender, receiver) = mpsc::channel();
    let first_receipt_gate = Arc::new(Barrier::new(2));
    let callback_gate = Arc::clone(&first_receipt_gate);
    let worker = PersistenceWorker::start(ports_in(&root), move |completion| {
        let block = matches!(completion, IoCompletion::AssetSync { request_id: 1, .. });
        sender.send(completion).unwrap();
        if block {
            callback_gate.wait();
        }
    })
    .unwrap();

    worker.submit_asset_sync(AssetSyncRequest {
        request_id: 1,
        generation: Generation::initial(),
        effects: vec![AssetEffect {
            name: asset.name.clone(),
            from: ManagedAssetLocation::Images,
            to: ManagedAssetLocation::Trash,
        }],
        references: HashMap::new(),
        reconcile: Some(AssetReconcileMode::Runtime),
        safe_expected: None,
    });
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        IoCompletion::AssetSync {
            request_id: 1,
            result: Ok(_),
            ..
        }
    ));
    assert!(trash.join(asset.name.as_str()).is_file());

    let mut latest_references = HashMap::new();
    latest_references.insert(asset.name.clone(), 1);
    worker.submit_asset_sync(AssetSyncRequest {
        request_id: 2,
        generation: Generation::initial().checked_next().unwrap(),
        effects: Vec::new(),
        references: latest_references,
        reconcile: Some(AssetReconcileMode::Runtime),
        safe_expected: None,
    });
    first_receipt_gate.wait();
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
        IoCompletion::AssetSync {
            request_id: 2,
            result: Ok(_),
            ..
        }
    ));
    assert!(images.join(asset.name.as_str()).is_file());
    assert!(!trash.join(asset.name.as_str()).exists());

    drop(worker);
    fs::remove_dir_all(root).unwrap();
}
