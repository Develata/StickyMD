//! Rule 2's block edges: an endpoint at a block's first or last visible position takes the source
//! edge of the outermost block there, also after rounding (rule 6) or a cut structure (rule 3)
//! moved it onto its leaf's first or last text.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use super::CopyMap;
use super::endpoint::{Endpoint, Side};
use crate::preview::SourceRange;

impl CopyMap {
    /// Rule 2 at a start: walking up from `block` through the ancestors whose first visible
    /// position is the same, the source start of the outermost one that takes part.
    pub(super) fn start_edge(&self, block: u32) -> Option<usize> {
        self.edge(block, |(first, _)| first, |range| range.start, usize::min)
    }

    /// Rule 2 at an end, with the last visible position and the source end.
    pub(super) fn end_edge(&self, block: u32) -> Option<usize> {
        self.edge(block, |(_, end)| end, |range| range.end, usize::max)
    }

    fn edge(
        &self,
        block: u32,
        position: impl Fn((u32, u32)) -> u32,
        side: impl Fn(SourceRange) -> usize,
        outward: fn(usize, usize) -> usize,
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
            // The outermost block's edge, or further out when Comrak reported an inner block
            // reaching past it: an endpoint only ever moves outward.
            if node.kind.expands()
                && let Some(range) = node.source
            {
                let edge = side(range);
                outermost = Some(outermost.map_or(edge, |known| outward(known, edge)));
            }
            next = node.parent;
        }
        outermost
    }

    /// Rule 2 after rules 6 and 3, at a start: an inline endpoint that rounding or a cut structure
    /// moved to its leaf's first text is at the leaf's first visible position, so the block-edge
    /// rule applies there too.
    pub(super) fn start_after_settling(&self, text: &str, endpoint: Endpoint) -> Endpoint {
        self.edge_after_settling(text, endpoint, Side::Start)
    }

    /// The same at an end, with the leaf's last text and last visible position.
    pub(super) fn end_after_settling(&self, text: &str, endpoint: Endpoint) -> Endpoint {
        self.edge_after_settling(text, endpoint, Side::End)
    }

    fn edge_after_settling(&self, text: &str, endpoint: Endpoint, side: Side) -> Endpoint {
        let (at, leaf) = match endpoint {
            Endpoint::Inline { at, leaf } => (at, leaf),
            // Already on its block's edge or outside it: rule 2 applies without a test.
            Endpoint::AtBlockEdge { at, leaf } => {
                let Some(owner) = self.outline.owner(leaf as usize) else {
                    return Endpoint::Fixed(at);
                };
                return Endpoint::Fixed(match side {
                    Side::Start => self.start_edge(owner).map_or(at, |edge| edge.min(at)),
                    Side::End => self.end_edge(owner).map_or(at, |edge| edge.max(at)),
                });
            }
            Endpoint::Fixed(_) | Endpoint::InBlock { .. } => return endpoint,
        };
        let (Some(span), Some(owner)) = (
            self.leaves.get(leaf as usize),
            self.outline.owner(leaf as usize),
        ) else {
            return endpoint;
        };
        let content = span.content as usize..span.end as usize;
        let from = self
            .pieces
            .partition_point(|piece| (piece.start as usize) < content.start);
        let to = self
            .pieces
            .partition_point(|piece| (piece.start as usize) < content.end);
        let edge = match side {
            Side::Start => self
                .sourced(text, leaf as usize, from, side)
                .filter(|first| at <= first.source_start as usize)
                .and_then(|_| self.start_edge(owner))
                .map(|edge| edge.min(at)),
            Side::End => self
                .sourced(text, leaf as usize, to, side)
                .filter(|last| at >= last.source_end as usize)
                .and_then(|_| self.end_edge(owner))
                .map(|edge| edge.max(at)),
        };
        edge.map_or(endpoint, Endpoint::Fixed)
    }
}
