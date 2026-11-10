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
    /// `new_note` is the host's first-creation style, which a tie resolves to.
    pub fn from_durable_bytes(bytes: &[u8], new_note: LineEnding) -> Result<Self, NoteDecodeError> {
        let without_bom = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        let durable = std::str::from_utf8(without_bom).map_err(|_| NoteDecodeError::InvalidUtf8)?;
        let line_ending = LineEnding::detect(durable, new_note);
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

/// What a recovery restore must still find at the canonical path before moving it aside: the
/// full-content fingerprint of the invalid canonical it observed, or `Unverifiable` for a
/// too-large canonical, which is never read and so never moved automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreserveExpectation {
    Content(DiskFingerprint),
    Unverifiable,
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
    /// The host's first-creation style: what a missing canonical note is recreated with.
    pub new_note: LineEnding,
}

/// Pure startup classification; adapters decide only how to read or delete files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDisposition {
    NoTemporary,
    RedundantTemporary,
    InvalidTemporary,
    Candidate(RecoveryCandidate),
}

/// Inspect canonical and temporary durable observations without mutating either. `new_note` is
/// the host's first-creation style.
pub fn inspect_recovery(
    canonical: Option<&RecoveryInspection>,
    temporary: Option<&RecoveryInspection>,
    new_note: LineEnding,
) -> RecoveryDisposition {
    let Some(temporary) = temporary else {
        return RecoveryDisposition::NoTemporary;
    };
    let Ok(decoded_temporary) = LoadedDocument::from_durable_bytes(&temporary.bytes, new_note)
    else {
        return RecoveryDisposition::InvalidTemporary;
    };

    let decoded_canonical =
        canonical.and_then(|value| LoadedDocument::from_durable_bytes(&value.bytes, new_note).ok());
    // An empty temporary was interrupted before its bytes were written: beside a usable note it
    // has nothing to offer, and restoring it would only empty the note.
    if decoded_canonical.as_ref().is_some_and(|value| {
        value.fingerprint == decoded_temporary.fingerprint || temporary.bytes.is_empty()
    }) {
        return RecoveryDisposition::RedundantTemporary;
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
        new_note,
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
    fn an_empty_temporary_has_nothing_to_offer_beside_a_usable_note() {
        assert_eq!(
            inspect_recovery(
                Some(&observed(b"text", 1)),
                Some(&observed(b"", 2)),
                LineEnding::Crlf
            ),
            RecoveryDisposition::RedundantTemporary
        );
    }

    #[test]
    fn an_empty_temporary_stays_a_choice_without_a_usable_note() {
        let invalid = observed(&[0xff], 1);
        for canonical in [None, Some(&invalid)] {
            assert!(
                matches!(
                    inspect_recovery(canonical, Some(&observed(b"", 2)), LineEnding::Crlf),
                    RecoveryDisposition::Candidate(_)
                ),
                "{canonical:?}"
            );
        }
    }

    #[test]
    fn durable_decode_accepts_bom_and_emits_bomless_runtime_text() {
        let loaded =
            LoadedDocument::from_durable_bytes(b"\xef\xbb\xbfA\r\nB", LineEnding::Crlf).unwrap();
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
            LoadedDocument::from_durable_bytes(&[0xff], LineEnding::Crlf),
            Err(NoteDecodeError::InvalidUtf8)
        );
    }

    #[test]
    fn mixed_newline_majority_and_isolated_cr_are_preserved() {
        let crlf =
            LoadedDocument::from_durable_bytes(b"a\r\nb\r\nc\nd\r", LineEnding::Crlf).unwrap();
        assert_eq!(crlf.line_ending, LineEnding::Crlf);
        assert_eq!(crlf.text, "a\nb\nc\nd\r");
        let lf = LoadedDocument::from_durable_bytes(b"a\nb\nc\r\nd", LineEnding::Crlf).unwrap();
        assert_eq!(lf.line_ending, LineEnding::Lf);
    }

    #[test]
    fn recovery_classifies_same_invalid_and_newer_temporary() {
        let note = observed(b"old", 1);
        assert_eq!(
            inspect_recovery(Some(&note), Some(&observed(b"old", 2)), LineEnding::Crlf),
            RecoveryDisposition::RedundantTemporary
        );
        assert_eq!(
            inspect_recovery(Some(&note), Some(&observed(&[0xff], 2)), LineEnding::Crlf),
            RecoveryDisposition::InvalidTemporary
        );
        let RecoveryDisposition::Candidate(candidate) =
            inspect_recovery(Some(&note), Some(&observed(b"new", 2)), LineEnding::Crlf)
        else {
            panic!("expected recovery candidate")
        };
        assert!(candidate.temporary_is_newer);
        assert_eq!(candidate.temporary.text, "new");
    }

    /// A valid temp whose content differs is always the user's choice, whatever the times: a
    /// failed save can leave newer text in a temp older than an external save (plan 05 recovery 3).
    #[test]
    fn a_different_temporary_is_a_recovery_choice_whatever_its_time() {
        for (canonical_time, temporary_time, newer) in [(2, 1, false), (2, 2, false), (1, 2, true)]
        {
            let RecoveryDisposition::Candidate(candidate) = inspect_recovery(
                Some(&observed(b"canonical", canonical_time)),
                Some(&observed(b"local temp", temporary_time)),
                LineEnding::Crlf,
            ) else {
                panic!("expected a recovery candidate at {canonical_time}/{temporary_time}");
            };
            assert_eq!(candidate.temporary.text, "local temp");
            assert_eq!(candidate.temporary_is_newer, newer);
        }
    }

    #[test]
    fn valid_temporary_survives_an_invalid_newer_canonical() {
        assert!(matches!(
            inspect_recovery(
                Some(&observed(&[0xff], 2)),
                Some(&observed(b"recoverable", 1)),
                LineEnding::Crlf,
            ),
            RecoveryDisposition::Candidate(_)
        ));
    }

    #[test]
    fn missing_note_with_valid_temporary_is_recoverable() {
        for new_note in [LineEnding::Crlf, LineEnding::Lf] {
            let RecoveryDisposition::Candidate(candidate) =
                inspect_recovery(None, Some(&observed("临时".as_bytes(), 1)), new_note)
            else {
                panic!("expected recovery candidate")
            };
            assert!(candidate.canonical.is_none());
            assert!(candidate.temporary_is_newer);
            // A temporary without line breaks takes, and the candidate keeps, the host's style.
            assert_eq!(candidate.temporary.line_ending, new_note);
            assert_eq!(candidate.new_note, new_note);
        }
    }

    #[test]
    fn a_body_without_line_breaks_takes_the_hosts_first_creation_style() {
        for new_note in [LineEnding::Crlf, LineEnding::Lf] {
            for bytes in [&b""[..], b"one line", b"\xef\xbb\xbfone line"] {
                let loaded = LoadedDocument::from_durable_bytes(bytes, new_note).unwrap();
                assert_eq!(loaded.line_ending, new_note, "{bytes:?}");
            }
            let tie = LoadedDocument::from_durable_bytes(b"a\r\nb\nc", new_note).unwrap();
            assert_eq!(tie.line_ending, new_note);
        }
    }
}
