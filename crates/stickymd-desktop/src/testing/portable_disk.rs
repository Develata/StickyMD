//! A disk for tests of the shell on any desktop: `std::fs` over one directory laid out as a
//! program directory's `note/`.
//!
//! It claims no atomicity and no crash safety. A save writes the temporary with `fs::write`,
//! observes the canonical note once and renames; nothing is flushed, a link is followed, and
//! another process may change the files between the observation and the rename. It decides a
//! guarded save with the shared `guarded_publish_verdict`, keeps the temporary when that refuses
//! the save, and moves an invalid canonical aside as `note.invalid-<seconds>.md`, as the Windows
//! adapter does; the shell's receipts read only those facts. It has no size bound, stores no
//! pasted image, copies no asset on export and does not encode configuration. A Platform Host
//! proves its own storage with the storage conformance rows, never with this.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use stickymd_app::config::RuntimeConfig;
use stickymd_app::execution::IoPorts;
use stickymd_app::flow::{PublishVerdict, guarded_publish_verdict};
use stickymd_app::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileReport,
    AssetSyncRequest, ConfigStorage, ExportCompletion, ExportRequest, ExportSink, FailureKind,
    ManagedAssetStore, NoteStorage, PersistMode, PersistRequest, PersistResult, PreserveOutcome,
    StorageFailure,
};
use stickymd_core::{
    ExternalFileState, LoadedDocument, ManagedAssetLocation, PreserveExpectation, hash_bytes,
};

/// The note, its temporary, its images and its trash in one directory.
#[derive(Clone, Debug)]
pub struct PortableDiskPorts {
    note_dir: PathBuf,
}

impl PortableDiskPorts {
    pub fn new(note_dir: impl Into<PathBuf>) -> Self {
        Self {
            note_dir: note_dir.into(),
        }
    }

    pub fn note_dir(&self) -> &Path {
        &self.note_dir
    }

    pub fn note_file(&self) -> PathBuf {
        self.note_dir.join("note.md")
    }

    pub fn note_tmp(&self) -> PathBuf {
        self.note_dir.join("note.md.tmp")
    }

    /// The four storage ports of the I/O worker, all on this directory.
    pub fn io_ports(&self) -> IoPorts<Self, Self, Self, Self> {
        IoPorts {
            note: self.clone(),
            assets: self.clone(),
            export: self.clone(),
            config: self.clone(),
        }
    }

    fn location(&self, location: ManagedAssetLocation) -> PathBuf {
        self.note_dir.join(match location {
            ManagedAssetLocation::Images => "images",
            ManagedAssetLocation::Trash => ".trash",
        })
    }
}

fn failed(action: &str, path: &Path, error: std::io::Error) -> StorageFailure {
    StorageFailure::new(
        FailureKind::Failed,
        format!("cannot {action} {}: {error}", path.display()),
    )
}

fn unsupported(what: &str) -> StorageFailure {
    StorageFailure::new(
        FailureKind::Unsupported,
        format!("the portable test disk {what}"),
    )
}

/// One read of the canonical note.
fn observe_note(path: &Path) -> Result<ExternalFileState, StorageFailure> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ExternalFileState::Missing),
        Err(error) => return Err(failed("read", path, error)),
    };
    Ok(match LoadedDocument::from_durable_bytes(&bytes) {
        Ok(document) => ExternalFileState::Present(document.into_external_fact()),
        Err(_) => ExternalFileState::InvalidUtf8 {
            fingerprint: hash_bytes(&bytes),
        },
    })
}

impl NoteStorage for PortableDiskPorts {
    fn persist(&mut self, request: &PersistRequest) -> Result<PersistResult, StorageFailure> {
        let (target, temporary) = (self.note_file(), self.note_tmp());
        fs::create_dir_all(&self.note_dir)
            .map_err(|error| failed("create", &self.note_dir, error))?;
        let bytes = LoadedDocument::encode_runtime(&request.text, request.line_ending);
        fs::write(&temporary, &bytes).map_err(|error| failed("write", &temporary, error))?;
        if let PersistMode::Guarded { expected } = request.mode {
            let observed = observe_note(&target)?;
            if guarded_publish_verdict(&observed, expected) == PublishVerdict::Conflict {
                return Ok(PersistResult::Conflict {
                    generation: request.generation,
                    observed,
                });
            }
        }
        fs::rename(&temporary, &target).map_err(|error| failed("publish", &target, error))?;
        Ok(PersistResult::Saved {
            generation: request.generation,
            fingerprint: hash_bytes(&bytes),
            durable_bytes: bytes.len(),
        })
    }

    fn observe(&mut self) -> Result<ExternalFileState, StorageFailure> {
        observe_note(&self.note_file())
    }

    fn remove_temporary(&mut self) -> Result<(), StorageFailure> {
        let temporary = self.note_tmp();
        match fs::remove_file(&temporary) {
            Err(error) if error.kind() != ErrorKind::NotFound => {
                Err(failed("remove", &temporary, error))
            }
            _ => Ok(()),
        }
    }

