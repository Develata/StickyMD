//! The one canonical note, guarded and published over the macOS directory handle.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use stickymd_app::flow::{NoteObservation, PublishVerdict, guarded_publish_verdict};
use stickymd_app::ports::{PersistMode, PersistRequest, PersistResult, PreserveOutcome};
use stickymd_core::{
    DiskFingerprint, ExternalFileState, LoadedDocument, NoteDecodeError, PreserveExpectation,
    RecoveryInspection, hash_bytes,
};
use thiserror::Error;

use crate::platform::macos::file::{DirectoryHandle, FileObservation, full_sync};

pub const MAX_NOTE_LOAD: u64 = 16 * 1024 * 1024;
pub const NOTE_NAME: &str = "note.md";
pub const TEMPORARY_NAME: &str = "note.md.tmp";

const OBSERVE_RETRIES_MS: [u64; 3] = [50, 150, 300];

#[derive(Debug, Error)]
pub enum NoteStorageError {
    #[error("note directory is not a plain directory: {}", .0.display())]
    UnsafeNoteDirectory(PathBuf),
    #[error("cannot recreate note directory {path}: {source}")]
    CreateNoteDirectory { path: PathBuf, source: io::Error },
    #[error("cannot read note metadata: {0}")]
    Metadata(io::Error),
    #[error("cannot read note: {0}")]
    Read(io::Error),
    #[error("note changed while it was being observed")]
    ObservationChanged,
    #[error("cannot publish note: {0}")]
    Publish(io::Error),
    #[error("note was published but its directory could not be flushed: {0}")]
    DirectorySync(io::Error),
    #[error("cannot remove the temporary note: {0}")]
    RemoveTemporary(io::Error),
    #[error("cannot preserve the unusable temporary note: {0}")]
    QuarantineTemporary(io::Error),
    #[error("all temporary-note preservation names are occupied")]
    QuarantineNameExhausted,
    #[error("cannot preserve the canonical note: {0}")]
    PreserveCanonical(io::Error),
    #[error(
        "note.md changed while it was moved aside and could not be moved back; it is at {path}: {source}"
    )]
    PreserveDisplaced { path: String, source: io::Error },
}

/// The note files of one program directory.
#[derive(Debug)]
pub struct NoteFiles {
    path: PathBuf,
    directory: DirectoryHandle,
    /// A publish whose directory flush failed. The disk holds these bytes while the document
    /// still counts as unsaved, so the next guard has to accept them as its own.
    unconfirmed_publish: Option<DiskFingerprint>,
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn suffixed(stem: &str, stamp: u64, suffix: u16) -> String {
    if suffix == 0 {
        format!("{stem}-{stamp}.md")
    } else {
        format!("{stem}-{stamp}-{suffix}.md")
    }
}

impl NoteFiles {
    pub fn open(path: &Path) -> Result<Self, NoteStorageError> {
        Ok(Self {
            path: path.to_owned(),
            directory: open_directory(path)?,
            unconfirmed_publish: None,
        })
    }

    /// The handle stays valid when the directory moves, but a deleted directory takes no new
    /// entries: it is recreated at its path, as a file or link there is refused.
    fn recreate_directory(&mut self) -> Result<(), NoteStorageError> {
        match std::fs::create_dir(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(NoteStorageError::CreateNoteDirectory {
                    path: self.path.clone(),
                    source,
                });
            }
        }
        self.directory = open_directory(&self.path)?;
        Ok(())
    }

    pub fn inspect_note(&self) -> Result<Option<NoteObservation>, NoteStorageError> {
        self.inspect(NOTE_NAME)
    }

    pub fn inspect_temporary(&self) -> Result<Option<NoteObservation>, NoteStorageError> {
        self.inspect(TEMPORARY_NAME)
    }

    fn inspect(&self, name: &str) -> Result<Option<NoteObservation>, NoteStorageError> {
        let mut outcome = self.inspect_once(name);
        for _ in 0..2 {
            if !matches!(outcome, Err(NoteStorageError::ObservationChanged)) {
                break;
            }
            std::thread::yield_now();
            outcome = self.inspect_once(name);
        }
        outcome
    }

