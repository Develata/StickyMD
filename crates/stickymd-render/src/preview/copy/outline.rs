//! What copying needs of the document's structure, kept while the render tree flattens it: the
//! blocks with their nesting and the render blocks (leaves) each covers, and the inline structures
//! of each leaf with their source ranges and nesting.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-semantics

use std::ops::Range;

use crate::preview::{InlineNode, SourceRange};

/// The kind of an outline block. Every kind but `Other` takes part in the block-edge rule
/// (rule 2): an endpoint at its first or last visible position expands to its source edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineKind {
    Paragraph,
    Heading,
    Quote,
    List,
    ListItem,
    Table,
    CodeBlock {
        fenced: bool,
    },
    /// Display math, a Raw HTML block, a thematic break: no block-edge expansion of its own.
    Other,
}

impl OutlineKind {
    pub(crate) const fn expands(self) -> bool {
        !matches!(self, Self::Other)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineBlock {
    pub kind: OutlineKind,
    pub source: Option<SourceRange>,
    pub parent: Option<u32>,
    /// The leaves it covers, in document order.
    pub leaves: Range<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructureKind {
    Emphasis,
    Strong,
    Strikethrough,
    Code,
    Link,
    Image,
}

/// An inline structure (rule 3), within one leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Structure {
    pub kind: StructureKind,
    pub source: SourceRange,
    /// Where its visible content starts and ends in the source: the extent of its children, or for
    /// inline code the text between its backtick runs. `None` for an image (atomic) and for a
    /// structure whose content cannot be located; the latter fails the closer check.
    pub content: Option<SourceRange>,
    pub leaf: u32,
    /// The innermost structure of the same leaf that encloses it.
    pub parent: Option<u32>,
}

/// Invariants, established by `OutlineBuilder`:
/// - `blocks` is in pre-order: a parent precedes its children, so its index is smaller.
/// - `leaf_owner[leaf]` is the block that owns render block `leaf`: the leaf block itself, or the
///   container a placeholder stands for.
/// - `structures` is grouped by leaf in leaf order and is in pre-order within a leaf, so within a
///   leaf it is sorted by source start, an enclosing structure before what it encloses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CopyOutline {
    pub blocks: Vec<OutlineBlock>,
    pub leaf_owner: Vec<u32>,
    pub structures: Vec<Structure>,
    /// Whole-line source ranges of the blocks whose inline positions Comrak reported shifted;
    /// in document order and disjoint.
    pub shifted: Vec<Range<usize>>,
}

impl CopyOutline {
    #[cfg(test)]
    pub(crate) fn heap_bytes(&self) -> usize {
        use std::mem::size_of;
        self.blocks.capacity() * size_of::<OutlineBlock>()
            + self.leaf_owner.capacity() * size_of::<u32>()
            + self.structures.capacity() * size_of::<Structure>()
            + self.shifted.capacity() * size_of::<Range<usize>>()
    }

    pub(crate) fn block(&self, index: u32) -> &OutlineBlock {
        &self.blocks[index as usize]
    }

    pub(crate) fn owner(&self, leaf: usize) -> Option<u32> {
        self.leaf_owner.get(leaf).copied()
    }

    /// The list items whose markers render block `leaf` carries, outermost first: the items among
    /// its ancestors whose first leaf it is. That is the order `append_list` inserts them in.
    pub(crate) fn marker_items(&self, leaf: usize) -> Vec<u32> {
        let mut items = Vec::new();
        let mut next = self.owner(leaf);
        while let Some(index) = next {
            let block = self.block(index);
            if block.kind == OutlineKind::ListItem && block.leaves.start as usize == leaf {
                items.push(index);
            }
            next = block.parent;
        }
        items.reverse();
        items
    }

    /// The whole-line range of the shifted block that `range` starts in.
    pub(crate) fn shifted_lines(&self, range: SourceRange) -> Option<Range<usize>> {
        let after = self
            .shifted
            .partition_point(|lines| lines.start <= range.start);
        let lines = self.shifted.get(after.checked_sub(1)?)?;
        (range.start < lines.end).then(|| lines.clone())
    }

    /// The indices of the structures of `leaf`, in pre-order. O(log structures).
    pub(crate) fn structure_range(&self, leaf: u32) -> Range<usize> {
        let start = self
            .structures
            .partition_point(|structure| structure.leaf < leaf);
        let end = self
            .structures
            .partition_point(|structure| structure.leaf <= leaf);
        start..end
    }
}

/// Records the outline while the render tree is built, one block at a time.
#[derive(Debug, Default)]
pub(crate) struct OutlineBuilder {
    outline: CopyOutline,
    open: Vec<u32>,
}

impl OutlineBuilder {
    /// Opens a container (quote, list, list item) whose leaves start at `first_leaf`.
    pub(crate) fn open(
        &mut self,
        kind: OutlineKind,
        source: Option<SourceRange>,
        first_leaf: usize,
    ) -> u32 {
        let index = self.push(kind, source, first_leaf..first_leaf);
        self.open.push(index);
        index
    }

