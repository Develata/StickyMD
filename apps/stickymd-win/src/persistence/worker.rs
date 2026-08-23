//! Single bounded I/O worker with latest-note coalescing.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use stickymd_core::{AssetEffect, ExternalFileState, Generation, ManagedAssetName};

use crate::assets::{
    AssetPasteCompletion, AssetPasteError, AssetPasteFailure, AssetPasteRequest,
    AssetReconcileMode, AssetReconcileReport, AssetStorage, AssetStorageError,
    prepare_and_store_paste, reconcile_safe_boundary,
};
use crate::config::{ConfigPersistRequest, ConfigRevision, ConfigStorageError, save_config};
use crate::export::{ExportCompletion, ExportError, ExportRequest, export_snapshot};

use super::NoteStorageError;
use super::{
    PersistRequest, PersistResult, inspect_note_state_with_retry, persist_note, preserve_canonical,
    remove_temporary,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporaryCleanup {
    RecoveryResolved,
    ConflictDiscarded,
}

#[derive(Debug)]
pub enum IoCompletion {
    Note(NoteCompletion),
    External(Result<ExternalFileState, NoteStorageError>),
    Config {
        revision: ConfigRevision,
        result: Result<(), ConfigStorageError>,
    },
    TemporaryRemoved {
        purpose: TemporaryCleanup,
        result: Result<(), NoteStorageError>,
    },
    CanonicalPreserved(Result<(), NoteStorageError>),
    AssetPaste(Result<AssetPasteCompletion, AssetPasteFailure>),
    AssetSync {
        request_id: u64,
        generation: Generation,
        result: Result<AssetReconcileReport, AssetStorageError>,
    },
    Export {
        generation: Generation,
        result: Result<ExportCompletion, ExportError>,
    },
    WorkerStopped,
}

#[derive(Debug)]
pub struct NoteCompletion {
    pub generation: Generation,
    pub result: Result<PersistResult, NoteStorageError>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerMetrics {
    pub note_submitted: u64,
    pub note_started: u64,
    pub note_completed: u64,
    pub note_coalesced: u64,
    pub external_checks: u64,
}

struct NoteJob {
    target: PathBuf,
    temporary: PathBuf,
    request: PersistRequest,
}

struct ConfigJob {
    target: PathBuf,
    temporary: PathBuf,
    request: ConfigPersistRequest,
}

struct AssetRoots {
    images: PathBuf,
    trash: PathBuf,
}
struct AssetPasteJob {
    roots: AssetRoots,
    request: AssetPasteRequest,
}
pub(crate) struct AssetSyncRequest {
    pub images: PathBuf,
    pub trash: PathBuf,
    pub request_id: u64,
    pub generation: Generation,
    pub effects: Vec<AssetEffect>,
    pub references: HashMap<ManagedAssetName, usize>,
    pub reconcile: Option<AssetReconcileMode>,
    pub safe_note: Option<(PathBuf, Option<stickymd_core::Hash32>)>,
}
struct AssetSyncJob {
    roots: AssetRoots,
    request_id: u64,
    generation: Generation,
    effects: Vec<AssetEffect>,
    references: HashMap<ManagedAssetName, usize>,
    reconcile: Option<AssetReconcileMode>,
    safe_note: Option<(PathBuf, Option<stickymd_core::Hash32>)>,
}
#[derive(Default)]
struct Mailbox {
    note: Option<NoteJob>,
    external: Option<PathBuf>,
    config: Option<ConfigJob>,
    cleanup_temporary: Option<(PathBuf, TemporaryCleanup)>,
    preserve_canonical: Option<(PathBuf, PathBuf)>,
    asset_paste: Option<AssetPasteJob>,
    asset_sync: Option<AssetSyncJob>,
    export: Option<ExportRequest>,
    shutdown: bool,
    note_blocked_for_ack: bool,
    metrics: WorkerMetrics,
}

impl Mailbox {
    fn replace_note(&mut self, job: NoteJob) {
        self.metrics.note_submitted += 1;
        if self.note.replace(job).is_some() {
            self.metrics.note_coalesced += 1;
        }
    }

    fn replace_asset_sync(&mut self, mut job: AssetSyncJob) {
        if let Some(previous) = self.asset_sync.take() {
            // A single mailbox slot must not lose an earlier logical move when
            // a later text edit has no asset transition of its own. Collapse
            // every touched name to the newest canonical reference state.
            let mut touched = BTreeSet::new();
            touched.extend(previous.effects.into_iter().map(|effect| effect.name));
            touched.extend(job.effects.into_iter().map(|effect| effect.name));
            job.effects = touched
                .into_iter()
                .map(|name| {
                    let referenced = job.references.get(&name).copied().unwrap_or(0) > 0;
                    let (from, to) = if referenced {
                        (
                            stickymd_core::ManagedAssetLocation::Trash,
                            stickymd_core::ManagedAssetLocation::Images,
                        )
                    } else {
                        (
                            stickymd_core::ManagedAssetLocation::Images,
                            stickymd_core::ManagedAssetLocation::Trash,
                        )
                    };
                    AssetEffect { name, from, to }
                })
                .collect();
            // A later non-boundary request invalidates an older destructive
            // boundary. Downgrade it to runtime convergence; only the newest
            // request may explicitly authorize physical deletion.
            job.reconcile = match (job.reconcile, previous.reconcile) {
                (Some(current), _) => Some(current),
                (None, Some(_)) => Some(AssetReconcileMode::Runtime),
                (None, None) => None,
            };
            if job.reconcile != Some(AssetReconcileMode::SafeBoundary) {
                job.safe_note = None;
            }
        }
        self.asset_sync = Some(job);
    }

    fn has_work(&self) -> bool {
        self.shutdown
            || (!self.note_blocked_for_ack && self.note.is_some())
            || self.external.is_some()
            || self.cleanup_temporary.is_some()
            || self.preserve_canonical.is_some()
            || self.asset_paste.is_some()
            || self.asset_sync.is_some()
            || self.export.is_some()
            || self.config.is_some()
    }
}

pub struct PersistenceWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl PersistenceWorker {
    pub fn start<F>(on_completion: F) -> Result<Self, std::io::Error>
    where
        F: Fn(IoCompletion) + Send + Sync + 'static,
    {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let on_completion = Arc::new(on_completion);
        let thread = thread::Builder::new()
            .name("stickymd-io".into())
            .stack_size(512 * 1024)
            .spawn(move || run_worker(worker_shared, on_completion))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn submit_note(&self, target: PathBuf, temporary: PathBuf, request: PersistRequest) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.replace_note(NoteJob {
                target,
                temporary,
                request,
            });
            ready.notify_one();
        }
    }

    /// Release the one-note completion barrier. Any request queued against the
    /// pre-completion base is discarded; the coordinator will resubmit its
    /// latest snapshot after applying the durable receipt.
    pub fn acknowledge_note_completion(&self) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            if mailbox.note.take().is_some() {
                mailbox.metrics.note_coalesced += 1;
            }
            mailbox.note_blocked_for_ack = false;
            ready.notify_one();
        }
    }

    pub fn inspect_external(&self, target: PathBuf) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.external = Some(target);
            ready.notify_one();
        }
    }

    pub fn submit_config(
        &self,
        target: PathBuf,
        temporary: PathBuf,
        request: ConfigPersistRequest,
    ) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.config = Some(ConfigJob {
                target,
                temporary,
                request,
            });
            ready.notify_one();
        }
    }

    pub fn remove_temporary(&self, path: PathBuf, purpose: TemporaryCleanup) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.cleanup_temporary = Some((path, purpose));
            ready.notify_one();
        }
    }

    pub fn preserve_canonical(&self, source: PathBuf, destination: PathBuf) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.preserve_canonical = Some((source, destination));
            ready.notify_one();
        }
    }

    pub fn submit_asset_paste(
        &self,
        images: PathBuf,
        trash: PathBuf,
        request: AssetPasteRequest,
    ) -> bool {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            if mailbox.asset_paste.is_some() {
                return false;
            }
            mailbox.asset_paste = Some(AssetPasteJob {
                roots: AssetRoots { images, trash },
                request,
            });
            ready.notify_one();
            true
        } else {
            false
        }
    }

    /// Keep only the latest desired asset state. Intermediate file moves are
    /// observationally irrelevant; reconciliation proves ownership and brings
    /// disk to the newest DocumentState reference counts.
    pub fn submit_asset_sync(&self, request: AssetSyncRequest) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.replace_asset_sync(AssetSyncJob {
                roots: AssetRoots {
                    images: request.images,
                    trash: request.trash,
                },
                request_id: request.request_id,
                generation: request.generation,
                effects: request.effects,
                references: request.references,
                reconcile: request.reconcile,
                safe_note: request.safe_note,
            });
            ready.notify_one();
        }
    }

    pub fn submit_export(&self, request: ExportRequest) -> bool {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            if mailbox.export.is_some() {
                return false;
            }
            mailbox.export = Some(request);
            ready.notify_one();
            true
        } else {
            false
        }
    }

    #[cfg(debug_assertions)]
    pub fn metrics(&self) -> WorkerMetrics {
        let (lock, _) = &*self.shared;
        lock.lock()
            .map_or_else(|_| WorkerMetrics::default(), |mailbox| mailbox.metrics)
    }
}