    fn inspect_once(&self, name: &str) -> Result<Option<NoteObservation>, NoteStorageError> {
        let mut file = match self.directory.open_read(name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(NoteStorageError::Read(error)),
        };
        let before = FileObservation::of(&file).map_err(NoteStorageError::Metadata)?;
        let mut bytes = Vec::new();
        if before.size() <= MAX_NOTE_LOAD {
            bytes.reserve(before.size() as usize);
            Read::by_ref(&mut file)
                .take(MAX_NOTE_LOAD + 1)
                .read_to_end(&mut bytes)
                .map_err(NoteStorageError::Read)?;
        }
        let after = FileObservation::of(&file).map_err(NoteStorageError::Metadata)?;
        let modified = file.metadata().and_then(|meta| meta.modified()).ok();
        // Unchanged through the read, and still the file the name means.
        let at_name = match self.directory.open_read(name) {
            Ok(file) => FileObservation::of(&file).map_err(NoteStorageError::Metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(NoteStorageError::ObservationChanged);
            }
            Err(error) => return Err(NoteStorageError::Metadata(error)),
        };
        if before != after || after != at_name {
            return Err(NoteStorageError::ObservationChanged);
        }
        if before.size() > MAX_NOTE_LOAD || bytes.len() as u64 > MAX_NOTE_LOAD {
            return Ok(Some(NoteObservation {
                state: ExternalFileState::TooLarge {
                    bytes: before.size().max(bytes.len() as u64),
                },
                recovery: RecoveryInspection {
                    bytes: Vec::new(),
                    modified,
                },
            }));
        }
        let state = match LoadedDocument::from_durable_bytes(&bytes) {
            Ok(loaded) => ExternalFileState::Present(loaded.into_external_fact()),
            Err(NoteDecodeError::InvalidUtf8) => ExternalFileState::InvalidUtf8 {
                fingerprint: hash_bytes(&bytes),
            },
        };
        Ok(Some(NoteObservation {
            state,
            recovery: RecoveryInspection { bytes, modified },
        }))
    }

    pub fn observe(&self) -> Result<ExternalFileState, NoteStorageError> {
        let mut outcome = self.inspect_note();
        for delay in OBSERVE_RETRIES_MS {
            match outcome {
                Err(
                    NoteStorageError::Read(_)
                    | NoteStorageError::Metadata(_)
                    | NoteStorageError::ObservationChanged,
                ) => {
                    std::thread::sleep(std::time::Duration::from_millis(delay));
                    outcome = self.inspect_note();
                }
                _ => break,
            }
        }
        Ok(outcome?.map_or(ExternalFileState::Missing, |observed| observed.state))
    }

    pub fn persist(&mut self, request: &PersistRequest) -> Result<PersistResult, NoteStorageError> {
        self.persist_with(request, |_| {}, DirectoryHandle::sync)
    }