    /// Closes the container `index`, whose leaves end before `end_leaf`.
    pub(crate) fn close(&mut self, index: u32, end_leaf: usize) {
        debug_assert_eq!(self.open.last(), Some(&index));
        self.open.pop();
        self.outline.blocks[index as usize].leaves.end = offset(end_leaf);
    }

    /// A block that is one render block, `leaf`, the next one in order.
    pub(crate) fn leaf(&mut self, kind: OutlineKind, source: Option<SourceRange>, leaf: usize) {
        let index = self.push(kind, source, leaf..leaf + 1);
        self.own(leaf, index);
    }

    /// The empty render block `leaf` that stands for the open container with no content.
    pub(crate) fn placeholder(&mut self, leaf: usize) {
        if let Some(&container) = self.open.last() {
            self.own(leaf, container);
        }
    }

    fn push(
        &mut self,
        kind: OutlineKind,
        source: Option<SourceRange>,
        leaves: Range<usize>,
    ) -> u32 {
        let index = offset(self.outline.blocks.len());
        self.outline.blocks.push(OutlineBlock {
            kind,
            source,
            parent: self.open.last().copied(),
            leaves: offset(leaves.start)..offset(leaves.end),
        });
        index
    }

    fn own(&mut self, leaf: usize, owner: u32) {
        debug_assert_eq!(
            leaf,
            self.outline.leaf_owner.len(),
            "leaves arrive in order"
        );
        if self.outline.leaf_owner.len() <= leaf {
            self.outline.leaf_owner.resize(leaf + 1, owner);
        }
        self.outline.leaf_owner[leaf] = owner;
    }

    /// Records the inline structures of `nodes` for render block `leaf`, nested ones included.
    /// A structure whose range leaves `block`, the leaf's own source range, is not recorded:
    /// a range Comrak reported shifted can name other text (plan 06 rule 6).
    pub(crate) fn inlines(
        &mut self,
        nodes: &[InlineNode],
        leaf: usize,
        block: Option<SourceRange>,
        source: &str,
    ) {
        self.inlines_within(nodes, offset(leaf), block, source, None);
    }

    fn inlines_within(
        &mut self,
        nodes: &[InlineNode],
        leaf: u32,
        block: Option<SourceRange>,
        source: &str,
        parent: Option<u32>,
    ) {
        for node in nodes {
            let (kind, children) = match node {
                InlineNode::Emphasis { children, .. } => (StructureKind::Emphasis, Some(children)),
                InlineNode::Strong { children, .. } => (StructureKind::Strong, Some(children)),
                InlineNode::Strikethrough { children, .. } => {
                    (StructureKind::Strikethrough, Some(children))
                }
                InlineNode::Link { children, .. } => (StructureKind::Link, Some(children)),
                InlineNode::Code { .. } => (StructureKind::Code, None),
                InlineNode::Image { .. } => (StructureKind::Image, None),
                _ => continue,
            };
            let recorded = node
                .source_range()
                .filter(|range| block.is_some_and(|block| within(*range, block)))
                .map(|range| {
                    let content = match kind {
                        StructureKind::Code => code_content(range, source),
                        StructureKind::Image => None,
                        _ => children
                            .and_then(|children| extent(children))
                            .filter(|content| within(*content, range)),
                    };
                    let index = offset(self.outline.structures.len());
                    self.outline.structures.push(Structure {
                        kind,
                        source: range,
                        content,
                        leaf,
                        parent,
                    });
                    index
                });
            if let Some(children) = children {
                self.inlines_within(children, leaf, block, source, recorded.or(parent));
            }
        }
    }

    pub(crate) fn finish(mut self, shifted: Vec<Range<usize>>) -> CopyOutline {
        debug_assert!(self.open.is_empty());
        // The outline lives as long as the tree and its layouts: drop the growth slack.
        self.outline.blocks.shrink_to_fit();
        self.outline.leaf_owner.shrink_to_fit();
        self.outline.structures.shrink_to_fit();
        self.outline.shifted = shifted;
        self.outline
    }
}

fn within(inner: SourceRange, outer: SourceRange) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// The source extent of `nodes`: from the first start to the last end.
fn extent(nodes: &[InlineNode]) -> Option<SourceRange> {
    let mut ranges = nodes.iter().filter_map(InlineNode::source_range);
    let first = ranges.next()?;
    let (start, end) = ranges.fold((first.start, first.end), |(start, end), range| {
        (start.min(range.start), end.max(range.end))
    });
    SourceRange::new(start, end)
}

/// The text between an inline code span's backtick runs, which must have the same length.
fn code_content(range: SourceRange, source: &str) -> Option<SourceRange> {
    let text = range.text(source)?;
    let run = text.bytes().take_while(|&byte| byte == b'`').count();
    let closing = text.bytes().rev().take_while(|&byte| byte == b'`').count();
    (run > 0 && closing == run && text.len() >= run * 2)
        .then(|| SourceRange::new(range.start + run, range.end - run))
        .flatten()
}

/// Outline indices are `u32`: the Preview parser caps a document at 5 MiB, far below `u32::MAX`
/// blocks, leaves or structures. Saturating keeps an impossible overflow from wrapping.
fn offset(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
