//! Single bounded I/O worker with latest-note coalescing.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use std::collections::BTreeSet;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use stickymd_core::{
    AssetEffect, DiskFingerprint, Generation, ManagedAssetLocation, PreserveExpectation,
};

use crate::config::{ConfigPersistRequest, ConfigRevision};
use crate::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileMode,
    AssetReconcileReport, AssetSyncRequest, ConfigStorage, ExportCompletion, ExportRequest,
    ExportSink, ManagedAssetStore, NoteReceipt, NoteStorage, PersistRequest, StorageFailure,
    TemporaryCleanup,
};

#[derive(Debug)]
pub enum IoCompletion {
    Note(NoteReceipt),
    Config {
        revision: ConfigRevision,
        result: Result<(), StorageFailure>,
    },
    AssetPaste(Result<AssetPasteCompletion, AssetPasteFailure>),
    AssetSync {
        request_id: u64,
        generation: Generation,
        result: Result<AssetReconcileReport, StorageFailure>,
    },
    Export {
        generation: Generation,
        result: Result<ExportCompletion, StorageFailure>,
    },
    WorkerStopped,
}

/// The capabilities the worker thread owns; all of them move onto it when it starts.
pub struct IoPorts<N, A, E, C> {
    pub note: N,
    pub assets: A,
    pub export: E,
    pub config: C,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkerMetrics {
    pub note_submitted: u64,
    pub note_started: u64,
    pub note_completed: u64,
    pub note_coalesced: u64,
    pub external_checks: u64,
}

struct PreserveJob {
    expected: PreserveExpectation,
    operation: u64,
}

#[derive(Default)]
struct Mailbox {
    note: Option<PersistRequest>,
    /// A queued observation, with the Durable Fingerprint known when it was requested.
    external: Option<Option<DiskFingerprint>>,
    config: Option<ConfigPersistRequest>,
    cleanup_temporary: Option<TemporaryCleanup>,
    preserve_canonical: Option<PreserveJob>,
    asset_paste: Option<AssetPasteRequest>,
    asset_sync: Option<AssetSyncRequest>,
    export: Option<ExportRequest>,
    shutdown: bool,
    note_blocked_for_ack: bool,
    metrics: WorkerMetrics,
}

impl Mailbox {
    fn replace_note(&mut self, request: PersistRequest) {
        // The note job reuses the fixed temp. A conflict-discard cleanup still queued would run
        // after it and delete what a failed save left there, the only durable copy of newer
        // text; the note job supersedes it (it replaces or overwrites that temp itself).
        if self.cleanup_temporary == Some(TemporaryCleanup::ConflictDiscarded) {
            self.cleanup_temporary = None;
        }
        self.metrics.note_submitted += 1;
        if self.note.replace(request).is_some() {
            self.metrics.note_coalesced += 1;
        }
    }

    fn replace_asset_sync(&mut self, mut job: AssetSyncRequest) {
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
                        (ManagedAssetLocation::Trash, ManagedAssetLocation::Images)
                    } else {
                        (ManagedAssetLocation::Images, ManagedAssetLocation::Trash)
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
                job.safe_expected = None;
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

    fn take_next_job(&mut self) -> Option<WorkerJob> {
        if !self.note_blocked_for_ack
            && let Some(note) = self.note.take()
        {
            self.metrics.note_started += 1;
            self.note_blocked_for_ack = true;
            Some(WorkerJob::Note(note))
        } else if let Some(paste) = self.asset_paste.take() {
            Some(WorkerJob::AssetPaste(paste))
        } else if let Some(sync) = self.asset_sync.take() {
            Some(WorkerJob::AssetSync(sync))
        } else if let Some(export) = self.export.take() {
            Some(WorkerJob::Export(export))
        } else if let Some(requested_base) = self.external.take() {
            self.metrics.external_checks += 1;
            Some(WorkerJob::External(requested_base))
        } else if let Some(purpose) = self.cleanup_temporary.take() {
            Some(WorkerJob::CleanupTemporary(purpose))
        } else if let Some(job) = self.preserve_canonical.take() {
            Some(WorkerJob::PreserveCanonical(job))
        } else {
            self.config.take().map(WorkerJob::Config)
        }
    }
}

pub struct PersistenceWorker {
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl PersistenceWorker {
    pub fn start<N, A, E, C, F>(
        ports: IoPorts<N, A, E, C>,
        on_completion: F,
    ) -> Result<Self, std::io::Error>
    where
        N: NoteStorage,
        A: ManagedAssetStore,
        E: ExportSink,
        C: ConfigStorage,
        F: Fn(IoCompletion) + Send + Sync + 'static,
    {
        let shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
        let worker_shared = Arc::clone(&shared);
        let on_completion = Arc::new(on_completion);
        let thread = thread::Builder::new()
            .name("stickymd-io".into())
            .stack_size(512 * 1024)
            .spawn(move || run_worker(ports, worker_shared, on_completion))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn submit_note(&self, request: PersistRequest) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.replace_note(request);
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

    /// Queues one durable observation; `requested_base` is the known Durable Fingerprint now.
    pub fn inspect_external(&self, requested_base: Option<DiskFingerprint>) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.external = Some(requested_base);
            ready.notify_one();
        }
    }

