//! Platform-independent byte-offset selection and cursor snapshots.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#selection-caret

use std::ops::Range;

/// UTF-8 byte position in the canonical document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextPosition {
    pub byte: usize,
}

impl TextPosition {
    pub const fn new(byte: usize) -> Self {
        Self { byte }
    }
}

/// Direction-preserving selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: TextPosition,
    pub active: TextPosition,
}

impl Selection {
    pub const fn caret(byte: usize) -> Self {
        let position = TextPosition::new(byte);
        Self {
            anchor: position,
            active: position,
        }
    }

    pub const fn new(anchor: usize, active: usize) -> Self {
        Self {
            anchor: TextPosition::new(anchor),
            active: TextPosition::new(active),
        }
    }

    pub const fn is_collapsed(self) -> bool {
        self.anchor.byte == self.active.byte
    }

    pub const fn start(self) -> usize {
        if self.anchor.byte < self.active.byte {
            self.anchor.byte
        } else {
            self.active.byte
        }
    }

    pub const fn end(self) -> usize {
        if self.anchor.byte > self.active.byte {
            self.anchor.byte
        } else {
            self.active.byte
        }
    }

    pub fn normalized_range(self) -> Range<usize> {
        self.start()..self.end()
    }

    pub(crate) fn is_valid_for(self, text: &str) -> bool {
        position_is_valid(text, self.anchor.byte) && position_is_valid(text, self.active.byte)
    }
}

/// Selection state restored by edit, undo, and redo outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorSnapshot {
    pub selection: Selection,
}

impl CursorSnapshot {
    pub const fn caret(byte: usize) -> Self {
        Self {
            selection: Selection::caret(byte),
        }
    }

    pub const fn new(selection: Selection) -> Self {
        Self { selection }
    }

    pub const fn active_byte(self) -> usize {
        self.selection.active.byte
    }

    pub(crate) fn is_valid_for(self, text: &str) -> bool {
        self.selection.is_valid_for(text)
    }
}

pub(crate) fn position_is_valid(text: &str, byte: usize) -> bool {
    byte <= text.len() && text.is_char_boundary(byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_selection_preserves_direction_and_normalizes_range() {
        let selection = Selection::new(7, 2);
        assert_eq!(selection.anchor.byte, 7);
        assert_eq!(selection.active.byte, 2);
        assert_eq!(selection.normalized_range(), 2..7);
        assert!(!selection.is_collapsed());
    }

    #[test]
    fn positions_reject_mid_codepoint_offsets() {
        let text = "a中🙂";
        assert!(Selection::new(0, text.len()).is_valid_for(text));
        assert!(!Selection::new(0, 2).is_valid_for(text));
    }
}
