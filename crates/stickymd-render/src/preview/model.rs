//! Project-owned Markdown semantic objects.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection

use std::ops::Range;
use std::sync::Arc;

use stickymd_core::Generation;

/// Half-open UTF-8 byte range in the canonical snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}

impl SourceRange {
    pub fn new(start: usize, end: usize) -> Option<Self> {
        (start <= end).then_some(Self { start, end })
    }

    pub const fn as_range(self) -> Range<usize> {
        self.start..self.end
    }

    pub fn text(self, source: &str) -> Option<&str> {
        source.get(self.as_range())
    }
}

/// An Arena-free semantic projection of one document revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedDocumentTree {
    pub generation: Generation,
    pub source: Arc<str>,
    pub blocks: Vec<BlockNode>,
    pub node_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockNode {
    Paragraph {
        content: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    Heading {
        level: u8,
        content: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    BlockQuote {
        blocks: Vec<BlockNode>,
        source_range: Option<SourceRange>,
    },
    List(ListNode),
    CodeBlock(CodeBlockNode),
    Table(TableNode),
    ThematicBreak {
        source_range: Option<SourceRange>,
    },
    HtmlLiteral {
        literal: String,
        source_range: Option<SourceRange>,
    },
    DisplayMath(MathNode),
}

impl BlockNode {
    pub const fn source_range(&self) -> Option<SourceRange> {
        match self {
            Self::Paragraph { source_range, .. }
            | Self::Heading { source_range, .. }
            | Self::BlockQuote { source_range, .. }
            | Self::ThematicBreak { source_range }
            | Self::HtmlLiteral { source_range, .. } => *source_range,
            Self::List(list) => list.source_range,
            Self::CodeBlock(code) => code.source_range,
            Self::Table(table) => table.source_range,
            Self::DisplayMath(math) => math.source_range,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListNode {
    pub ordered: bool,
    pub start: usize,
    pub tight: bool,
    pub items: Vec<ListItem>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub checked: Option<bool>,
    pub blocks: Vec<BlockNode>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockNode {
    pub literal: String,
    pub info: String,
    pub fenced: bool,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableNode {
    pub alignments: Vec<TableAlignment>,
    pub rows: Vec<TableRow>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub header: bool,
    pub cells: Vec<TableCell>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub content: Vec<InlineNode>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Http,
    Https,
    Mailto,
    File,
    Relative,
    Blocked,
}

impl LinkKind {
    pub const fn may_open(self) -> bool {
        !matches!(self, Self::Blocked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    LocalRelative,
    LocalAbsolute,
    Remote,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathNode {
    pub literal: String,
    pub source_literal: String,
    pub display: bool,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineNode {
    Text {
        literal: String,
        source_range: Option<SourceRange>,
    },
    Emphasis {
        children: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    Strong {
        children: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    Strikethrough {
        children: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    Code {
        literal: String,
        source_range: Option<SourceRange>,
    },
    Link {
        destination: String,
        title: String,
        kind: LinkKind,
        children: Vec<InlineNode>,
        source_range: Option<SourceRange>,
    },
    Image {
        destination: String,
        title: String,
        alt: String,
        kind: ImageKind,
        source_range: Option<SourceRange>,
    },
    InlineMath(MathNode),
    SoftBreak {
        source_range: Option<SourceRange>,
    },
    HardBreak {
        source_range: Option<SourceRange>,
    },
    HtmlLiteral {
        literal: String,
        source_range: Option<SourceRange>,
    },
}

impl InlineNode {
    pub const fn source_range(&self) -> Option<SourceRange> {
        match self {
            Self::Text { source_range, .. }
            | Self::Emphasis { source_range, .. }
            | Self::Strong { source_range, .. }
            | Self::Strikethrough { source_range, .. }
            | Self::Code { source_range, .. }
            | Self::Link { source_range, .. }
            | Self::Image { source_range, .. }
            | Self::SoftBreak { source_range }
            | Self::HardBreak { source_range }
            | Self::HtmlLiteral { source_range, .. } => *source_range,
            Self::InlineMath(math) => math.source_range,
        }
    }
}
