//! Immutable document snapshots for worker and projection boundaries.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#document-snapshot

use std::sync::Arc;

use crate::{Generation, LineEnding};

/// Immutable, non-authoritative projection of canonical document state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSnapshot {
    pub text: Arc<str>,
    pub generation: Generation,
    pub line_ending: LineEnding,
}
