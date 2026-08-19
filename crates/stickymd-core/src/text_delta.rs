//! Text delta: the single unit of document mutation.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
//!
//! A `TextDelta` replaces a byte range of the canonical UTF-8 text with new text.
//! `TextDelta.range` **must** fall on UTF-8 character boundaries. One IME commit and
//! one image paste each produce exactly one delta; the editor layer never applies a
//! delta that crosses a char boundary.

use std::ops::Range;

use crate::error::EditError;

/// A single mutation: replace `range` (byte offsets) with `replacement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDelta {
    /// Byte range of the text to be replaced. Must lie on char boundaries.
    pub range: Range<usize>,
    /// Text inserted in place of `range`.
    pub replacement: String,
}

impl TextDelta {
    /// Construct a delta; does not validate until [`TextDelta::validate`].
    pub fn new(range: Range<usize>, replacement: impl Into<String>) -> Self {
        Self {
            range,
            replacement: replacement.into(),
        }
    }

    /// A pure insertion at `offset` (zero-width range).
    pub fn insert(offset: usize, replacement: impl Into<String>) -> Self {
        Self::new(offset..offset, replacement)
    }

    /// Validate this delta against `text`. Returns the removed byte slice on
    /// success so callers (e.g. undo) can reuse it without re-reading.
    pub fn validate<'a>(&self, text: &'a str) -> Result<&'a str, EditError> {
        let len = text.len();
        if self.range.start > self.range.end {
            return Err(EditError::InvalidRange);
        }
        if self.range.end > len {
            return Err(EditError::OutOfBounds);
        }
        if !text.is_char_boundary(self.range.start) || !text.is_char_boundary(self.range.end) {
            return Err(EditError::NotCharBoundary);
        }
        Ok(&text[self.range.clone()])
    }

    /// True when this delta is a pure insertion (removes nothing).
    pub fn is_insert(&self) -> bool {
        self.range.start == self.range.end
    }

    /// The caret position immediately after applying this delta.
    pub fn caret_after(&self) -> usize {
        self.range.start + self.replacement.len()
    }
}

/// Classification of the input that produced a delta, used for undo grouping.
///
/// plan_ref: docs/plan/07_editor_and_ime.md#undo-分组
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// A single typed character (mergeable with an adjacent run).
    Typing,
    /// A single backward delete (mergeable with an adjacent run).
    Backspace,
    /// A single forward delete (mergeable with an adjacent run).
    Delete,
    /// One IME composition commit. Always its own undo entry.
    ImeCommit,
    /// Clipboard text paste. Always its own undo entry.
    Paste,
    /// Image paste (text reference + asset effect). Always its own entry.
    ImagePaste,
    /// Enter / newline. Always its own undo entry.
    Newline,
    /// Replacing a non-empty selection. Always its own undo entry.
    DeleteSelection,
    /// Programmatic change (external reload, restore). Not grouped.
    Programmatic,
}

impl InputKind {
    /// Whether consecutive deltas of this kind may merge into one undo entry.
    ///
    /// Only plain typing and single-step deletes merge; IME commit, paste, image
    /// paste, newline and selection replacement must stay independent.
    pub const fn is_mergeable(self) -> bool {
        matches!(
            self,
            InputKind::Typing | InputKind::Backspace | InputKind::Delete
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_delta_is_valid_and_reports_caret() {
        let text = "hello";
        let d = TextDelta::insert(5, "!");
        assert!(d.validate(text).is_ok());
        assert!(d.is_insert());
        assert_eq!(d.caret_after(), 6);
    }

    #[test]
    fn delta_rejects_mid_codepoint_range() {
        let text = "héllo";
        // byte 2 is inside 'é'
        let d = TextDelta::new(1..2, "x");
        assert!(matches!(d.validate(text), Err(EditError::NotCharBoundary)));
    }

    #[test]
    fn delta_rejects_out_of_bounds_and_reversed() {
        let text = "abc";
        assert!(matches!(
            TextDelta::new(0..10, "").validate(text),
            Err(EditError::OutOfBounds)
        ));
        // Intentionally reversed range: it must be rejected, not iterated.
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = TextDelta::new(2..1, "");
        assert!(matches!(
            reversed.validate(text),
            Err(EditError::InvalidRange)
        ));
    }

    #[test]
    fn validate_returns_removed_slice() {
        let text = "abcdef";
        let d = TextDelta::new(2..4, "Z");
        assert_eq!(d.validate(text).unwrap(), "cd");
    }

    #[test]
    fn mergeable_kinds() {
        assert!(InputKind::Typing.is_mergeable());
        assert!(InputKind::Backspace.is_mergeable());
        assert!(InputKind::Delete.is_mergeable());
        assert!(!InputKind::ImeCommit.is_mergeable());
        assert!(!InputKind::Paste.is_mergeable());
        assert!(!InputKind::ImagePaste.is_mergeable());
        assert!(!InputKind::Newline.is_mergeable());
        assert!(!InputKind::DeleteSelection.is_mergeable());
        assert!(!InputKind::Programmatic.is_mergeable());
    }
}
