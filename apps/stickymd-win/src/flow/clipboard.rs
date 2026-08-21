//! Typed clipboard capability contract.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#paste

use std::path::PathBuf;
use stickymd_core::{Generation, Selection};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardPaste {
    Files(Vec<PathBuf>),
    EncodedImage(Vec<u8>),
    Dib(Vec<u8>),
    Rgba {
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    },
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAssetPaste {
    pub expected_generation: Generation,
    pub selection: Selection,
    pub timestamp_ms: u64,
    pub payload: ClipboardPaste,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    #[error("clipboard is unavailable: {0}")]
    Unavailable(String),
}

pub trait ClipboardPort {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;

    fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
        self.read_text().map(|text| text.map(ClipboardPaste::Text))
    }
}