impl Drop for PersistenceWorker {
    fn drop(&mut self) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.shutdown = true;
            mailbox.note = None;
            mailbox.note_blocked_for_ack = false;
            ready.notify_one();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run_worker<F>(shared: Arc<(Mutex<Mailbox>, Condvar)>, on_completion: Arc<F>)
where
    F: Fn(IoCompletion),
{
    loop {
        let work = {
            let (lock, ready) = &*shared;
            let mut mailbox = match lock.lock() {
                Ok(mailbox) => mailbox,
                Err(_) => {
                    on_completion(IoCompletion::WorkerStopped);
                    return;
                }
            };
            while !mailbox.has_work() {
                mailbox = match ready.wait(mailbox) {
                    Ok(mailbox) => mailbox,
                    Err(_) => {
                        on_completion(IoCompletion::WorkerStopped);
                        return;
                    }
                };
            }
            if mailbox.shutdown
                && mailbox.note.is_none()
                && mailbox.external.is_none()
                && mailbox.cleanup_temporary.is_none()
                && mailbox.preserve_canonical.is_none()
                && mailbox.asset_paste.is_none()
                && mailbox.asset_sync.is_none()
                && mailbox.export.is_none()
                && mailbox.config.is_none()
            {
                return;
            }
            if let Some(note) = mailbox.note.take() {
                mailbox.metrics.note_started += 1;
                mailbox.note_blocked_for_ack = true;
                WorkerJob::Note(note)
            } else if let Some(paste) = mailbox.asset_paste.take() {
                WorkerJob::AssetPaste(paste)
            } else if let Some(sync) = mailbox.asset_sync.take() {
                WorkerJob::AssetSync(sync)
            } else if let Some(export) = mailbox.export.take() {
                WorkerJob::Export(export)
            } else if let Some(external) = mailbox.external.take() {
                mailbox.metrics.external_checks += 1;
                WorkerJob::External(external)
            } else if let Some((temporary, purpose)) = mailbox.cleanup_temporary.take() {
                WorkerJob::CleanupTemporary(temporary, purpose)
            } else if let Some((source, destination)) = mailbox.preserve_canonical.take() {
                WorkerJob::PreserveCanonical(source, destination)
            } else if let Some(config) = mailbox.config.take() {
                WorkerJob::Config(config)
            } else {
                continue;
            }
        };

        match work {
            WorkerJob::Note(job) => {
                let generation = job.request.generation;
                let result = persist_note(&job.target, &job.temporary, &job.request);
                if let Ok(mut mailbox) = shared.0.lock() {
                    mailbox.metrics.note_completed += 1;
                }
                on_completion(IoCompletion::Note(NoteCompletion { generation, result }));
            }
            WorkerJob::External(target) => {
                let completion = inspect_note_state_with_retry(&target);
                on_completion(IoCompletion::External(completion));
            }
            WorkerJob::AssetPaste(job) => {
                let expected_generation = job.request.expected_generation;
                let result = AssetStorage::open(&job.roots.images, &job.roots.trash)
                    .map_err(|error| {
                        AssetPasteFailure::without_publication(
                            expected_generation,
                            AssetPasteError::Storage(error),
                        )
                    })
                    .and_then(|storage| prepare_and_store_paste(&storage, job.request));
                on_completion(IoCompletion::AssetPaste(result));
            }
            WorkerJob::AssetSync(job) => {
                let result =
                    AssetStorage::open(&job.roots.images, &job.roots.trash).and_then(|storage| {
                        for effect in &job.effects {
                            storage.apply_effect(effect)?;
                        }
                        match (job.reconcile, job.safe_note) {
                            (Some(AssetReconcileMode::SafeBoundary), Some((note, expected))) => {
                                reconcile_safe_boundary(&storage, &note, expected, &job.references)
                            }
                            (Some(mode), _) => storage.reconcile(&job.references, mode),
                            (None, _) => Ok(AssetReconcileReport::default()),
                        }
                    });
                on_completion(IoCompletion::AssetSync {
                    request_id: job.request_id,
                    generation: job.generation,
                    result,
                });
            }
            WorkerJob::Export(request) => {
                let generation = request.snapshot.generation;
                on_completion(IoCompletion::Export {
                    generation,
                    result: export_snapshot(request),
                });
            }
            WorkerJob::Config(job) => {
                let revision = job.request.revision;
                let result = save_config(&job.target, &job.temporary, &job.request.config);
                on_completion(IoCompletion::Config { revision, result });
            }
            WorkerJob::CleanupTemporary(path, purpose) => {
                let result = remove_temporary(&path);
                on_completion(IoCompletion::TemporaryRemoved { purpose, result });
            }
            WorkerJob::PreserveCanonical(source, destination) => {
                let completion = preserve_canonical(&source, &destination);
                on_completion(IoCompletion::CanonicalPreserved(completion));
            }
        }
    }
}

enum WorkerJob {
    Note(NoteJob),
    External(PathBuf),
    Config(ConfigJob),
    CleanupTemporary(PathBuf, TemporaryCleanup),
    PreserveCanonical(PathBuf, PathBuf),
    AssetPaste(AssetPasteJob),
    AssetSync(AssetSyncJob),
    Export(ExportRequest),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;
    use stickymd_core::{Generation, LineEnding, hash_bytes};

