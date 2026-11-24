//! The text a Preview selection addresses, recorded in the same pass with where each piece of it
//! came from: one record per span, so the copy map costs O(span) (plan 06 #preview-copy-semantics).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use std::ops::Range;
use std::sync::Arc;

use super::outline::CopyOutline;
use crate::preview::RenderSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PieceKind {
    /// Inline text. It maps byte for byte when its display text equals its source slice; otherwise
    /// an endpoint in it rounds outward to its edges (rule 6).
    Text,
    /// Inline code: byte for byte when its display text equals the text between its backtick runs.
    InlineCode,
    /// A formula, block or inline: atomic. Its TeX is `CopyMap::formulas[index]`.
    Formula(u32),
    /// An image: atomic, displayed as its alt text.
    Image,
    /// A code block or Raw HTML block: mapped line by line.
    Block,
    /// A generated list marker. Recording keeps its ordinal within the leaf; `finish` replaces it
    /// with the outline index of its list item, or `u32::MAX` when it has none.
    Marker(u32),
}

impl PieceKind {
    /// Rule 1 objects: a selection inside one copies only its content.
    pub(crate) const fn is_object(self) -> bool {
        matches!(self, Self::Formula(_) | Self::Image | Self::Block)
    }
}

/// One stretch of display text with one source range. Display offsets are `u32`: the Preview
/// parser caps a document at 5 MiB and the display text stays within a small factor of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Piece {
    pub start: u32,
    pub end: u32,
    pub source_start: u32,
    pub source_end: u32,
    pub kind: PieceKind,
}

impl Piece {
    pub(crate) const fn display(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }

    pub(crate) const fn source(self) -> Range<usize> {
        self.source_start as usize..self.source_end as usize
    }

    /// Whether Comrak gave it a usable source range. An object keeps its piece without one, since
    /// rule 1 reads only its display text or TeX; nothing maps through it to the source.
    pub(crate) const fn has_source(self) -> bool {
        self.source_start != NO_SOURCE
    }
}

/// The source offsets of a piece with no source range. `SelectionText::finish` marks a map whose
/// source reaches `u32::MAX` inconsistent, so no real offset collides with it.
const NO_SOURCE: u32 = u32::MAX;

/// The display extent of one render block: `start..content` holds its list markers and
/// `content..end` its own text. Text between leaves (the `\n` that separates them) is in none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeafText {
    pub start: u32,
    pub content: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Outside,
    Markers(u32),
    Content {
        block: bool,
    },
}

/// Builds the display text and its pieces. Layout appends to it in display order.
#[derive(Debug, Default)]
pub(crate) struct SelectionText {
    text: String,
    pieces: Vec<Piece>,
    formulas: Vec<Arc<str>>,
    leaves: Vec<LeafText>,
    phase: Phase,
}

/// The state of a `SelectionText` that a layout pass can return to (`rollback`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectionCheckpoint {
    text: usize,
    pieces: usize,
    last_piece: Option<Piece>,
    formulas: usize,
}

impl SelectionText {
    /// O(1) mark of the current end, for a pass that may be laid out again.
    pub(crate) fn checkpoint(&self) -> SelectionCheckpoint {
        SelectionCheckpoint {
            text: self.text.len(),
            pieces: self.pieces.len(),
            last_piece: self.pieces.last().copied(),
            formulas: self.formulas.len(),
        }
    }

