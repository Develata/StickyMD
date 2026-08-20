//! Typed failures at the canonical document boundary.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#core-invariants

use crate::Generation;

/// Fail-closed document mutation errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DocumentError {
    #[error("edit range start is greater than its end")]
    InvalidRange,
    #[error("edit range is outside the canonical document")]
    RangeOutOfBounds,
    #[error("edit range is not on UTF-8 character boundaries")]
    InvalidCharBoundary,
    #[error("cursor or selection is not valid for the relevant document state")]
    InvalidTextPosition,
    #[error("edit expected generation {expected}, but the document is at {current}")]
    StaleEdit {
        expected: Generation,
        current: Generation,
    },
    #[error("the text to be replaced no longer matches the recorded delta")]
    DeletedTextMismatch,
    #[error("document generation space is exhausted")]
    GenerationExhausted,
    #[error("persisted generation is ahead of the current document")]
    InvalidPersistedGeneration,
    #[error("undo history is empty")]
    UndoUnavailable,
    #[error("redo history is empty")]
    RedoUnavailable,
}