    fn job(text: &str) -> NoteJob {
        NoteJob {
            target: "note.md".into(),
            temporary: "note.md.tmp".into(),
            request: PersistRequest {
                generation: Generation::initial(),
                text: text.into(),
                line_ending: LineEnding::Crlf,
                mode: super::super::PersistMode::ForceOverwrite,
            },
        }
    }

    #[test]
    fn mailbox_has_at_most_one_latest_pending_note() {
        let mut mailbox = Mailbox::default();
        for index in 0..1000 {
            mailbox.replace_note(job(&index.to_string()));
        }
        assert_eq!(mailbox.metrics.note_submitted, 1000);
        assert_eq!(mailbox.metrics.note_coalesced, 999);
        assert_eq!(mailbox.note.unwrap().request.text.as_ref(), "999");
    }

    #[test]
    fn next_note_waits_for_ack_and_stale_base_request_is_discarded() {
        let root = unique_temp_path("worker-barrier");
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"base").unwrap();

        let (sender, receiver) = mpsc::channel();
        let worker = PersistenceWorker::start(move |completion| {
            sender.send(completion).unwrap();
        })
        .unwrap();
        let request = |text: &'static str| PersistRequest {
            generation: Generation::initial(),
            text: text.into(),
            line_ending: LineEnding::Lf,
            mode: super::super::PersistMode::Guarded {
                expected: Some(hash_bytes(b"base")),
            },
        };
        worker.submit_note(target.clone(), temporary.clone(), request("first"));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(3)).unwrap(),
            IoCompletion::Note(_)
        ));

        worker.submit_note(target.clone(), temporary.clone(), request("stale-second"));
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        worker.acknowledge_note_completion();
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"first");

        drop(worker);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn asset_sync_coalescing_preserves_old_touches_but_uses_latest_references() {
        use stickymd_core::ManagedAssetLocation;

        let name = ManagedAssetName::parse("stickymd-0123456789abcdef0123.png").unwrap();
        let roots = || AssetRoots {
            images: "images".into(),
            trash: "trash".into(),
        };
        let mut mailbox = Mailbox::default();
        mailbox.replace_asset_sync(AssetSyncJob {
            roots: roots(),
            request_id: 1,
            generation: Generation::initial(),
            effects: vec![AssetEffect {
                name: name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            }],
            references: HashMap::new(),
            reconcile: Some(AssetReconcileMode::SafeBoundary),
            safe_note: Some(("note.md".into(), None)),
        });
        let mut restored_references = HashMap::new();
        restored_references.insert(name.clone(), 1);
        mailbox.replace_asset_sync(AssetSyncJob {
            roots: roots(),
            request_id: 2,
            generation: Generation::initial(),
            effects: Vec::new(),
            references: restored_references,
            reconcile: None,
            safe_note: None,
        });

        let combined = mailbox.asset_sync.unwrap();
        assert_eq!(combined.request_id, 2);
        assert_eq!(combined.reconcile, Some(AssetReconcileMode::Runtime));
        assert!(combined.safe_note.is_none());
        assert_eq!(combined.effects.len(), 1);
        assert_eq!(combined.effects[0].name, name);
        assert_eq!(combined.effects[0].from, ManagedAssetLocation::Trash);
        assert_eq!(combined.effects[0].to, ManagedAssetLocation::Images);
    }

    #[test]
    fn newer_asset_convergence_runs_after_an_older_sync_has_left_the_mailbox() {
        use stickymd_core::ManagedAssetLocation;
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
        let worker = PersistenceWorker::start(move |completion| {
            let block = matches!(completion, IoCompletion::AssetSync { request_id: 1, .. });
            sender.send(completion).unwrap();
            if block {
                callback_gate.wait();
            }
        })
        .unwrap();

        worker.submit_asset_sync(AssetSyncRequest {
            images: images.clone(),
            trash: trash.clone(),
            request_id: 1,
            generation: Generation::initial(),
            effects: vec![AssetEffect {
                name: asset.name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            }],
            references: HashMap::new(),
            reconcile: Some(AssetReconcileMode::Runtime),
            safe_note: None,
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
            images: images.clone(),
            trash: trash.clone(),
            request_id: 2,
            generation: Generation::initial().checked_next().unwrap(),
            effects: Vec::new(),
            references: latest_references,
            reconcile: Some(AssetReconcileMode::Runtime),
            safe_note: None,
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
}
