//! Pure persistence facts and reconciliation models.
//!
//! plan_ref: docs/plan/05_document_persistence.md#persistence-authority
//!
//! This module never opens files. It decodes already-read durable bytes into an
//! immutable external fact, classifies startup recovery evidence, and carries
//! conflict state. `DocumentState` remains the sole runtime text authority.

use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::{Generation, Hash32, LineEnding};

/// Durable byte fingerprint used by optimistic concurrency checks.
pub type DiskFingerprint = Hash32;

/// Compute the SHA-256 digest of the exact durable bytes.
pub fn hash_bytes(bytes: &[u8]) -> DiskFingerprint {
    DiskFingerprint::new(Sha256::digest(bytes).into())
}

/// A validated note decoded at the persistence boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedDocument {
    pub text: String,
    pub line_ending: LineEnding,
    pub fingerprint: DiskFingerprint,
    pub durable_len: usize,
}

impl LoadedDocument {
    /// Decode UTF-8 (optionally BOM-prefixed), detect line endings, and normalize
    /// CRLF to the runtime representation. Invalid UTF-8 is rejected, never lost.
    pub fn from_durable_bytes(bytes: &[u8]) -> Result<Self, NoteDecodeError> {
        let without_bom = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        let durable = std::str::from_utf8(without_bom).map_err(|_| NoteDecodeError::InvalidUtf8)?;
        let line_ending = LineEnding::detect(durable);
        Ok(Self {
            text: LineEnding::to_internal(durable),
            line_ending,
            fingerprint: hash_bytes(bytes),
            durable_len: bytes.len(),
        })
    }

    /// Encode canonical runtime text as UTF-8 without BOM using recorded line endings.
    pub fn encode_runtime(text: &str, line_ending: LineEnding) -> Vec<u8> {
        line_ending.apply(text).into_bytes()
    }

    pub fn into_external_fact(self) -> ExternalFileFact {
        ExternalFileFact {
            fingerprint: self.fingerprint,
            text: self.text,
            line_ending: self.line_ending,
            durable_len: self.durable_len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NoteDecodeError {
    #[error("note bytes are not valid UTF-8")]
    InvalidUtf8,
}

/// Latest validated content observed outside the runtime authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFileFact {
    pub fingerprint: DiskFingerprint,
    pub text: String,
    pub line_ending: LineEnding,
    pub durable_len: usize,
}

/// Result of observing the durable note path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalFileState {
    Present(ExternalFileFact),
    Missing,
    InvalidUtf8 { fingerprint: DiskFingerprint },
    TooLarge { bytes: u64 },
}

/// Explicit conflict between dirty runtime text and a newer durable fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConflict {
    pub external: ExternalFileState,
    pub detected_at_generation: Generation,
    pub previous_disk_fingerprint: Option<DiskFingerprint>,
}

/// One durable file observation used during startup recovery inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryInspection {
    pub bytes: Vec<u8>,
    pub modified: Option<SystemTime>,
}

/// Recovery evidence held until the user explicitly resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCandidate {
    pub canonical: Option<LoadedDocument>,
    pub temporary: LoadedDocument,
    pub temporary_is_newer: bool,
}

/// Pure startup classification; adapters decide only how to read or delete files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDisposition {
    NoTemporary,
    RedundantTemporary,
    StaleTemporary,
    InvalidTemporary,
    Candidate(RecoveryCandidate),
}