    fn persist_with(
        &mut self,
        request: &PersistRequest,
        before_publish: impl FnOnce(&Self),
        flush_directory: impl FnOnce(&DirectoryHandle) -> io::Result<()>,
    ) -> Result<PersistResult, NoteStorageError> {
        let bytes = LoadedDocument::encode_runtime(&request.text, request.line_ending);
        let fingerprint = hash_bytes(&bytes);
        match self.write_temporary(&bytes) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.recreate_directory()?;
                self.write_temporary(&bytes)
                    .map_err(NoteStorageError::Publish)?;
            }
            other => other.map_err(NoteStorageError::Publish)?,
        }

        // The final guard: after the temporary is durable, immediately before publishing. The
        // watcher is only a hint; this check is what protects external edits.
        let publish_new = match request.mode {
            PersistMode::Guarded { expected } => {
                let observed = self.observe()?;
                let expected = self.expected_given_unconfirmed(&observed, expected);
                match guarded_publish_verdict(&observed, expected) {
                    PublishVerdict::PublishNew => true,
                    PublishVerdict::ReplaceExisting => false,
                    PublishVerdict::Conflict => {
                        return Ok(PersistResult::Conflict {
                            generation: request.generation,
                            observed,
                        });
                    }
                }
            }
            PersistMode::ForceOverwrite => false,
        };

        before_publish(self);
        if publish_new {
            if let Err(error) = self.directory.rename_no_replace(TEMPORARY_NAME, NOTE_NAME) {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    return Ok(PersistResult::Conflict {
                        generation: request.generation,
                        observed: self.observe()?,
                    });
                }
                return Err(NoteStorageError::Publish(error));
            }
        } else {
            self.directory
                .replace(TEMPORARY_NAME, NOTE_NAME)
                .map_err(NoteStorageError::Publish)?;
        }
        // The name now means the new bytes. Until the directory is flushed that is not durable,
        // and a failure here is a failed save, not a success with a footnote (plan 05).
        if let Err(error) = flush_directory(&self.directory) {
            self.unconfirmed_publish = Some(fingerprint);
            return Err(NoteStorageError::DirectorySync(error));
        }
        self.unconfirmed_publish = None;
        Ok(PersistResult::Saved {
            generation: request.generation,
            fingerprint,
            durable_bytes: bytes.len(),
        })
    }

    /// A retry after a failed directory flush finds its own earlier bytes on disk.
    fn expected_given_unconfirmed(
        &self,
        observed: &ExternalFileState,
        expected: Option<DiskFingerprint>,
    ) -> Option<DiskFingerprint> {
        match (observed, self.unconfirmed_publish) {
            (ExternalFileState::Present(fact), Some(published))
                if fact.fingerprint == published =>
            {
                Some(published)
            }
            (ExternalFileState::InvalidUtf8 { fingerprint }, Some(published))
                if *fingerprint == published =>
            {
                Some(published)
            }
            _ => expected,
        }
    }

    fn write_temporary(&self, bytes: &[u8]) -> io::Result<()> {
        let mut temporary = self.directory.create_or_truncate(TEMPORARY_NAME)?;
        temporary.write_all(bytes)?;
        full_sync(&temporary)
    }

    /// Removes the temporary; an absent one is success.
    pub fn remove_temporary(&self) -> Result<(), NoteStorageError> {
        match self.directory.remove_file(TEMPORARY_NAME) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(NoteStorageError::RemoveTemporary(error)),
        }
    }

    /// Moves unusable recovery evidence away from the fixed temporary name.
    pub fn quarantine_temporary(&self) -> Result<PathBuf, NoteStorageError> {
        let stamp = timestamp();
        for suffix in 0..1000_u16 {
            let name = suffixed("note.invalid-tmp", stamp, suffix);
            match self.directory.rename_no_replace(TEMPORARY_NAME, &name) {
                Ok(()) => return Ok(self.path.join(name)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(NoteStorageError::QuarantineTemporary(error)),
            }
        }
        Err(NoteStorageError::QuarantineNameExhausted)
    }

    /// Moves the canonical note aside only while it still has the expected content.
    ///
    /// POSIX cannot exclude writers or rename by descriptor, so the check comes after the move:
    /// a rename destroys nothing, and a file that turns out not to be the expected one goes back.
    pub fn preserve_canonical(
        &self,
        expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, NoteStorageError> {
        self.preserve_canonical_with(expected, |_| {})
    }

    fn preserve_canonical_with(
        &self,
        expected: PreserveExpectation,
        before_move: impl FnOnce(&Self),
    ) -> Result<PreserveOutcome, NoteStorageError> {
        let PreserveExpectation::Content(expected) = expected else {
            return Ok(PreserveOutcome::Unverifiable);
        };
        let mut file = match self.directory.open_read(NOTE_NAME) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PreserveOutcome::Changed(ExternalFileState::Missing));
            }
            // A link or other non-file at the canonical name is refused and left in place.
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                return Err(NoteStorageError::PreserveCanonical(error));
            }
            Err(error) if error.raw_os_error() == Some(symbolic_link_loop()) => {
                return Err(NoteStorageError::PreserveCanonical(error));
            }
            Err(error) => return Err(NoteStorageError::Read(error)),
        };
        let observed = FileObservation::of(&file).map_err(NoteStorageError::Metadata)?;
        if !content_matches(&mut file, observed.size(), expected)? {
            return self.observe().map(PreserveOutcome::Changed);
        }
        before_move(self);
        let stamp = timestamp();
        for suffix in 0..1000_u16 {
            let name = suffixed("note.invalid", stamp, suffix);
            match self.directory.rename_no_replace(NOTE_NAME, &name) {
                Ok(()) => return self.confirm_preserved(&name, &observed, expected),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    return Ok(PreserveOutcome::Changed(ExternalFileState::Missing));
                }
                Err(error) => return Err(NoteStorageError::PreserveCanonical(error)),
            }
        }
        Err(NoteStorageError::PreserveCanonical(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "all canonical recovery preservation names are occupied",
        )))
    }

    fn confirm_preserved(
        &self,
        name: &str,
        observed: &FileObservation,
        expected: DiskFingerprint,
    ) -> Result<PreserveOutcome, NoteStorageError> {
        let moved_is_expected = (|| -> Result<bool, NoteStorageError> {
            let mut moved = self
                .directory
                .open_read(name)
                .map_err(NoteStorageError::PreserveCanonical)?;
            let now = FileObservation::of(&moved).map_err(NoteStorageError::Metadata)?;
            Ok(now.same_file_as(observed) && content_matches(&mut moved, now.size(), expected)?)
        })()
        .unwrap_or(false);
        if moved_is_expected {
            return Ok(PreserveOutcome::Preserved);
        }
        match self.directory.rename_no_replace(name, NOTE_NAME) {
            Ok(()) => self.observe().map(PreserveOutcome::Changed),
            Err(source) => Err(NoteStorageError::PreserveDisplaced {
                path: self.path.join(name).display().to_string(),
                source,
            }),
        }
    }
}

