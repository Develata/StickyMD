//! Typed errors for the StickyMD core.
//!
//! plan_ref: docs/plan/05_document_persistence.md#failure-paths一级内容汇总
//!
//! Phase 1 skeleton: only the error categories needed to shape future
//! persistence and edit contracts. No runtime behavior yet.

/// Errors produced by text edit operations.
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("edit range is not on UTF-8 character boundaries")]
    NotCharBoundary,
    #[error("edit range is out of bounds")]
    OutOfBounds,
    #[error("edit range start is greater than its end")]
    InvalidRange,
}

/// Errors produced by persistence operations.
///
/// Save failures must never be swallowed silently: the caller is required
/// to surface them and keep the in-memory document intact.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("program directory is not writable")]
    DirectoryNotWritable,
    #[error("I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("content is not valid UTF-8")]
    InvalidUtf8,
    #[error("atomic replace failed and no safe fallback applied")]
    ReplaceFailed,
}

/// Errors produced when acknowledging a completed persist.
///
/// `saved_generation` must never exceed the generation actually persisted
/// (core invariant #7), so an ack referencing an unknown generation is rejected.
#[derive(Debug, thiserror::Error)]
pub enum PersistAckError {
    #[error("acknowledged generation is ahead of the current document generation")]
    AheadOfDocument,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_typed_and_displayable() {
        let e = PersistError::InvalidUtf8;
        assert!(e.to_string().contains("UTF-8"));
        let io: PersistError = std::io::Error::other("disk").into();
        assert!(matches!(io, PersistError::Io(_)));
    }
}
