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

/// A host's clipboard behind one pointer. The clipboard runs at the frequency of a user's gesture,
/// so the session need not be generic over it. Every method forwards: the default `read_paste`
/// would turn a host's image paste into its text.
impl ClipboardPort for Box<dyn ClipboardPort> {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        (**self).read_text()
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        (**self).write_text(text)
    }

    fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
        (**self).read_paste()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ImageClipboard {
        written: Vec<String>,
    }

    impl ClipboardPort for ImageClipboard {
        fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
            Ok(Some("text".to_owned()))
        }

        fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
            self.written.push(text.to_owned());
            Ok(())
        }

        fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
            Ok(Some(ClipboardPaste::EncodedImage(vec![1, 2, 3])))
        }
    }

    #[test]
    fn a_boxed_clipboard_keeps_the_hosts_own_paste() {
        let mut boxed: Box<dyn ClipboardPort> = Box::new(ImageClipboard {
            written: Vec::new(),
        });
        assert_eq!(
            ClipboardPort::read_paste(&mut boxed),
            Ok(Some(ClipboardPaste::EncodedImage(vec![1, 2, 3])))
        );
        assert_eq!(
            ClipboardPort::read_text(&mut boxed),
            Ok(Some("text".to_owned()))
        );
        assert_eq!(ClipboardPort::write_text(&mut boxed, "x"), Ok(()));
    }
}