/// `ELOOP`, what `O_NOFOLLOW` answers for a link.
const fn symbolic_link_loop() -> i32 {
    62
}

fn open_directory(path: &Path) -> Result<DirectoryHandle, NoteStorageError> {
    DirectoryHandle::open(path).map_err(|error| {
        if error.raw_os_error() == Some(symbolic_link_loop())
            || error.kind() == io::ErrorKind::NotADirectory
        {
            NoteStorageError::UnsafeNoteDirectory(path.to_owned())
        } else {
            NoteStorageError::Metadata(error)
        }
    })
}

fn content_matches(
    file: &mut std::fs::File,
    size: u64,
    expected: DiskFingerprint,
) -> Result<bool, NoteStorageError> {
    if size > MAX_NOTE_LOAD {
        return Ok(false);
    }
    let mut bytes = Vec::with_capacity(size as usize);
    Read::by_ref(file)
        .take(MAX_NOTE_LOAD + 1)
        .read_to_end(&mut bytes)
        .map_err(NoteStorageError::Read)?;
    Ok(hash_bytes(&bytes) == expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::sync::Arc;
    use stickymd_core::{Generation, LineEnding};

    fn fixture(label: &str) -> (PathBuf, NoteFiles) {
        let root = unique_temp_path(label);
        fs::create_dir(&root).unwrap();
        let files = NoteFiles::open(&root).unwrap();
        (root, files)
    }

    fn request(text: &str, mode: PersistMode) -> PersistRequest {
        PersistRequest {
            generation: Generation::initial(),
            text: Arc::from(text),
            line_ending: LineEnding::Lf,
            mode,
        }
    }

    fn guarded(expected: &[u8]) -> PersistMode {
        PersistMode::Guarded {
            expected: Some(hash_bytes(expected)),
        }
    }

    const NEW: PersistMode = PersistMode::Guarded { expected: None };

    #[test]
    fn a_save_replaces_the_expected_note_and_leaves_no_temporary() {
        let (root, mut files) = fixture("persist-replace");
        fs::write(root.join(NOTE_NAME), b"base").unwrap();
        let outcome = files.persist(&request("next", guarded(b"base"))).unwrap();
        assert!(matches!(outcome, PersistResult::Saved { .. }));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"next");
        assert!(!root.join(TEMPORARY_NAME).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_stale_expectation_conflicts_and_writes_nothing() {
        let (root, mut files) = fixture("persist-conflict");
        fs::write(root.join(NOTE_NAME), b"external").unwrap();
        let outcome = files.persist(&request("mine", guarded(b"base"))).unwrap();
        assert!(matches!(outcome, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"external");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn force_overwrite_skips_the_guard() {
        let (root, mut files) = fixture("persist-force");
        fs::write(root.join(NOTE_NAME), b"external").unwrap();
        let outcome = files
            .persist(&request("mine", PersistMode::ForceOverwrite))
            .unwrap();
        assert!(matches!(outcome, PersistResult::Saved { .. }));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"mine");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_first_creation_never_overwrites_a_note_that_appears() {
        let (root, mut files) = fixture("persist-new-race");
        let outcome = files
            .persist_with(
                &request("mine", NEW),
                |_| fs::write(root.join(NOTE_NAME), b"appeared").unwrap(),
                DirectoryHandle::sync,
            )
            .unwrap();
        assert!(matches!(outcome, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"appeared");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_first_creation_with_a_note_present_conflicts() {
        let (root, mut files) = fixture("persist-new-present");
        fs::write(root.join(NOTE_NAME), b"there").unwrap();
        let outcome = files.persist(&request("mine", NEW)).unwrap();
        assert!(matches!(outcome, PersistResult::Conflict { .. }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_directory_flush_is_a_failed_save_and_the_retry_accepts_its_own_bytes() {
        let (root, mut files) = fixture("persist-flush");
        fs::write(root.join(NOTE_NAME), b"base").unwrap();
        let failure = files
            .persist_with(
                &request("first", guarded(b"base")),
                |_| {},
                |_| Err(io::Error::other("disk")),
            )
            .unwrap_err();
        assert!(matches!(failure, NoteStorageError::DirectorySync(_)));
        // The name moved: the disk holds the new bytes although the save was reported failed.
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"first");
        // The coordinator still expects the old base; the retry must not see a conflict.
        let retry = files.persist(&request("first", guarded(b"base"))).unwrap();
        assert!(matches!(retry, PersistResult::Saved { .. }));
        assert_eq!(files.unconfirmed_publish, None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_real_external_change_after_a_failed_flush_is_still_a_conflict() {
        let (root, mut files) = fixture("persist-flush-external");
        fs::write(root.join(NOTE_NAME), b"base").unwrap();
        files
            .persist_with(
                &request("first", guarded(b"base")),
                |_| {},
                |_| Err(io::Error::other("disk")),
            )
            .unwrap_err();
        fs::write(root.join(NOTE_NAME), b"external").unwrap();
        let retry = files.persist(&request("first", guarded(b"base"))).unwrap();
        assert!(matches!(retry, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"external");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_deleted_note_directory_is_recreated_for_the_save() {
        let (root, mut files) = fixture("persist-recreate");
        fs::remove_dir(&root).unwrap();
        files.persist(&request("back", NEW)).unwrap();
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"back");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_link_at_the_note_is_not_followed() {
        let (root, files) = fixture("inspect-link");
        fs::write(root.join("elsewhere"), b"x").unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join(NOTE_NAME)).unwrap();
        assert!(matches!(
            files.inspect_note(),
            Err(NoteStorageError::Read(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_too_large_note_is_reported_without_being_read() {
        let (root, files) = fixture("inspect-large");
        let file = fs::File::create(root.join(NOTE_NAME)).unwrap();
        file.set_len(MAX_NOTE_LOAD + 1).unwrap();
        let observed = files.inspect_note().unwrap().unwrap();
        assert_eq!(
            observed.state,
            ExternalFileState::TooLarge {
                bytes: MAX_NOTE_LOAD + 1
            }
        );
        assert!(observed.recovery.bytes.is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn preserved_files(root: &Path) -> Vec<PathBuf> {
        fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("note.invalid-")
            })
            .collect()
    }

    #[test]
    fn preserving_moves_the_unchanged_note_aside() {
        let (root, files) = fixture("preserve-ok");
        fs::write(root.join(NOTE_NAME), b"\xff").unwrap();
        let outcome = files
            .preserve_canonical(PreserveExpectation::Content(hash_bytes(b"\xff")))
            .unwrap();
        assert_eq!(outcome, PreserveOutcome::Preserved);
        assert!(!root.join(NOTE_NAME).exists());
        assert_eq!(preserved_files(&root).len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_note_changed_before_the_check_stays_where_it_is() {
        let (root, files) = fixture("preserve-changed");
        fs::write(root.join(NOTE_NAME), b"new").unwrap();
        let outcome = files
            .preserve_canonical(PreserveExpectation::Content(hash_bytes(b"\xff")))
            .unwrap();
        assert!(matches!(outcome, PreserveOutcome::Changed(_)));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"new");
        assert!(preserved_files(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_note_replaced_after_the_check_is_moved_back() {
        let (root, files) = fixture("preserve-race");
        fs::write(root.join(NOTE_NAME), b"\xff").unwrap();
        let outcome = files
            .preserve_canonical_with(PreserveExpectation::Content(hash_bytes(b"\xff")), |_| {
                let replacement = root.join("replacement");
                fs::write(&replacement, b"user").unwrap();
                fs::rename(&replacement, root.join(NOTE_NAME)).unwrap();
            })
            .unwrap();
        assert!(matches!(outcome, PreserveOutcome::Changed(_)));
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"user");
        assert!(preserved_files(&root).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_move_back_reports_where_the_file_is() {
        let (root, files) = fixture("preserve-displaced");
        fs::write(root.join(NOTE_NAME), b"\xff").unwrap();
        fs::write(root.join("note.invalid-held.md"), b"user").unwrap();
        let _wrong =
            FileObservation::of(&files.directory.open_read("note.invalid-held.md").unwrap())
                .unwrap();
        // The canonical name is taken again by the time the wrong file would go back.
        fs::write(root.join(NOTE_NAME), b"later").unwrap();
        let failure = files
            .confirm_preserved(
                "note.invalid-held.md",
                &FileObservation::of(&files.directory.open_read(NOTE_NAME).unwrap()).unwrap(),
                hash_bytes(b"\xff"),
            )
            .unwrap_err();
        assert!(matches!(
            failure,
            NoteStorageError::PreserveDisplaced { .. }
        ));
        assert_eq!(
            fs::read(root.join("note.invalid-held.md")).unwrap(),
            b"user"
        );
        assert_eq!(fs::read(root.join(NOTE_NAME)).unwrap(), b"later");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_too_large_note_is_unverifiable_and_never_moved() {
        let (root, files) = fixture("preserve-unverifiable");
        fs::write(root.join(NOTE_NAME), b"x").unwrap();
        let outcome = files
            .preserve_canonical(PreserveExpectation::Unverifiable)
            .unwrap();
        assert_eq!(outcome, PreserveOutcome::Unverifiable);
        assert!(root.join(NOTE_NAME).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_link_at_the_canonical_name_is_not_moved() {
        let (root, files) = fixture("preserve-link");
        fs::write(root.join("elsewhere"), b"\xff").unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), root.join(NOTE_NAME)).unwrap();
        let failure = files
            .preserve_canonical(PreserveExpectation::Content(hash_bytes(b"\xff")))
            .unwrap_err();
        assert!(matches!(failure, NoteStorageError::PreserveCanonical(_)));
        assert!(fs::symlink_metadata(root.join(NOTE_NAME)).is_ok());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unusable_temporary_evidence_is_kept_under_its_own_name() {
        let (root, files) = fixture("quarantine");
        fs::write(root.join(TEMPORARY_NAME), b"\xff").unwrap();
        let preserved = files.quarantine_temporary().unwrap();
        assert_eq!(fs::read(&preserved).unwrap(), b"\xff");
        assert!(!root.join(TEMPORARY_NAME).exists());
        assert!(
            preserved
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("note.invalid-tmp-")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removing_an_absent_temporary_succeeds() {
        let (root, files) = fixture("remove-absent");
        files.remove_temporary().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
