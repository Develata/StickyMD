//! The optimistic-concurrency rule of a guarded save, independent of how a host observes the disk.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{DiskFingerprint, ExternalFileState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishVerdict {
    /// Nothing is at the canonical path and nothing was expected there.
    PublishNew,
    /// The canonical file is the one this save was based on.
    ReplaceExisting,
    /// Anything else: nothing may be published.
    Conflict,
}

/// Decides a guarded save from the final observation made right before publishing. `expected` is
/// the Durable Fingerprint the save was based on; `None` means it expected no file. A too-large
/// file has no fingerprint, so it never matches.
pub fn guarded_publish_verdict(
    observed: &ExternalFileState,
    expected: Option<DiskFingerprint>,
) -> PublishVerdict {
    let fingerprint = match observed {
        ExternalFileState::Present(fact) => Some(fact.fingerprint),
        ExternalFileState::InvalidUtf8 { fingerprint } => Some(*fingerprint),
        ExternalFileState::TooLarge { .. } => return PublishVerdict::Conflict,
        ExternalFileState::Missing => {
            return if expected.is_none() {
                PublishVerdict::PublishNew
            } else {
                PublishVerdict::Conflict
            };
        }
    };
    if expected.is_some() && fingerprint == expected {
        PublishVerdict::ReplaceExisting
    } else {
        PublishVerdict::Conflict
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_core::{ExternalFileFact, LineEnding, hash_bytes};

    fn present(text: &str) -> ExternalFileState {
        ExternalFileState::Present(ExternalFileFact {
            fingerprint: hash_bytes(text.as_bytes()),
            text: text.into(),
            line_ending: LineEnding::Lf,
            durable_len: text.len(),
        })
    }

    #[test]
    fn every_observed_state_against_every_expectation() {
        let known = hash_bytes(b"known");
        let other = hash_bytes(b"other");
        let invalid = ExternalFileState::InvalidUtf8 { fingerprint: known };
        let too_large = ExternalFileState::TooLarge { bytes: 9 };
        let rows = [
            (ExternalFileState::Missing, None, PublishVerdict::PublishNew),
            (
                ExternalFileState::Missing,
                Some(known),
                PublishVerdict::Conflict,
            ),
            (present("known"), None, PublishVerdict::Conflict),
            (
                present("known"),
                Some(known),
                PublishVerdict::ReplaceExisting,
            ),
            (present("known"), Some(other), PublishVerdict::Conflict),
            (invalid.clone(), None, PublishVerdict::Conflict),
            (
                invalid.clone(),
                Some(known),
                PublishVerdict::ReplaceExisting,
            ),
            (invalid, Some(other), PublishVerdict::Conflict),
            (too_large.clone(), None, PublishVerdict::Conflict),
            (too_large, Some(known), PublishVerdict::Conflict),
        ];
        for (observed, expected, verdict) in rows {
            assert_eq!(
                guarded_publish_verdict(&observed, expected),
                verdict,
                "{observed:?} against {expected:?}"
            );
        }
    }
}
