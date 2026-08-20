//! Text-only clipboard capability contract.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ClipboardError {
    #[error("clipboard is unavailable: {0}")]
    Unavailable(String),
}

pub trait ClipboardPort {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}
