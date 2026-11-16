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
}

impl Endpoint {
    pub(super) fn at(&self) -> usize {
        match self {
            Self::Fixed(at) | Self::Inline { at, .. } | Self::InBlock { at, .. } => *at,
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
enum Side {
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
            return self.fixed(self.start_edge(item).or_else(|| self.block_start(item)));
        }
        if boundary == leaf.content as usize
            && let Some(at) = self.start_edge(owner)
        {
            return Ok(Some(Endpoint::Fixed(at)));
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
        if boundary == leaf.end as usize
            && let Some(at) = self.end_edge(owner)
        {
            return Ok(Some(Endpoint::Fixed(at)));
        }
        if boundary <= leaf.content as usize {
            // The selection ends in a generated marker: at its item's start.
            let item = self.marker_item(boundary - 1).unwrap_or(owner);
            return self.fixed(self.block_start(item));
        }
        self.interior(text, index, owner, boundary, Side::End)
            .map(Some)
    }

    fn fixed(&self, at: Option<usize>) -> Result<Option<Endpoint>, PreviewCopyError> {
        at.map(|at| Some(Endpoint::Fixed(at)))
            .ok_or(PreviewCopyError::Unmapped)
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

    /// Rule 2 at a start: walking up from `block` through the ancestors whose first visible
    /// position is the same, the source start of the outermost one that takes part.
    fn start_edge(&self, block: u32) -> Option<usize> {
        self.edge(block, |(first, _)| first, |range| range.start)
    }

    /// Rule 2 at an end, with the last visible position and the source end.
    fn end_edge(&self, block: u32) -> Option<usize> {
        self.edge(block, |(_, end)| end, |range| range.end)
    }

    fn edge(
        &self,
        block: u32,
        position: impl Fn((u32, u32)) -> u32,
        side: impl Fn(SourceRange) -> usize,
    ) -> Option<usize> {
        let &(first, end) = self.visible.get(block as usize)?;
        if first > end {
            return None;
        }
        let at = position((first, end));
        let mut outermost = None;
        let mut next = Some(block);
        while let Some(index) = next {
            let visible = *self.visible.get(index as usize)?;
            if position(visible) != at {
                break;
            }
            let node = self.outline.blocks.get(index as usize)?;
            if node.kind.expands()
                && let Some(range) = node.source
            {
                outermost = Some(side(range));
            }
            next = node.parent;
        }
        outermost
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
        if let Some(lines) = block.and_then(|range| self.outline.shifted_lines(range)) {
            // Comrak's inline positions in this block may name other text: whole lines (rule 6).
            return Ok(Endpoint::Fixed(match side {
                Side::Start => lines.start,
                Side::End => lines.end,
            }));
        }
        let byte = match side {
            Side::Start => boundary,
            Side::End => boundary - 1,
        };
        let content = self.leaves[leaf].content as usize..self.leaves[leaf].end as usize;
        let leaf_index = u32::try_from(leaf).map_err(|_| PreviewCopyError::Unmapped)?;
        let Some(piece) = self
            .piece_at(byte)
            .filter(|piece| content.contains(&(piece.start as usize)))
        else {
            // Generated text inside a leaf (a table's cell and row separators, an empty cell):
            // the start moves on to the next piece, the end back to the previous one.
            return Ok(match self.neighbour(content, byte, side) {
                Some(piece) => Endpoint::Inline {
                    at: match side {
                        Side::Start => piece.source_start as usize,
                        Side::End => piece.source_end as usize,
                    },
                    leaf: leaf_index,
                },
                None => Endpoint::Fixed(block.ok_or(PreviewCopyError::Unmapped)?.start),
            });
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
                let fenced = matches!(
                    self.outline
                        .blocks
                        .get(owner as usize)
                        .map(|block| block.kind),
                    Some(super::OutlineKind::CodeBlock { fenced: true })
                );
                let at = block_position(text, &self.source, piece, fenced, boundary, side)
                    .unwrap_or_else(|| round(range.clone(), side));
                Endpoint::InBlock {
                    at,
                    fence: fenced.then(|| fence(&self.source, range)).flatten(),
                }
            }
        })
    }

    /// The nearest piece of `content` after `byte` (for a start) or before it (for an end).
    fn neighbour(&self, content: Range<usize>, byte: usize, side: Side) -> Option<Piece> {
        let after = self
            .pieces
            .partition_point(|piece| piece.start as usize <= byte);
        match side {
            Side::Start => self.pieces.get(after).copied(),
            Side::End => after
                .checked_sub(1)
                .and_then(|index| self.pieces.get(index).copied()),
        }
        .filter(|piece| content.contains(&(piece.start as usize)))
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

fn round(range: Range<usize>, side: Side) -> usize {
    match side {
        Side::Start => range.start,
        Side::End => range.end,
    }
}

/// The source position of display boundary `boundary` inside a code or Raw HTML block piece.
/// Display line `k` of the block's literal is a suffix of source line `k` of the block (after the
/// open fence line when fenced), since Comrak only strips container prefixes and indentation.
/// When that does not hold the boundary rounds outward to its source line. `None` when the line
/// does not exist. O(block bytes).
fn block_position(
    text: &str,
    source: &str,
    piece: Piece,
    fenced: bool,
    boundary: usize,
    side: Side,
) -> Option<usize> {
    let literal = text.get(piece.display())?;
    let block = source.get(piece.source())?;
    let offset = boundary - piece.start as usize;
    // A char boundary on the display line holding the bound byte: the byte after the boundary
    // for a start, the one before it for an end. They differ only when the end follows a newline,
    // which then binds the end to the line that newline closes.
    let byte = match side {
        Side::End if literal[..offset].ends_with('\n') => offset - 1,
        Side::Start | Side::End => offset,
    };
    let line_start = literal[..byte].rfind('\n').map_or(0, |index| index + 1);
    let line_end = literal[byte..]
        .find('\n')
        .map_or(literal.len(), |index| byte + index);
    let line = &literal[line_start..line_end];
    let number = literal[..line_start].matches('\n').count();
    let mut source_start = if fenced { block.find('\n')? + 1 } else { 0 };
    for _ in 0..number {
        source_start += block[source_start..].find('\n')? + 1;
    }
    let source_end = block[source_start..]
        .find('\n')
        .map_or(block.len(), |index| source_start + index);
    let base = piece.source_start as usize;
    let source_line = &block[source_start..source_end];
    if source_line.ends_with(line) {
        Some(base + source_end - line.len() + (offset - line_start))
    } else {
        Some(
            base + match side {
                Side::Start => source_start,
                Side::End => source_end,
            },
        )
    }
}