    pub fn submit_config(&self, request: ConfigPersistRequest) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.config = Some(request);
            ready.notify_one();
        }
    }

    pub fn remove_temporary(&self, purpose: TemporaryCleanup) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.cleanup_temporary = Some(purpose);
            ready.notify_one();
        }
    }

    /// Queues moving the canonical note aside for recovery operation `operation`, only while
    /// it is still `expected`.
    pub fn preserve_canonical(&self, expected: PreserveExpectation, operation: u64) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.preserve_canonical = Some(PreserveJob {
                expected,
                operation,
            });
            ready.notify_one();
        }
    }

    pub fn submit_asset_paste(&self, request: AssetPasteRequest) -> bool {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            if mailbox.asset_paste.is_some() {
                return false;
            }
            mailbox.asset_paste = Some(request);
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
            mailbox.replace_asset_sync(request);
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

fn run_worker<N, A, E, C, F>(
    mut ports: IoPorts<N, A, E, C>,
    shared: Arc<(Mutex<Mailbox>, Condvar)>,
    on_completion: Arc<F>,
) where
    N: NoteStorage,
    A: ManagedAssetStore,
    E: ExportSink,
    C: ConfigStorage,
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
            if let Some(job) = mailbox.take_next_job() {
                job
            } else {
                continue;
            }
        };

        match work {
            WorkerJob::Note(request) => {
                let result = ports.note.persist(&request);
                if let Ok(mut mailbox) = shared.0.lock() {
                    mailbox.metrics.note_completed += 1;
                }
                on_completion(IoCompletion::Note(NoteReceipt::Save {
                    generation: request.generation,
                    mode: request.mode,
                    result,
                }));
            }
            WorkerJob::External(requested_base) => {
                let result = ports.note.observe();
                on_completion(IoCompletion::Note(NoteReceipt::External {
                    requested_base,
                    result,
                }));
            }
            WorkerJob::AssetPaste(request) => {
                on_completion(IoCompletion::AssetPaste(ports.assets.paste(request)));
            }
            WorkerJob::AssetSync(request) => {
                let result = ports.assets.sync(&request);
                on_completion(IoCompletion::AssetSync {
                    request_id: request.request_id,
                    generation: request.generation,
                    result,
                });
            }
            WorkerJob::Export(request) => {
                let generation = request.snapshot.generation;
                on_completion(IoCompletion::Export {
                    generation,
                    result: ports.export.export(request),
                });
            }
            WorkerJob::Config(request) => {
                let result = ports.config.save(&request.config);
                on_completion(IoCompletion::Config {
                    revision: request.revision,
                    result,
                });
            }
            WorkerJob::CleanupTemporary(purpose) => {
                let result = ports.note.remove_temporary();
                on_completion(IoCompletion::Note(NoteReceipt::TemporaryRemoved {
                    purpose,
                    result,
                }));
            }
            WorkerJob::PreserveCanonical(job) => {
                let result = ports.note.preserve_canonical(job.expected);
                on_completion(IoCompletion::Note(NoteReceipt::CanonicalPreserved {
                    operation: job.operation,
                    result,
                }));
            }
        }
    }
}

enum WorkerJob {
    Note(PersistRequest),
    External(Option<DiskFingerprint>),
    Config(ConfigPersistRequest),
    CleanupTemporary(TemporaryCleanup),
    PreserveCanonical(PreserveJob),
    AssetPaste(AssetPasteRequest),
    AssetSync(AssetSyncRequest),
    Export(ExportRequest),
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use stickymd_core::{DocumentSnapshot, LineEnding, ManagedAssetName, Selection};

    use super::*;
    use crate::config::RuntimeConfig;
    use crate::flow::ClipboardPaste;
    use crate::ports::PersistMode;

    fn job(text: &str) -> PersistRequest {
        PersistRequest {
            generation: Generation::initial(),
            text: text.into(),
            line_ending: LineEnding::Crlf,
            mode: PersistMode::ForceOverwrite,
        }
    }

    fn sync(request_id: u64) -> AssetSyncRequest {
        AssetSyncRequest {
            request_id,
            generation: Generation::initial(),
            effects: Vec::new(),
            references: HashMap::new(),
            reconcile: None,
            safe_expected: None,
        }
    }

