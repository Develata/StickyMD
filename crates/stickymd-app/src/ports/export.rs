//! The Markdown export port.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#export

use std::path::PathBuf;

use stickymd_core::{DocumentSnapshot, Generation};

use super::StorageFailure;

/// An immutable snapshot and the target the user chose in the host's dialog.
#[derive(Debug)]
pub struct ExportRequest {
    pub snapshot: DocumentSnapshot,
    pub target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCompletion {
    pub generation: Generation,
    pub target: PathBuf,
    pub copied_assets: usize,
}

pub trait ExportSink: Send + 'static {
    /// Publishes local assets before the Markdown that refers to them.
    fn export(&mut self, request: ExportRequest) -> Result<ExportCompletion, StorageFailure>;
}
