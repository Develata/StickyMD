//! Maps each selection endpoint to a source position: block edges (rule 2), rounding and snapping
//! where positions are untrusted (rule 6), and line-by-line mapping inside code blocks.
//!
//! A display position is a boundary between two bytes. The start binds to the byte after it (the
//! first selected byte), the end to the byte before it (the last selected byte), so a boundary
//! between two pieces belongs to the piece that begins there for the start and to the piece that
//! ends there for the end. Both functions are total: every boundary maps, or reports that nothing
//! visible lies on its side.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use std::ops::Range;

use super::display::{Piece, PieceKind};
use super::{CopyMap, Fence, PreviewCopyError, fence};
use crate::preview::SourceRange;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Endpoint {
    /// Settled: a block edge, a snap, or a position no inline structure can contain.
    Fixed(usize),
    /// In a leaf's inline text; rule 3 may still move it outward.
    Inline { at: usize, leaf: u32 },
    /// In a code or Raw HTML block's text, with the fence lines when it cuts a closed fenced code
    /// block (rule 3).
    InBlock { at: usize, fence: Option<Fence> },
    /// On the edge of leaf `leaf`'s block or outside it, put there by rounding where positions
    /// cannot be trusted (rule 6) or by a failed closer check (rule 3): whole lines, or past text
    /// that cannot be mapped. No structure moves it, but rule 2 still applies.
    AtBlockEdge { at: usize, leaf: u32 },
}

impl Endpoint {
    pub(super) fn at(&self) -> usize {
        match self {
            Self::Fixed(at)
            | Self::Inline { at, .. }
            | Self::InBlock { at, .. }
            | Self::AtBlockEdge { at, .. } => *at,
        }
    }

    pub(super) fn fence(&self) -> Option<Fence> {
        match self {
            Self::InBlock { fence, .. } => fence.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Start,
    End,
}

impl CopyMap {
    /// The start endpoint at display boundary `boundary`, or `None` when nothing visible follows.
    pub(super) fn start_endpoint(
        &self,
        text: &str,
        boundary: usize,
    ) -> Result<Option<Endpoint>, PreviewCopyError> {
        // The first non-empty leaf ending after the boundary; a boundary between leaves moves to
        // the next one, since the separator before it maps to nothing.
        let first = self
            .leaves
            .partition_point(|leaf| leaf.end as usize <= boundary);
        let Some((index, leaf)) = self
            .leaves
            .iter()
            .enumerate()
            .skip(first)
            .find(|(_, leaf)| leaf.start < leaf.end)
        else {
            return Ok(None);
        };
        let boundary = boundary.max(leaf.start as usize);
        let owner = self.owner(index)?;
        if boundary < leaf.content as usize {
            // A generated list marker snaps to its item's start, which is the item's first
            // visible position, so the block-edge rule applies from there.
            let item = self.marker_item(boundary).unwrap_or(owner);
            let at = self.start_edge(item).or_else(|| self.block_start(item));
            return Ok(Some(Endpoint::Fixed(
                at.unwrap_or_else(|| self.start_gap(index)),
            )));
        }
        if boundary == leaf.content as usize {
            let edge = self.start_edge(owner);
            let at = if self.leaf_trusted(text, index) {
                edge
            } else {
                Some(edge.map_or(self.start_gap(index), |at| at.min(self.start_gap(index))))
            };
            if let Some(at) = at {
                return Ok(Some(Endpoint::Fixed(at)));
            }
        }
        self.interior(text, index, owner, boundary, Side::Start)
            .map(Some)
    }

    /// The end endpoint at display boundary `boundary`, or `None` when nothing visible precedes.
    pub(super) fn end_endpoint(
        &self,
        text: &str,
        boundary: usize,
    ) -> Result<Option<Endpoint>, PreviewCopyError> {
        let Some(last) = boundary.checked_sub(1) else {
            return Ok(None);
        };
        // The last non-empty leaf starting at or before the last selected byte.
        let after = self
            .leaves
            .partition_point(|leaf| leaf.start as usize <= last);
        let Some((index, leaf)) = self.leaves[..after]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, leaf)| leaf.start < leaf.end)
        else {
            return Ok(None);
        };
        let boundary = boundary.min(leaf.end as usize);
        let owner = self.owner(index)?;
        // At the leaf's end first: a placeholder leaf (an empty item) ends in its marker, which
        // is then its owner's last visible position.
        if boundary == leaf.end as usize {
            let edge = self.end_edge(owner);
            let at = if self.leaf_trusted(text, index) {
                edge
            } else {
                Some(edge.map_or(self.end_gap(index), |at| at.max(self.end_gap(index))))
            };
            if let Some(at) = at {
                return Ok(Some(Endpoint::Fixed(at)));
            }
        }
        if boundary <= leaf.content as usize {
            // The selection ends in a generated marker: at its item's start.
            let item = self.marker_item(boundary - 1).unwrap_or(owner);
            return Ok(Some(Endpoint::Fixed(
                self.block_start(item)
                    .unwrap_or_else(|| self.start_gap(index)),
            )));
        }
        self.interior(text, index, owner, boundary, Side::End)
            .map(Some)
    }

    fn owner(&self, leaf: usize) -> Result<u32, PreviewCopyError> {
        self.outline.owner(leaf).ok_or(PreviewCopyError::Unmapped)
    }

    pub(super) fn block_source(&self, block: u32) -> Option<SourceRange> {
        self.outline.blocks.get(block as usize)?.source
    }

