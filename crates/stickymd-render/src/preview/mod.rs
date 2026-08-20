//! Native Markdown preview projection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection
//!
//! Every value in this module is derived from an immutable `DocumentSnapshot`.
//! No preview type exposes a source mutation or persistence API.

mod layout;
mod math_layout;
mod model;
mod paint;
mod parser;
mod pipeline;
mod render_tree;
mod selection;
mod source_map;
mod table_layout;

pub use model::{
    BlockNode, CodeBlockNode, ImageKind, InlineNode, LinkKind, ListItem, ListNode, MathNode,
    OwnedDocumentTree, SourceRange, TableAlignment, TableCell, TableNode, TableRow,
};
pub use paint::{PreviewFrame, PreviewPaintError, PreviewTheme};
pub use parser::{
    MAX_OWNED_NODES, MAX_PARSE_DEPTH, MAX_PREVIEW_SOURCE_BYTES, PreviewParseError,
    PreviewParseMetrics, PreviewParser,
};
pub use pipeline::{
    PreviewMathCounters, PreviewPipeline, PreviewPipelineCounters, PreviewPipelineError,
};
pub use render_tree::{
    RenderBlock, RenderBlockKind, RenderSpan, RenderStyle, RenderTable, RenderTableRow, RenderTree,
    RenderTreeBuilder, SpanAction,
};
pub use selection::{PreviewRect, PreviewSelection, PreviewTextBox, PreviewTextIndex};
use source_map::SourceMap;
