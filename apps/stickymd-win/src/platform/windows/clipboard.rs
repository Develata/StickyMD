//! Text-only Windows clipboard adapter.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use crate::flow::{ClipboardError, ClipboardPort};

pub struct ArboardClipboard {
    inner: Option<arboard::Clipboard>,
}

impl ArboardClipboard {
    pub const fn new() -> Self {
        Self { inner: None }
    }

    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, ClipboardError> {
        if self.inner.is_none() {
            self.inner = Some(
                arboard::Clipboard::new()
                    .map_err(|error| ClipboardError::Unavailable(error.to_string()))?,
            );
        }
        self.inner
            .as_mut()
            .ok_or_else(|| ClipboardError::Unavailable("initialization failed".to_owned()))
    }
}

impl Default for ArboardClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardPort for ArboardClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        match self.clipboard()?.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(ClipboardError::Unavailable(error.to_string())),
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.clipboard()?
            .set_text(text.to_owned())
            .map_err(|error| ClipboardError::Unavailable(error.to_string()))
    }
}
