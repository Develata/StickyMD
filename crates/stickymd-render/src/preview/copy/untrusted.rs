//! Rule 6 where Comrak's positions cannot be trusted: text Preview generates maps to nothing and is
//! stepped over; shown text whose source cannot be reached is rounded past outward, to the trusted
//! text around it, the block's edges, or the neighbouring blocks, so nothing shown is left out and
//! select all never fails.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use super::display::{Piece, PieceKind};
use super::endpoint::{Endpoint, Side};
use super::{CopyMap, PreviewCopyError};
use crate::preview::SourceRange;

impl CopyMap {
    /// Generated text inside a leaf (a table's cell and row separators, an empty cell) shows
    /// nothing of the source: the start moves on to the next piece, the end back to the previous.
    pub(super) fn across_generated(
        &self,
        text: &str,
        leaf: usize,
        leaf_index: u32,
        block: Option<SourceRange>,
        byte: usize,
        side: Side,
    ) -> Result<Endpoint, PreviewCopyError> {
        let edge = |at| Endpoint::AtBlockEdge {
            at,
            leaf: leaf_index,
        };
        Ok(match self.neighbour(text, leaf, byte, side) {
            Some(piece) => Endpoint::Inline {
                at: match side {
                    Side::Start => piece.source_start as usize,
                    Side::End => piece.source_end as usize,
                },
                leaf: leaf_index,
            },
            // Nothing in the leaf to map through: before the block when it is trustworthy,
            // otherwise outward to the neighbouring blocks Comrak placed (rule 6).
            None if self.leaf_trusted(text, leaf) => {
                Endpoint::Fixed(block.ok_or(PreviewCopyError::Unmapped)?.start)
            }
            None => edge(match side {
                Side::Start => block.map_or_else(|| self.start_gap(leaf), |range| range.start),
                Side::End => self.end_gap(leaf),
            }),
        })
    }

    /// An endpoint in shown text that cannot be mapped rounds outward past it: a start to the end
    /// of the trusted piece before it, an end to the start of the one after it; with none, to the
    /// block's edge, or past the block when its own range cannot be trusted either.
    pub(super) fn round_past(
        &self,
        text: &str,
        leaf: usize,
        leaf_index: u32,
        block: Option<SourceRange>,
        byte: usize,
        side: Side,
    ) -> Endpoint {
        let at = self
            .pieces
            .partition_point(|piece| piece.start as usize <= byte);
        let edge = |at| Endpoint::AtBlockEdge {
            at,
            leaf: leaf_index,
        };
        match side {
            Side::Start => match self.sourced(text, leaf, at.saturating_sub(1), Side::End) {
                Some(previous) => Endpoint::Inline {
                    at: previous.source_end as usize,
                    leaf: leaf_index,
                },
                None => edge(block.map_or_else(|| self.start_gap(leaf), |range| range.start)),
            },
            Side::End => match self.sourced(text, leaf, at, Side::Start) {
                Some(next) => Endpoint::Inline {
                    at: next.source_start as usize,
                    leaf: leaf_index,
                },
                None => edge(match block {
                    Some(range) if self.leaf_trusted(text, leaf) => range.end,
                    _ => self.end_gap(leaf),
                }),
            },
        }
    }

    /// Whether leaf `leaf`'s own text can be mapped through: it shows no piece at all (only
    /// markers or separators, so nothing of it can be lost), or some piece of it is trusted.
    pub(super) fn leaf_trusted(&self, text: &str, leaf: usize) -> bool {
        let Some(span) = self.leaves.get(leaf) else {
            return false;
        };
        let from = self
            .pieces
            .partition_point(|piece| (piece.start as usize) < span.content as usize);
        let shows_a_piece = self
            .pieces
            .get(from)
            .is_some_and(|piece| (piece.start as usize) < span.end as usize);
        !shows_a_piece || self.sourced(text, leaf, from, Side::Start).is_some()
    }

    /// A piece the source can be reached through: one with a source range, which for a code or
    /// Raw HTML block must hold at least the lines it shows.
    pub(super) fn trusted(&self, text: &str, piece: Piece, owner: u32) -> bool {
        piece.has_source()
            && (piece.kind != PieceKind::Block
                || super::block::holds_its_lines(text, &self.source, piece, self.fenced(owner)))
    }

    fn fenced(&self, owner: u32) -> bool {
        matches!(
            self.outline
                .blocks
                .get(owner as usize)
                .map(|block| block.kind),
            Some(super::OutlineKind::CodeBlock { fenced: true, .. })
        )
    }

    /// Where a start rounds outward to when its leaf has nothing to map through: the end of the
    /// nearest earlier leaf Comrak gave a range, or the document start (rule 6). O(leaves) at worst.
    pub(super) fn start_gap(&self, leaf: usize) -> usize {
        (0..leaf)
            .rev()
            .find_map(|index| self.block_source(self.outline.owner(index)?))
            .map_or(0, |range| range.end)
    }

    /// The same for an end: the start of the nearest later leaf with a range, or the document end.
    pub(super) fn end_gap(&self, leaf: usize) -> usize {
        (leaf + 1..self.leaves.len())
            .find_map(|index| self.block_source(self.outline.owner(index)?))
            .map_or(self.source.len(), |range| range.start)
    }
}