/// Inspect canonical and temporary durable observations without mutating either.
pub fn inspect_recovery(
    canonical: Option<&RecoveryInspection>,
    temporary: Option<&RecoveryInspection>,
) -> RecoveryDisposition {
    let Some(temporary) = temporary else {
        return RecoveryDisposition::NoTemporary;
    };
    let Ok(decoded_temporary) = LoadedDocument::from_durable_bytes(&temporary.bytes) else {
        return RecoveryDisposition::InvalidTemporary;
    };

    let decoded_canonical =
        canonical.and_then(|value| LoadedDocument::from_durable_bytes(&value.bytes).ok());
    if decoded_canonical
        .as_ref()
        .is_some_and(|value| value.fingerprint == decoded_temporary.fingerprint)
    {
        return RecoveryDisposition::RedundantTemporary;
    }

    if let (Some(_), Some(canonical_modified), Some(temporary_modified)) = (
        decoded_canonical.as_ref(),
        canonical.and_then(|value| value.modified),
        temporary.modified,
    ) && temporary_modified <= canonical_modified
    {
        return RecoveryDisposition::StaleTemporary;
    }

    let temporary_is_newer = match (canonical.and_then(|v| v.modified), temporary.modified) {
        (Some(canonical), Some(temporary)) => temporary > canonical,
        (None, _) => true,
        _ => false,
    };
    RecoveryDisposition::Candidate(RecoveryCandidate {
        canonical: decoded_canonical,
        temporary: decoded_temporary,
        temporary_is_newer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn observed(bytes: &[u8], seconds: u64) -> RecoveryInspection {
        RecoveryInspection {
            bytes: bytes.to_vec(),
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
        }
    }

    #[test]
    fn durable_decode_accepts_bom_and_emits_bomless_runtime_text() {
        let loaded = LoadedDocument::from_durable_bytes(b"\xef\xbb\xbfA\r\nB").unwrap();
        assert_eq!(loaded.text, "A\nB");
        assert_eq!(loaded.line_ending, LineEnding::Crlf);
        assert_eq!(
            LoadedDocument::encode_runtime(&loaded.text, loaded.line_ending),
            b"A\r\nB"
        );
    }

    #[test]
    fn durable_decode_rejects_invalid_utf8() {
        assert_eq!(
            LoadedDocument::from_durable_bytes(&[0xff]),
            Err(NoteDecodeError::InvalidUtf8)
        );
    }

    #[test]
    fn mixed_newline_majority_and_isolated_cr_are_preserved() {
        let crlf = LoadedDocument::from_durable_bytes(b"a\r\nb\r\nc\nd\r").unwrap();
        assert_eq!(crlf.line_ending, LineEnding::Crlf);
        assert_eq!(crlf.text, "a\nb\nc\nd\r");
        let lf = LoadedDocument::from_durable_bytes(b"a\nb\nc\r\nd").unwrap();
        assert_eq!(lf.line_ending, LineEnding::Lf);
    }

    #[test]
    fn recovery_classifies_same_invalid_and_newer_temporary() {
        let note = observed(b"old", 1);
        assert_eq!(
            inspect_recovery(Some(&note), Some(&observed(b"old", 2))),
            RecoveryDisposition::RedundantTemporary
        );
        assert_eq!(
            inspect_recovery(Some(&note), Some(&observed(&[0xff], 2))),
            RecoveryDisposition::InvalidTemporary
        );
        let RecoveryDisposition::Candidate(candidate) =
            inspect_recovery(Some(&note), Some(&observed(b"new", 2)))
        else {
            panic!("expected recovery candidate")
        };
        assert!(candidate.temporary_is_newer);
        assert_eq!(candidate.temporary.text, "new");
    }

    #[test]
    fn older_different_temporary_is_stale_not_a_recovery_choice() {
        assert_eq!(
            inspect_recovery(
                Some(&observed(b"canonical", 2)),
                Some(&observed(b"old temp", 1))
            ),
            RecoveryDisposition::StaleTemporary
        );
    }

    #[test]
    fn valid_temporary_survives_an_invalid_newer_canonical() {
        assert!(matches!(
            inspect_recovery(
                Some(&observed(&[0xff], 2)),
                Some(&observed(b"recoverable", 1))
            ),
            RecoveryDisposition::Candidate(_)
        ));
    }

    #[test]
    fn missing_note_with_valid_temporary_is_recoverable() {
        let RecoveryDisposition::Candidate(candidate) =
            inspect_recovery(None, Some(&observed("临时".as_bytes(), 1)))
        else {
            panic!("expected recovery candidate")
        };
        assert!(candidate.canonical.is_none());
        assert!(candidate.temporary_is_newer);
    }
}