    /// The priority is a contract: the note goes first, and a config write never delays anything.
    #[test]
    fn the_mailbox_drains_every_job_kind_in_its_priority_order() {
        let mut mailbox = Mailbox {
            config: Some(ConfigPersistRequest {
                revision: ConfigRevision::initial(),
                config: RuntimeConfig::default(),
            }),
            preserve_canonical: Some(PreserveJob {
                expected: PreserveExpectation::Unverifiable,
                operation: 1,
            }),
            external: Some(None),
            export: Some(ExportRequest {
                snapshot: DocumentSnapshot {
                    text: "".into(),
                    generation: Generation::initial(),
                    line_ending: LineEnding::Lf,
                },
                target: "export.md".into(),
            }),
            asset_sync: Some(sync(1)),
            asset_paste: Some(AssetPasteRequest {
                expected_generation: Generation::initial(),
                selection: Selection::caret(0),
                timestamp_ms: 0,
                payload: ClipboardPaste::EncodedImage(Vec::new()),
            }),
            ..Mailbox::default()
        };
        mailbox.replace_note(job("first"));
        // After the note: a new note job drops a queued conflict discard.
        mailbox.cleanup_temporary = Some(TemporaryCleanup::ConflictDiscarded);
        let order: Vec<_> = std::iter::from_fn(|| mailbox.take_next_job())
            .map(|job| match job {
                WorkerJob::Note(_) => "note",
                WorkerJob::AssetPaste(_) => "paste",
                WorkerJob::AssetSync(_) => "sync",
                WorkerJob::Export(_) => "export",
                WorkerJob::External(..) => "external",
                WorkerJob::CleanupTemporary(..) => "cleanup",
                WorkerJob::PreserveCanonical(_) => "preserve",
                WorkerJob::Config(_) => "config",
            })
            .collect();
        assert_eq!(
            order,
            [
                "note", "paste", "sync", "export", "external", "cleanup", "preserve", "config"
            ]
        );
    }

    #[test]
    fn mailbox_has_at_most_one_latest_pending_note() {
        let mut mailbox = Mailbox::default();
        for index in 0..1000 {
            mailbox.replace_note(job(&index.to_string()));
        }
        assert_eq!(mailbox.metrics.note_submitted, 1000);
        assert_eq!(mailbox.metrics.note_coalesced, 999);
        assert_eq!(mailbox.note.unwrap().text.as_ref(), "999");
    }

    #[test]
    fn a_newer_note_job_supersedes_only_a_queued_conflict_discard_cleanup() {
        let mut mailbox = Mailbox {
            cleanup_temporary: Some(TemporaryCleanup::ConflictDiscarded),
            ..Mailbox::default()
        };
        mailbox.replace_note(job("newer text"));
        assert!(mailbox.cleanup_temporary.is_none());
        // A recovery cleanup stays: recovery blocks ordinary saves, and its receipt ends it.
        let recovery = TemporaryCleanup::RecoveryResolved { operation: 1 };
        mailbox.cleanup_temporary = Some(recovery);
        mailbox.replace_note(job("recovery save"));
        assert_eq!(mailbox.cleanup_temporary, Some(recovery));
    }

    #[test]
    fn unrelated_work_cannot_bypass_the_note_completion_barrier() {
        let mut mailbox = Mailbox {
            note_blocked_for_ack: true,
            ..Mailbox::default()
        };
        mailbox.replace_note(job("must-wait"));
        mailbox.external = Some(None);

        assert!(matches!(
            mailbox.take_next_job(),
            Some(WorkerJob::External(None))
        ));
        assert!(mailbox.note.is_some());
        assert!(mailbox.note_blocked_for_ack);
        assert_eq!(mailbox.metrics.note_started, 0);
        assert!(mailbox.take_next_job().is_none());
    }

    #[test]
    fn asset_sync_coalescing_preserves_old_touches_but_uses_latest_references() {
        let name = ManagedAssetName::parse("stickymd-0123456789abcdef0123.png").unwrap();
        let mut mailbox = Mailbox::default();
        mailbox.replace_asset_sync(AssetSyncRequest {
            effects: vec![AssetEffect {
                name: name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            }],
            reconcile: Some(AssetReconcileMode::SafeBoundary),
            safe_expected: Some(None),
            ..sync(1)
        });
        let mut restored_references = HashMap::new();
        restored_references.insert(name.clone(), 1);
        mailbox.replace_asset_sync(AssetSyncRequest {
            references: restored_references,
            ..sync(2)
        });

        let combined = mailbox.asset_sync.unwrap();
        assert_eq!(combined.request_id, 2);
        assert_eq!(combined.reconcile, Some(AssetReconcileMode::Runtime));
        assert!(combined.safe_expected.is_none());
        assert_eq!(combined.effects.len(), 1);
        assert_eq!(combined.effects[0].name, name);
        assert_eq!(combined.effects[0].from, ManagedAssetLocation::Trash);
        assert_eq!(combined.effects[0].to, ManagedAssetLocation::Images);
    }
}
