//! Single bounded I/O worker with latest-note coalescing.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use stickymd_core::ExternalFileState;
use stickymd_core::Generation;

use crate::config::{ConfigStorageError, RuntimeConfig, save_config};

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
    Config(Result<(), ConfigStorageError>),
    TemporaryRemoved {
        purpose: TemporaryCleanup,
        result: Result<(), NoteStorageError>,
    },
    CanonicalPreserved(Result<(), NoteStorageError>),
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
    config: RuntimeConfig,
}

#[derive(Default)]
struct Mailbox {
    note: Option<NoteJob>,
    external: Option<PathBuf>,
    config: Option<ConfigJob>,
    cleanup_temporary: Option<(PathBuf, TemporaryCleanup)>,
    preserve_canonical: Option<(PathBuf, PathBuf)>,
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

    fn has_work(&self) -> bool {
        self.shutdown
            || (!self.note_blocked_for_ack && self.note.is_some())
            || self.external.is_some()
            || self.cleanup_temporary.is_some()
            || self.preserve_canonical.is_some()
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

    pub fn submit_config(&self, target: PathBuf, temporary: PathBuf, config: RuntimeConfig) {
        let (lock, ready) = &*self.shared;
        if let Ok(mut mailbox) = lock.lock() {
            mailbox.config = Some(ConfigJob {
                target,
                temporary,
                config,
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
                && mailbox.config.is_none()
            {
                return;
            }
            if let Some(note) = mailbox.note.take() {
                mailbox.metrics.note_started += 1;
                mailbox.note_blocked_for_ack = true;
                WorkerJob::Note(note)
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
            WorkerJob::Config(job) => {
                let completion = save_config(&job.target, &job.temporary, &job.config);
                on_completion(IoCompletion::Config(completion));
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-worker-barrier-{}-{nonce}",
            std::process::id()
        ));
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
}