    fn preserve_canonical(
        &mut self,
        expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, StorageFailure> {
        let PreserveExpectation::Content(expected) = expected else {
            return Ok(PreserveOutcome::Unverifiable);
        };
        let target = self.note_file();
        let observed = observe_note(&target)?;
        let fingerprint = match &observed {
            ExternalFileState::Present(fact) => Some(fact.fingerprint),
            ExternalFileState::InvalidUtf8 { fingerprint } => Some(*fingerprint),
            ExternalFileState::Missing | ExternalFileState::TooLarge { .. } => None,
        };
        if fingerprint != Some(expected) {
            return Ok(PreserveOutcome::Changed(observed));
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let destination = self.note_dir.join(format!("note.invalid-{stamp}.md"));
        fs::rename(&target, &destination).map_err(|error| failed("move", &target, error))?;
        Ok(PreserveOutcome::Preserved)
    }
}

impl ManagedAssetStore for PortableDiskPorts {
    fn paste(
        &mut self,
        request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure> {
        Err(AssetPasteFailure {
            expected_generation: request.expected_generation,
            error: unsupported("stores no pasted image"),
            rollback: Default::default(),
        })
    }

    /// Moves each named file between `images/` and `.trash/` when it is there; reconciles nothing.
    fn sync(&mut self, request: &AssetSyncRequest) -> Result<AssetReconcileReport, StorageFailure> {
        for effect in &request.effects {
            let from = self.location(effect.from).join(effect.name.as_str());
            let to = self.location(effect.to).join(effect.name.as_str());
            match fs::rename(&from, &to) {
                Err(error) if error.kind() != ErrorKind::NotFound => {
                    return Err(failed("move", &from, error));
                }
                _ => {}
            }
        }
        Ok(AssetReconcileReport::default())
    }
}

impl ExportSink for PortableDiskPorts {
    fn export(&mut self, request: ExportRequest) -> Result<ExportCompletion, StorageFailure> {
        let snapshot = request.snapshot;
        let bytes = LoadedDocument::encode_runtime(&snapshot.text, snapshot.line_ending);
        fs::write(&request.target, bytes)
            .map_err(|error| failed("write", &request.target, error))?;
        Ok(ExportCompletion {
            generation: snapshot.generation,
            target: request.target,
            copied_assets: 0,
        })
    }
}

impl ConfigStorage for PortableDiskPorts {
    fn save(&mut self, _config: &RuntimeConfig) -> Result<(), StorageFailure> {
        Err(unsupported("does not encode configuration"))
    }
}

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// Creates a new empty directory under the system temporary directory; the caller removes it.
/// The sequence keeps parallel tests apart where the clock is coarse, the time keeps a reused
/// process id from meeting an old directory, and `create_dir` refuses one that exists.
pub fn unique_temp_dir(label: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "stickymd-test-{label}-{}-{timestamp}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&path).expect("create an owned test directory");
    path
}

#[cfg(test)]
mod tests {
    use stickymd_core::{Generation, LineEnding};

    use super::*;

    fn request(text: &str, mode: PersistMode) -> PersistRequest {
        PersistRequest {
            generation: Generation::initial(),
            text: text.into(),
            line_ending: LineEnding::Lf,
            mode,
        }
    }

    #[test]
    fn a_guarded_save_publishes_only_over_what_it_expected_and_keeps_the_temporary_otherwise() {
        let root = unique_temp_dir("portable-disk");
        let mut disk = PortableDiskPorts::new(root.join("note"));
        let fresh = request("one", PersistMode::Guarded { expected: None });
        let Ok(PersistResult::Saved { fingerprint, .. }) = disk.persist(&fresh) else {
            panic!("a new note publishes");
        };
        assert_eq!(fs::read(disk.note_file()).unwrap(), b"one");
        assert!(!disk.note_tmp().exists());
        let stale = request("two", PersistMode::Guarded { expected: None });
        assert!(matches!(
            disk.persist(&stale),
            Ok(PersistResult::Conflict { .. })
        ));
        assert_eq!(fs::read(disk.note_tmp()).unwrap(), b"two");
        let based = request(
            "three",
            PersistMode::Guarded {
                expected: Some(fingerprint),
            },
        );
        assert!(matches!(
            disk.persist(&based),
            Ok(PersistResult::Saved { .. })
        ));
        assert_eq!(fs::read(disk.note_file()).unwrap(), b"three");
        disk.remove_temporary().unwrap();
        disk.remove_temporary().unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_invalid_canonical_moves_aside_only_while_it_is_what_was_expected() {
        let root = unique_temp_dir("portable-disk");
        let mut disk = PortableDiskPorts::new(&root);
        fs::write(disk.note_file(), b"\xff").unwrap();
        assert_eq!(
            disk.observe().unwrap(),
            ExternalFileState::InvalidUtf8 {
                fingerprint: hash_bytes(b"\xff")
            }
        );
        let other = PreserveExpectation::Content(hash_bytes(b"other"));
        assert!(matches!(
            disk.preserve_canonical(other),
            Ok(PreserveOutcome::Changed(_))
        ));
        let expected = PreserveExpectation::Content(hash_bytes(b"\xff"));
        assert_eq!(
            disk.preserve_canonical(expected),
            Ok(PreserveOutcome::Preserved)
        );
        assert_eq!(disk.observe().unwrap(), ExternalFileState::Missing);
        let moved: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(moved.len(), 1);
        assert!(moved[0].starts_with("note.invalid-"), "{moved:?}");
        fs::remove_dir_all(root).unwrap();
    }
}