    /// Where `block` starts in the source: its own start, or when Comrak gave it no range, the
    /// start of the first of its leaves that has one, which leaves its marker's source out.
    fn block_start(&self, block: u32) -> Option<usize> {
        let node = self.outline.blocks.get(block as usize)?;
        node.source.map(|range| range.start).or_else(|| {
            (node.leaves.start..node.leaves.end).find_map(|leaf| {
                let owner = self.outline.owner(leaf as usize)?;
                self.block_source(owner).map(|range| range.start)
            })
        })
    }

    fn marker_item(&self, byte: usize) -> Option<u32> {
        match self.piece_at(byte)?.kind {
            PieceKind::Marker(item) if item != u32::MAX => Some(item),
            _ => None,
        }
    }

    /// An endpoint strictly inside a leaf's own text.
    fn interior(
        &self,
        text: &str,
        leaf: usize,
        owner: u32,
        boundary: usize,
        side: Side,
    ) -> Result<Endpoint, PreviewCopyError> {
        let block = self.block_source(owner);
        let leaf_index = u32::try_from(leaf).map_err(|_| PreviewCopyError::Unmapped)?;
        let byte = match side {
            Side::Start => boundary,
            Side::End => boundary - 1,
        };
        let content = self.leaves[leaf].content as usize..self.leaves[leaf].end as usize;
        let shown = self
            .piece_at(byte)
            .filter(|piece| content.contains(&(piece.start as usize)));
        let piece = match shown {
            Some(piece) if self.trusted(text, piece, owner) => piece,
            // Shown text whose source cannot be reached (an object without a range, a block
            // reported cut short): round outward past it (rule 6), to the trusted text around it
            // or to the block's edges, so nothing shown is left out.
            Some(_) => return Ok(self.round_past(text, leaf, leaf_index, block, byte, side)),
            None => return self.across_generated(text, leaf, leaf_index, block, byte, side),
        };
        let offset = boundary - piece.start as usize;
        let inline = |at| Endpoint::Inline {
            at,
            leaf: leaf_index,
        };
        Ok(match piece.kind {
            PieceKind::Text => {
                inline(self.exact_or_round(text, piece, piece.source(), offset, side))
            }
            PieceKind::InlineCode => {
                let inner = self
                    .code_content(leaf_index, piece)
                    .unwrap_or(piece.source());
                inline(self.exact_or_round(text, piece, inner, offset, side))
            }
            PieceKind::Formula(_) | PieceKind::Image | PieceKind::Marker(_) => {
                inline(round(piece.source(), side))
            }
            PieceKind::Block => {
                let range = piece.source();
                let (fenced, closed) = match self
                    .outline
                    .blocks
                    .get(owner as usize)
                    .map(|block| block.kind)
                {
                    Some(super::OutlineKind::CodeBlock { fenced, closed }) => (fenced, closed),
                    _ => (false, false),
                };
                let at = super::block::position(text, &self.source, piece, fenced, boundary, side)
                    .unwrap_or_else(|| round(range.clone(), side));
                // Only a block Comrak closed has a close fence to copy (rule 3).
                Endpoint::InBlock {
                    at,
                    fence: (fenced && closed)
                        .then(|| fence(&self.source, range))
                        .flatten(),
                }
            }
        })
    }

    /// The nearest trusted piece of leaf `leaf` after `byte` (for a start) or before it (for an end).
    pub(super) fn neighbour(
        &self,
        text: &str,
        leaf: usize,
        byte: usize,
        side: Side,
    ) -> Option<Piece> {
        let after = self
            .pieces
            .partition_point(|piece| piece.start as usize <= byte);
        self.sourced(text, leaf, after, side)
    }

    /// From piece `from`, the first trusted piece of leaf `leaf`'s own text: forward for a start,
    /// backward from the one before `from` for an end.
    pub(super) fn sourced(
        &self,
        text: &str,
        leaf: usize,
        from: usize,
        side: Side,
    ) -> Option<Piece> {
        let span = self.leaves.get(leaf)?;
        let owner = self.outline.owner(leaf)?;
        let content = span.content as usize..span.end as usize;
        let inside = |piece: &&Piece| content.contains(&(piece.start as usize));
        let trusted = |piece: &&Piece| self.trusted(text, **piece, owner);
        match side {
            Side::Start => self
                .pieces
                .get(from..)?
                .iter()
                .take_while(inside)
                .find(trusted),
            Side::End => self
                .pieces
                .get(..from)?
                .iter()
                .rev()
                .take_while(inside)
                .find(trusted),
        }
        .copied()
    }

    /// The text between the backtick runs of the inline code `piece` belongs to.
    fn code_content(&self, leaf: u32, piece: Piece) -> Option<Range<usize>> {
        let structures = &self.outline.structures[self.outline.structure_range(leaf)];
        let found = structures
            .binary_search_by_key(&piece.source_start, |structure| {
                u32::try_from(structure.source.start).unwrap_or(u32::MAX)
            })
            .ok()?;
        let structure = structures[found..]
            .iter()
            .take_while(|structure| structure.source.start == piece.source_start as usize)
            .find(|structure| structure.source.end == piece.source_end as usize)?;
        structure.content.map(SourceRange::as_range)
    }

    /// Byte for byte when the piece's display text equals `source` (rule 6: it corresponds one
    /// to one); otherwise outward to the edges of `source`.
    fn exact_or_round(
        &self,
        text: &str,
        piece: Piece,
        source: Range<usize>,
        offset: usize,
        side: Side,
    ) -> usize {
        if text.get(piece.display()) == self.source.get(source.clone()) {
            source.start + offset
        } else {
            round(source, side)
        }
    }
}

pub(super) fn round(range: Range<usize>, side: Side) -> usize {
    match side {
        Side::Start => range.start,
        Side::End => range.end,
    }
}
