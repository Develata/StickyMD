//! Caret / selection snapshot carried alongside edits.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-语义
//!
//! A `CursorSnapshot` records where the caret (and optional selection) sits at a
//! given document generation. It is presentation-adjacent data: it is *not* document
//! authority, but undo/redo must restore it so a whole IME commit (or any edit)
//! returns the caret to the right place.

use std::ops::Range;

use crate::generation::Generation;

/// Caret position + optional selection at a specific document generation.
///
/// All offsets are **byte** offsets into the internal UTF-8 text and must fall on
/// character boundaries; `DocumentState` validates this before accepting a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorSnapshot {
    /// Caret byte offset.
    pub byte_offset: usize,
    /// Optional selection as a byte range (empty selections use `None`).
    pub selection: Option<Range<usize>>,
    /// The document generation this snapshot refers to.
    pub generation: Generation,
}

impl CursorSnapshot {
    /// A caret-only snapshot (no selection).
    pub fn caret(byte_offset: usize, generation: Generation) -> Self {
        Self {
            byte_offset,
            selection: None,
            generation,
        }
    }

    /// True if every offset is in range and on a UTF-8 char boundary of `text`.
    pub fn is_valid_for(&self, text: &str) -> bool {
        if !in_bounds_on_boundary(text, self.byte_offset) {
            return false;
        }
        if let Some(sel) = &self.selection {
            if sel.start > sel.end {
                return false;
            }
            return in_bounds_on_boundary(text, sel.start) && in_bounds_on_boundary(text, sel.end);
        }
        true
    }
}

fn in_bounds_on_boundary(text: &str, offset: usize) -> bool {
    offset <= text.len() && text.is_char_boundary(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_snapshot_validates_boundaries() {
        let text = "héllo"; // 'é' is 2 bytes
        let g = Generation::initial();
        assert!(CursorSnapshot::caret(0, g).is_valid_for(text));
        assert!(CursorSnapshot::caret(1, g).is_valid_for(text)); // after 'h'
        assert!(!CursorSnapshot::caret(2, g).is_valid_for(text)); // mid 'é'
        assert!(CursorSnapshot::caret(3, g).is_valid_for(text)); // after 'é'
        assert!(!CursorSnapshot::caret(text.len() + 1, g).is_valid_for(text));
    }

    #[test]
    fn selection_snapshot_validates_range() {
        let text = "abc";
        let g = Generation::initial();
        let ok = CursorSnapshot {
            byte_offset: 3,
            selection: Some(0..2),
            generation: g,
        };
        assert!(ok.is_valid_for(text));
        // Intentionally reversed range: it must be rejected, not iterated.
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = CursorSnapshot {
            byte_offset: 0,
            selection: Some(2..0),
            generation: g,
        };
        assert!(!reversed.is_valid_for(text));
    }
}
