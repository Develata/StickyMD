//! Pure external-fact decision table.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{DiskFingerprint, ExternalFileFact, ExternalFileState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalDecision {
    IgnoreKnown,
    ReloadClean(ExternalFileFact),
    EnterConflict(ExternalFileState),
    RecreateMissing,
}

pub fn decide_external_change(
    dirty: bool,
    known: Option<DiskFingerprint>,
    observed: ExternalFileState,
) -> ExternalDecision {
    let fingerprint = match &observed {
        ExternalFileState::Present(fact) => Some(fact.fingerprint),
        ExternalFileState::InvalidUtf8 { fingerprint } => Some(*fingerprint),
        ExternalFileState::Missing | ExternalFileState::TooLarge { .. } => None,
    };
    if fingerprint.is_some() && fingerprint == known {
        return ExternalDecision::IgnoreKnown;
    }
    match observed {
        ExternalFileState::Missing => ExternalDecision::RecreateMissing,
        ExternalFileState::Present(fact) if !dirty => ExternalDecision::ReloadClean(fact),
        other => ExternalDecision::EnterConflict(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_core::{LineEnding, hash_bytes};

    fn external(text: &str) -> ExternalFileFact {
        ExternalFileFact {
            fingerprint: hash_bytes(text.as_bytes()),
            text: text.into(),
            line_ending: LineEnding::Lf,
            durable_len: text.len(),
        }
    }

    #[test]
    fn known_self_write_is_ignored_by_hash() {
        let fact = external("same");
        assert_eq!(
            decide_external_change(
                false,
                Some(fact.fingerprint),
                ExternalFileState::Present(fact)
            ),
            ExternalDecision::IgnoreKnown
        );
    }

    #[test]
    fn clean_external_change_reloads_but_dirty_change_conflicts() {
        let fact = external("external");
        assert!(matches!(
            decide_external_change(false, None, ExternalFileState::Present(fact.clone())),
            ExternalDecision::ReloadClean(_)
        ));
        assert!(matches!(
            decide_external_change(true, None, ExternalFileState::Present(fact)),
            ExternalDecision::EnterConflict(_)
        ));
    }

    #[test]
    fn missing_recreates_and_invalid_utf8_never_reloads() {
        assert_eq!(
            decide_external_change(false, None, ExternalFileState::Missing),
            ExternalDecision::RecreateMissing
        );
        assert!(matches!(
            decide_external_change(
                false,
                None,
                ExternalFileState::InvalidUtf8 {
                    fingerprint: hash_bytes(&[0xff])
                }
            ),
            ExternalDecision::EnterConflict(_)
        ));
    }
}
