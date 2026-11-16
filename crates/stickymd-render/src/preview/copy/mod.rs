//! What Ctrl+C copies from a Preview selection (plan 06 #preview-copy-semantics): an object's own
//! text when the selection lies inside one (rule 1), otherwise slices of the frame's own source
//! snapshot (rules 2 and 3), with untrusted positions handled conservatively (rule 6).
//!
//! The map is built once per layout and shared by every viewport frame of it; copying resolves
//! each endpoint with binary searches over it. Code blocks are mapped line by line, which scans
//! the one block an endpoint is in.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

mod display;
mod endpoint;
mod outline;
mod structures;
#[cfg(test)]
mod tests;

use std::ops::Range;
use std::sync::Arc;

pub(crate) use display::SelectionText;
use display::{LeafText, Piece};
pub(crate) use outline::{CopyOutline, OutlineBuilder, OutlineKind};

use endpoint::Endpoint;

/// The source and structure facts of one layout that copying reads. See `SelectionText::finish`.
#[derive(Debug, Clone)]
pub(crate) struct CopyMap {
    source: Arc<str>,
    outline: Arc<CopyOutline>,
    /// In display order and disjoint.
    pieces: Box<[Piece]>,
    formulas: Box<[Arc<str>]>,
    /// One per render block, in display order and disjoint.
    leaves: Box<[LeafText]>,
    /// Per outline block: its first visible display position and the end of its last visible
    /// byte, or `(u32::MAX, 0)`.
    visible: Box<[(u32, u32)]>,
    consistent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PreviewCopyError {
    #[error("the Preview selection could not be mapped to its source")]
    Unmapped,
}

/// The result of resolving a selection, before any text is copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CopySource<'a> {
    /// Rule 1: an object's own text.
    Object(&'a str),
    /// Rules 2 and 3: ranges of the source snapshot, increasing, disjoint and non-adjacent, each
    /// on UTF-8 boundaries. Empty when nothing selected maps to source.
    Slices(Vec<Range<usize>>),
}

impl CopyMap {
    /// A map for display text with no source behind it, as unit tests of the selection index
    /// build: every copy fails closed.
    #[cfg(test)]
    pub(crate) fn unmapped() -> Self {
        Self {
            source: Arc::from(""),
            outline: Arc::default(),
            pieces: Box::default(),
            formulas: Box::default(),
            leaves: Box::default(),
            visible: Box::default(),
            consistent: false,
        }
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    /// Resolves the display-text `selection` of `text`, the display text this map was built with.
    pub(crate) fn resolve<'a>(
        &'a self,
        text: &'a str,
        selection: Range<usize>,
    ) -> Result<CopySource<'a>, PreviewCopyError> {
        if !self.consistent
            || selection.start > selection.end
            || !text.is_char_boundary(selection.start)
            || !text.is_char_boundary(selection.end)
        {
            return Err(PreviewCopyError::Unmapped);
        }
        if selection.is_empty() {
            return Ok(CopySource::Slices(Vec::new()));
        }
        if let Some(object) = self.object_text(text, &selection) {
            return Ok(CopySource::Object(object));
        }
        let (Some(start), Some(end)) = (
            self.start_endpoint(text, selection.start)?,
            self.end_endpoint(text, selection.end)?,
        ) else {
            return Ok(CopySource::Slices(Vec::new()));
        };
        let (start, end) = self.settle(start, end)?;
        self.slices(start, end).map(CopySource::Slices)
    }

    /// Rule 1: a selection inside one object's display text copies the object's own text: a
    /// formula's TeX without delimiters, or the selected code, Raw HTML or alt text.
    fn object_text<'a>(&'a self, text: &'a str, selection: &Range<usize>) -> Option<&'a str> {
        let piece = self.piece_at(selection.start)?;
        if !piece.kind.is_object() || selection.end > piece.end as usize {
            return None;
        }
        match piece.kind {
            display::PieceKind::Formula(index) => {
                self.formulas.get(index as usize).map(|tex| &**tex)
            }
            _ => text.get(selection.clone()),
        }
    }

    /// The piece holding display byte `byte`. O(log pieces).
    fn piece_at(&self, byte: usize) -> Option<Piece> {
        let after = self
            .pieces
            .partition_point(|piece| piece.start as usize <= byte);
        let piece = *self.pieces.get(after.checked_sub(1)?)?;
        (byte < piece.end as usize).then_some(piece)
    }

    /// Assembles the final ranges: the open fence line of a fenced code block the start cuts,
    /// the main slice, and the rest of the line plus the close fence line of one the end cuts
    /// (rule 3). Every range is checked, so a bad mapping fails closed (rule 6).
    fn slices(
        &self,
        start: Endpoint,
        end: Endpoint,
    ) -> Result<Vec<Range<usize>>, PreviewCopyError> {
        let (from, to) = (start.at(), end.at());
        if from >= to {
            return Ok(Vec::new());
        }
        let mut ranges = Vec::with_capacity(4);
        if let Some(fence) = start.fence() {
            ranges.push(fence.open_line);
        }
        ranges.push(from..to);
        if let Some(fence) = end.fence() {
            if !self.source[..to].ends_with('\n')
                && let Some(newline) = self.source[to..fence.close_line.start].find('\n')
            {
                ranges.push(to + newline..to + newline + 1);
            }
            ranges.push(fence.close_line);
        }
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(ranges.len());
        for range in ranges.into_iter().filter(|range| !range.is_empty()) {
            if self.source.get(range.clone()).is_none() {
                return Err(PreviewCopyError::Unmapped);
            }
            match merged.last_mut() {
                Some(last) if last.end == range.start => last.end = range.end,
                Some(last) if last.end > range.start => return Err(PreviewCopyError::Unmapped),
                _ => merged.push(range),
            }
        }
        Ok(merged)
    }
}

/// The open and close fence lines of a closed fenced code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fence {
    /// From the block's start through the open fence line's newline.
    pub open_line: Range<usize>,
    /// From the close fence line's start (container prefix included) to the block's end.
    pub close_line: Range<usize>,
}

/// The fence lines of the fenced code block at `block`, or `None` when it has no close fence;
/// then a cut through it copies a plain slice (rule 3). The open line must be a run of at least
/// three backticks or tildes after indentation; the last line must end in a run of the same
/// character at least as long, with only indentation and quote markers before it.
pub(crate) fn fence(source: &str, block: Range<usize>) -> Option<Fence> {
    let text = source.get(block.clone())?;
    let open_end = text.find('\n')? + 1;
    let open = text[..open_end].trim_start_matches([' ', '\t']);
    let marker = open
        .chars()
        .next()
        .filter(|&mark| mark == '`' || mark == '~')?;
    let length = open.chars().take_while(|&mark| mark == marker).count();
    let close_start = text.trim_end_matches('\n').rfind('\n')? + 1;
    if length < 3 || close_start < open_end {
        return None;
    }
    let close = text[close_start..].trim_end();
    let run = close
        .chars()
        .rev()
        .take_while(|&mark| mark == marker)
        .count();
    let prefix = &close[..close.len() - run];
    (run >= length && prefix.chars().all(|mark| matches!(mark, ' ' | '\t' | '>'))).then(|| Fence {
        open_line: block.start..block.start + open_end,
        close_line: block.start + close_start..block.end,
    })
}
