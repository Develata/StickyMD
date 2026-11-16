//! Rule 3 for inline structures: an endpoint inside a structure the other endpoint is not inside
//! (a cut structure), or on its visible content's edge, moves to the structure's source boundary;
//! repeated until no structure is cut. A structure whose source range does not end in its own
//! closing delimiter fails the check, and the endpoint snaps to its block's whole lines instead.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use super::endpoint::Endpoint;
use super::outline::{Structure, StructureKind};
use super::{CopyMap, PreviewCopyError};

impl CopyMap {
    /// Moves the inline endpoints outward until neither cuts a structure. Each step moves one
    /// endpoint strictly outward to a boundary of a structure that contained it, within its leaf,
    /// so the loop ends after at most the nesting depth at each endpoint.
    pub(super) fn settle(
        &self,
        mut start: Endpoint,
        mut end: Endpoint,
    ) -> Result<(Endpoint, Endpoint), PreviewCopyError> {
        loop {
            let mut moved = false;
            if let Endpoint::Inline { at, leaf } = start
                && let Some(structure) = self.cut(leaf, at, end.at())
            {
                start = if self.closes(&structure) {
                    Endpoint::Inline {
                        at: structure.source.start,
                        leaf,
                    }
                } else {
                    Endpoint::AtBlockEdge {
                        at: self.whole_lines(leaf)?.0,
                        leaf,
                    }
                };
                moved = true;
            }
            if let Endpoint::Inline { at, leaf } = end
                && let Some(structure) = self.cut(leaf, at, start.at())
            {
                end = if self.closes(&structure) {
                    Endpoint::Inline {
                        at: structure.source.end,
                        leaf,
                    }
                } else {
                    Endpoint::AtBlockEdge {
                        at: self.whole_lines(leaf)?.1,
                        leaf,
                    }
                };
                moved = true;
            }
            if !moved {
                return Ok((start, end));
            }
        }
    }

    /// The innermost structure of `leaf` strictly containing `at`, if it must move: `at` is not
    /// inside its visible content (it is on an edge), or `other` is not. When the innermost one
    /// needs no move, no enclosing one does: its content lies within theirs. O(log structures +
    /// depth).
    fn cut(&self, leaf: u32, at: usize, other: usize) -> Option<Structure> {
        let range = self.outline.structure_range(leaf);
        let before = self.outline.structures[range.clone()]
            .partition_point(|structure| structure.source.start < at);
        let mut next = before.checked_sub(1).map(|index| range.start + index);
        while let Some(index) = next {
            let structure = *self.outline.structures.get(index)?;
            if structure.source.start < at && at < structure.source.end {
                let inside = |position: usize| {
                    structure
                        .content
                        .is_some_and(|content| content.start < position && position < content.end)
                };
                return (!inside(at) || !inside(other)).then_some(structure);
            }
            next = structure.parent.map(|parent| parent as usize);
        }
        None
    }

    /// Whether the structure's source range ends in its own closing delimiter: emphasis the same
    /// run it opens with, a link or image `)` or `]`, inline code a backtick run as long as its
    /// opening one.
    fn closes(&self, structure: &Structure) -> bool {
        let Some(text) = structure.source.text(&self.source) else {
            return false;
        };
        match structure.kind {
            StructureKind::Emphasis | StructureKind::Strong | StructureKind::Strikethrough => {
                structure.content.is_some_and(|content| {
                    let open = &self.source[structure.source.start..content.start];
                    !open.is_empty()
                        && structure.source.end >= content.end + open.len()
                        && text.ends_with(open)
                })
            }
            StructureKind::Link | StructureKind::Image => text.ends_with([')', ']']),
            StructureKind::Code => structure.content.is_some(),
        }
    }

    /// The start of the first line and the end of the last line of the block owning `leaf`.
    fn whole_lines(&self, leaf: u32) -> Result<(usize, usize), PreviewCopyError> {
        let block = self
            .outline
            .owner(leaf as usize)
            .and_then(|owner| self.block_source(owner))
            .ok_or(PreviewCopyError::Unmapped)?;
        let source = &*self.source;
        let start = source
            .get(..block.start)
            .and_then(|before| before.rfind('\n'))
            .map_or(0, |index| index + 1);
        // A range that already ends with its newline ends on that line.
        let last = block.end.saturating_sub(1).max(block.start);
        let end = source
            .get(last..)
            .and_then(|after| after.find('\n'))
            .map_or(source.len(), |index| last + index);
        Ok((start, end))
    }
}