    /// Drops everything appended since `mark`, including a merge into the piece before it.
    pub(crate) fn rollback(&mut self, mark: SelectionCheckpoint) {
        self.text.truncate(mark.text);
        self.pieces.truncate(mark.pieces);
        if let (Some(last), Some(saved)) = (self.pieces.last_mut(), mark.last_piece) {
            *last = saved;
        }
        self.formulas.truncate(mark.formulas);
    }

    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        &self.text
    }

    /// Empties it for reuse as scratch space, such as a measuring pass that records nothing.
    pub(crate) fn clear(&mut self) {
        self.text.clear();
        self.pieces.clear();
        self.formulas.clear();
        self.leaves.clear();
        self.phase = Phase::Outside;
    }

    /// Starts the next render block; its list markers come first.
    pub(crate) fn begin_leaf(&mut self) {
        let at = offset(self.text.len());
        self.leaves.push(LeafText {
            start: at,
            content: at,
            end: at,
        });
        self.phase = Phase::Markers(0);
    }

    /// The render block's own text starts here. `block` marks a code or Raw HTML block.
    pub(crate) fn begin_content(&mut self, block: bool) {
        if let Some(leaf) = self.leaves.last_mut() {
            leaf.content = offset(self.text.len());
        }
        self.phase = Phase::Content { block };
    }

    pub(crate) fn end_leaf(&mut self) {
        if let Some(leaf) = self.leaves.last_mut() {
            leaf.end = offset(self.text.len());
        }
        self.phase = Phase::Outside;
    }

    /// Appends a span's copy text and returns its display range.
    pub(crate) fn push_span(&mut self, span: &RenderSpan) -> Range<usize> {
        let start = self.text.len();
        self.text.push_str(&span.copy_text);
        let end = self.text.len();
        if start < end {
            self.record(span, start, end);
        }
        start..end
    }

    /// Appends text Preview generates (separators between blocks, cells and rows): no source.
    pub(crate) fn push_generated(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn record(&mut self, span: &RenderSpan, start: usize, end: usize) {
        let kind = match self.phase {
            Phase::Outside => return,
            Phase::Markers(ordinal) => {
                self.phase = Phase::Markers(ordinal.saturating_add(1));
                self.pieces.push(Piece {
                    start: offset(start),
                    end: offset(end),
                    source_start: NO_SOURCE,
                    source_end: NO_SOURCE,
                    kind: PieceKind::Marker(ordinal),
                });
                return;
            }
            Phase::Content { block: true } => PieceKind::Block,
            Phase::Content { block: false } => {
                if let Some(math) = &span.math {
                    self.formulas.push(Arc::clone(&math.source));
                    PieceKind::Formula(offset(self.formulas.len() - 1))
                } else if span.image.is_some() {
                    PieceKind::Image
                } else if span.style.code && !span.style.html_literal {
                    PieceKind::InlineCode
                } else {
                    PieceKind::Text
                }
            }
        };
        let (source_start, source_end) = match span.source_range {
            Some(source) => (offset(source.start), offset(source.end)),
            None if kind.is_object() => (NO_SOURCE, NO_SOURCE),
            None => return,
        };
        // Layout splits a long text or inline code span into word tokens that each carry the whole
        // span's range; they merge back here, so exactness is judged on the whole span (rule 6).
        if let Some(last) = self.pieces.last_mut()
            && last.kind == kind
            && matches!(kind, PieceKind::Text | PieceKind::InlineCode)
            && last.end == offset(start)
            && (last.source_start, last.source_end) == (source_start, source_end)
        {
            last.end = offset(end);
            return;
        }
        self.pieces.push(Piece {
            start: offset(start),
            end: offset(end),
            source_start,
            source_end,
            kind,
        });
    }

    /// The display text and its copy map. `outline` must describe the same render tree, leaf for
    /// leaf; if it does not, the map is marked inconsistent and copying fails closed.
    pub(crate) fn finish(
        self,
        source: Arc<str>,
        outline: Arc<CopyOutline>,
    ) -> (String, super::CopyMap) {
        let Self {
            text,
            mut pieces,
            formulas,
            leaves,
            phase: _,
        } = self;
        let consistent = leaves.len() == outline.leaf_owner.len()
            && u32::try_from(text.len()).is_ok_and(|len| len < u32::MAX)
            && u32::try_from(source.len()).is_ok_and(|len| len < u32::MAX);
        let visible = if consistent {
            resolve_markers(&mut pieces, &leaves, &outline);
            visibility(&pieces, &leaves, &outline)
        } else {
            Vec::new()
        };
        let map = super::CopyMap {
            source,
            outline,
            pieces: pieces.into(),
            formulas: formulas.into(),
            leaves: leaves.into(),
            visible: visible.into(),
            consistent,
        };
        (text, map)
    }
}

/// Gives each marker piece the list item it belongs to. A leaf's markers are its first-leaf item
/// ancestors, outermost first. O(markers + leaves · depth).
fn resolve_markers(pieces: &mut [Piece], leaves: &[LeafText], outline: &CopyOutline) {
    let mut leaf = 0;
    let mut items = Vec::new();
    let mut items_leaf = usize::MAX;
    for piece in pieces.iter_mut() {
        let PieceKind::Marker(ordinal) = piece.kind else {
            continue;
        };
        while leaf + 1 < leaves.len() && leaves[leaf + 1].start <= piece.start {
            leaf += 1;
        }
        if items_leaf != leaf {
            items = outline.marker_items(leaf);
            items_leaf = leaf;
        }
        piece.kind = PieceKind::Marker(items.get(ordinal as usize).copied().unwrap_or(u32::MAX));
    }
}

/// The first and last visible display position of every outline block: `(first, end)`, where
/// `end` is just after its last visible byte, or `(u32::MAX, 0)` when nothing of it is visible.
/// A leaf block shows its content; a list item also its marker; a container shows what its
/// children show. Blocks are in pre-order, so one reverse pass folds children into parents. O(blocks
/// + leaves + markers).
fn visibility(pieces: &[Piece], leaves: &[LeafText], outline: &CopyOutline) -> Vec<(u32, u32)> {
    let mut visible = vec![(u32::MAX, 0); outline.blocks.len()];
    let mut widen = |block: u32, start: u32, end: u32| {
        if let Some(slot) = visible.get_mut(block as usize) {
            *slot = (slot.0.min(start), slot.1.max(end));
        }
    };
    for (index, leaf) in leaves.iter().enumerate() {
        if leaf.content < leaf.end
            && let Some(owner) = outline.owner(index)
        {
            widen(owner, leaf.content, leaf.end);
        }
    }
    for piece in pieces {
        if let PieceKind::Marker(item) = piece.kind {
            widen(item, piece.start, piece.end);
        }
    }
    for index in (0..outline.blocks.len()).rev() {
        let (start, end) = visible[index];
        if let Some(parent) = outline.blocks[index].parent
            && start <= end
        {
            let slot = &mut visible[parent as usize];
            *slot = (slot.0.min(start), slot.1.max(end));
        }
    }
    visible
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
