//! Native block layout derived from `RenderTree`.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::ops::Range;
use std::sync::Arc;

use crate::math::{MathEngine, MathRaster};
use crate::source::{FontSelection, ScriptClass, segment_script_runs};
use cosmic_text::{
    Align, Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Style, UnderlineStyle, Weight, Wrap,
};
use stickymd_core::Generation;

use super::{
    PreviewRect, PreviewTextBox, PreviewTextIndex, RenderBlock, RenderBlockKind, RenderSpan,
    RenderStyle, RenderTree, SpanAction,
};

const BODY_SIZE_DIP: f32 = 17.0;
const BODY_LINE_DIP: f32 = 26.35;
const CODE_SIZE_DIP: f32 = 15.3;
const CODE_LINE_DIP: f32 = 23.0;
const PADDING_DIP: f32 = 24.0;
const BLOCK_GAP_DIP: f32 = 12.0;
const INDENT_DIP: f32 = 16.0;

pub(super) struct LaidOutDocument {
    pub generation: Generation,
    pub width_px: u32,
    pub height_px: f32,
    pub blocks: Vec<LaidOutBlock>,
    pub index: std::sync::Arc<PreviewTextIndex>,
    pub scale: f32,
    pub theme: super::PreviewTheme,
}

pub(super) struct LaidOutBlock {
    pub top: f32,
    pub bottom: f32,
    pub chunks: Vec<LayoutChunk>,
    pub decorations: Vec<LayoutDecoration>,
}

pub(super) struct LayoutChunk {
    pub content: LayoutContent,
    pub x: f32,
    pub y: f32,
}

pub(super) enum LayoutContent {
    Text(Buffer),
    Math(Arc<MathRaster>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DecorationRole {
    QuoteBar,
    CodeBackground,
    MathBackground,
    MathError,
    Rule,
    TableCell,
    TableHeader,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LayoutDecoration {
    pub rect: PreviewRect,
    pub role: DecorationRole,
}

#[derive(Debug, Clone)]
struct Segment {
    visual_range: Range<usize>,
    selection_range: Range<usize>,
    source_range: Option<super::SourceRange>,
    action: Option<SpanAction>,
}

pub(super) struct ChunkBuild {
    pub chunks: Vec<LayoutChunk>,
    pub height: f32,
    pub boxes: Vec<PreviewTextBox>,
    pub decorations: Vec<LayoutDecoration>,
}

pub(super) fn layout_document(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    math_engine: &mut MathEngine,
    tree: &RenderTree,
    width_px: u32,
    scale: f32,
    theme: super::PreviewTheme,
) -> LaidOutDocument {
    let scale = scale.max(0.5);
    let padding = PADDING_DIP * scale;
    let gap = BLOCK_GAP_DIP * scale;
    let content_width = (width_px as f32 - padding * 2.0).max(1.0);
    let mut y = padding;
    let mut blocks = Vec::with_capacity(tree.blocks.len());
    let mut selection_text = String::new();
    let mut boxes = Vec::new();
    let mut formula_count = 0usize;
    let foreground = math_foreground(theme);
    math_engine.prepare_projection(scale, foreground);

    for (block_index, block) in tree.blocks.iter().enumerate() {
        let top = y;
        let mut laid_out = match &block.kind {
            RenderBlockKind::Table(table) => super::table_layout::layout_table(
                font_system,
                fonts,
                math_engine,
                block,
                table,
                padding,
                y,
                content_width,
                scale,
                &mut selection_text,
                &mut formula_count,
                theme,
            ),
            RenderBlockKind::ThematicBreak => {
                let height = 13.0 * scale;
                BlockBuild {
                    height,
                    chunks: Vec::new(),
                    decorations: vec![LayoutDecoration {
                        rect: PreviewRect {
                            x: padding,
                            y: y + height * 0.5,
                            width: content_width,
                            height: scale.max(1.0),
                        },
                        role: DecorationRole::Rule,
                    }],
                    boxes: Vec::new(),
                }
            }
            _ => layout_text_block(
                font_system,
                fonts,
                math_engine,
                block,
                padding,
                y,
                content_width,
                scale,
                &mut selection_text,
                &mut formula_count,
                theme,
            ),
        };
        boxes.append(&mut laid_out.boxes);
        y += laid_out.height;
        blocks.push(LaidOutBlock {
            top,
            bottom: y,
            chunks: laid_out.chunks,
            decorations: laid_out.decorations,
        });
        if block_index + 1 < tree.blocks.len() {
            selection_text.push('\n');
        }
        y += gap;
    }

    let height_px = (y - gap + padding).max(width_px.min(1) as f32);
    LaidOutDocument {
        generation: tree.generation,
        width_px,
        height_px,
        blocks,
        index: std::sync::Arc::new(PreviewTextIndex::new(
            tree.generation,
            selection_text,
            boxes,
        )),
        scale,
        theme,
    }
}

pub(super) struct BlockBuild {
    pub height: f32,
    pub chunks: Vec<LayoutChunk>,
    pub decorations: Vec<LayoutDecoration>,
    pub boxes: Vec<PreviewTextBox>,
}

#[allow(clippy::too_many_arguments)]
fn layout_text_block(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    math_engine: &mut MathEngine,
    block: &RenderBlock,
    padding: f32,
    y: f32,
    content_width: f32,
    scale: f32,
    selection_text: &mut String,
    formula_count: &mut usize,
    theme: super::PreviewTheme,
) -> BlockBuild {
    let indent = block.indent as f32 * INDENT_DIP * scale;
    let quote_extra = matches!(block.kind, RenderBlockKind::Quote)
        .then_some(8.0 * scale)
        .unwrap_or(0.0);
    let x = padding + indent + quote_extra;
    let width = (content_width - indent - quote_extra).max(1.0);
    let metrics = metrics_for(&block.kind, scale);
    let align = matches!(block.kind, RenderBlockKind::DisplayMath).then_some(Align::Center);
    let built = make_chunk(
        font_system,
        fonts,
        math_engine,
        &block.spans,
        x,
        y,
        width,
        metrics,
        align.unwrap_or(Align::Left),
        selection_text,
        formula_count,
        theme,
    );
    let height = built.height.max(metrics.line_height);
    let mut decorations = Vec::new();
    match block.kind {
        RenderBlockKind::Quote => decorations.push(LayoutDecoration {
            rect: PreviewRect {
                x: padding + indent,
                y,
                width: 3.0 * scale,
                height,
            },
            role: DecorationRole::QuoteBar,
        }),
        RenderBlockKind::CodeBlock { .. } | RenderBlockKind::HtmlLiteral => {
            decorations.push(LayoutDecoration {
                rect: PreviewRect {
                    x: x - 7.0 * scale,
                    y: y - 4.0 * scale,
                    width: width + 14.0 * scale,
                    height: height + 8.0 * scale,
                },
                role: DecorationRole::CodeBackground,
            });
        }
        RenderBlockKind::DisplayMath => decorations.push(LayoutDecoration {
            rect: PreviewRect {
                x: x - 7.0 * scale,
                y: y - 3.0 * scale,
                width: width + 14.0 * scale,
                height: height + 6.0 * scale,
            },
            role: DecorationRole::MathBackground,
        }),
        _ => {}
    }
    BlockBuild {
        height,
        chunks: built.chunks,
        decorations: {
            decorations.extend(built.decorations);
            decorations
        },
        boxes: built.boxes,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_chunk(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    math_engine: &mut MathEngine,
    spans: &[RenderSpan],
    x: f32,
    y: f32,
    width: f32,
    metrics: Metrics,
    align: Align,
    selection_text: &mut String,
    formula_count: &mut usize,
    theme: super::PreviewTheme,
) -> ChunkBuild {
    if spans.iter().all(|span| span.math.is_none()) {
        return make_text_chunk(
            font_system,
            fonts,
            spans,
            x,
            y,
            width,
            metrics,
            align,
            Wrap::WordOrGlyph,
            selection_text,
        );
    }
    super::math_layout::make_mixed_chunk(
        font_system,
        fonts,
        math_engine,
        spans,
        x,
        y,
        width,
        metrics,
        align,
        selection_text,
        formula_count,
        theme,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn make_text_chunk(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    x: f32,
    y: f32,
    width: f32,
    metrics: Metrics,
    align: Align,
    wrap: Wrap,
    selection_text: &mut String,
) -> ChunkBuild {
    let mut visual = String::new();
    let mut attributed = Vec::new();
    let mut segments = Vec::with_capacity(spans.len());
    for (index, span) in spans.iter().enumerate() {
        let visual_start = visual.len();
        visual.push_str(&span.text);
        let visual_end = visual.len();
        let selection_start = selection_text.len();
        selection_text.push_str(&span.copy_text);
        let selection_end = selection_text.len();
        segments.push(Segment {
            visual_range: visual_start..visual_end,
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            action: span.action.clone(),
        });
        append_attributed_runs(
            &visual,
            visual_start..visual_end,
            index + 1,
            span.style,
            metrics,
            fonts,
            &mut attributed,
        );
    }
    if visual.is_empty() {
        visual.push(' ');
        attributed.push((0..1, Attrs::new().metrics(metrics)));
    }

    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(Some(width), None);
    buffer.set_wrap(wrap);
    let default = Attrs::new().family(Family::Serif).metrics(metrics);
    buffer.set_rich_text(
        attributed
            .iter()
            .map(|(range, attrs)| (&visual[range.clone()], attrs.clone())),
        &default,
        Shaping::Advanced,
        Some(align),
    );
    buffer.shape_until_scroll(font_system, false);
    let height = buffer
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .fold(metrics.line_height, f32::max);
    let boxes = boxes_for_buffer(&buffer, &segments, x, y);
    ChunkBuild {
        chunks: vec![LayoutChunk {
            content: LayoutContent::Text(buffer),
            x,
            y,
        }],
        height,
        boxes,
        decorations: Vec::new(),
    }
}

pub(super) fn math_foreground(theme: super::PreviewTheme) -> [u8; 4] {
    match theme {
        super::PreviewTheme::Light => [40, 38, 34, 255],
        super::PreviewTheme::Dark => [226, 223, 214, 255],
    }
}

#[allow(clippy::too_many_arguments)]
fn append_attributed_runs(
    visual: &str,
    range: Range<usize>,
    metadata: usize,
    style: RenderStyle,
    metrics: Metrics,
    fonts: &FontSelection,
    output: &mut Vec<(Range<usize>, Attrs<'static>)>,
) {
    let text = &visual[range.clone()];
    if text.is_empty() {
        return;
    }
    if style.code || style.math_placeholder || style.html_literal {
        output.push((
            range,
            styled_attrs(Family::Monospace, metadata, style, metrics),
        ));
        return;
    }
    for run in segment_script_runs(text) {
        let family = match run.class {
            ScriptClass::Cjk => Family::Name(fonts.cjk_family),
            ScriptClass::Latin => Family::Name(fonts.latin_family),
        };
        output.push((
            (range.start + run.range.start)..(range.start + run.range.end),
            styled_attrs(family, metadata, style, metrics),
        ));
    }
}

fn styled_attrs(
    family: Family<'static>,
    metadata: usize,
    style: RenderStyle,
    metrics: Metrics,
) -> Attrs<'static> {
    let mut attrs = Attrs::new()
        .family(family)
        .metadata(metadata)
        .metrics(metrics);
    if style.strong {
        attrs = attrs.weight(Weight::BOLD);
    }
    if style.emphasis {
        attrs = attrs.style(Style::Italic);
    }
    if style.strikethrough {
        attrs = attrs.strikethrough();
    }
    if style.link {
        attrs = attrs.underline(UnderlineStyle::Single);
    }
    attrs
}

fn boxes_for_buffer(buffer: &Buffer, segments: &[Segment], x: f32, y: f32) -> Vec<PreviewTextBox> {
    let mut boxes = Vec::new();
    for run in buffer.layout_runs() {
        let mut extents = vec![None::<(f32, f32, usize, usize)>; segments.len()];
        for glyph in run.glyphs {
            let Some(index) = glyph.metadata.checked_sub(1) else {
                continue;
            };
            let Some(extent) = extents.get_mut(index) else {
                continue;
            };
            let left = glyph.x.min(glyph.x + glyph.w);
            let right = glyph.x.max(glyph.x + glyph.w);
            let visual_start = glyph.start.max(segments[index].visual_range.start);
            let visual_end = glyph.end.min(segments[index].visual_range.end);
            if visual_start >= visual_end {
                continue;
            }
            *extent = Some(extent.map_or(
                (left, right, visual_start, visual_end),
                |(current_left, current_right, current_start, current_end)| {
                    (
                        current_left.min(left),
                        current_right.max(right),
                        current_start.min(visual_start),
                        current_end.max(visual_end),
                    )
                },
            ));
        }
        for (index, extent) in extents.into_iter().enumerate() {
            let (left, right, visual_start, visual_end) = match extent {
                Some(extent) => extent,
                None => continue,
            };
            let segment = &segments[index];
            let selection_range =
                selection_range_for_visual_line(segment, visual_start..visual_end);
            if selection_range.is_empty() {
                continue;
            }
            boxes.push(PreviewTextBox {
                selection_range,
                source_range: segment.source_range,
                rect: PreviewRect {
                    x: x + left,
                    y: y + run.line_top,
                    width: (right - left).max(1.0),
                    height: run.line_height,
                },
                action: segment.action.clone(),
                tooltip: None,
                atomic: false,
            });
        }
    }
    boxes
}

fn selection_range_for_visual_line(segment: &Segment, visual: Range<usize>) -> Range<usize> {
    if segment.visual_range.len() != segment.selection_range.len() {
        return segment.selection_range.clone();
    }
    let start = visual.start.saturating_sub(segment.visual_range.start);
    let end = visual.end.saturating_sub(segment.visual_range.start);
    (segment.selection_range.start + start)..(segment.selection_range.start + end)
}

fn metrics_for(kind: &RenderBlockKind, scale: f32) -> Metrics {
    match kind {
        RenderBlockKind::Heading(level) => {
            let multiplier = match level {
                1 => 1.75,
                2 => 1.45,
                3 => 1.25,
                _ => 1.1,
            };
            Metrics::new(
                BODY_SIZE_DIP * multiplier * scale,
                BODY_LINE_DIP * multiplier * scale,
            )
        }
        RenderBlockKind::CodeBlock { .. } | RenderBlockKind::HtmlLiteral => {
            Metrics::new(CODE_SIZE_DIP * scale, CODE_LINE_DIP * scale)
        }
        _ => Metrics::new(BODY_SIZE_DIP * scale, BODY_LINE_DIP * scale),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;
    use crate::preview::{PreviewParser, RenderTreeBuilder};

    fn layout(source: &str, width: u32) -> LaidOutDocument {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let owned = PreviewParser.parse(&snapshot).unwrap();
        let tree = RenderTreeBuilder.build(&owned);
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        layout_document(
            &mut font_system,
            &fonts,
            &mut MathEngine::new(),
            &tree,
            width,
            1.0,
            crate::preview::PreviewTheme::Light,
        )
    }

    #[test]
    fn layout_produces_selectable_native_text_and_source_ranges() {
        let document = layout("# 标题\n\n[text](https://example.com) $x$", 520);
        assert!(document.height_px > 0.0);
        assert_eq!(document.index.generation(), Generation::initial());
        assert!(document.index.text().contains("标题"));
        assert!(document.index.text().contains("$x$"));
        assert!(
            document
                .index
                .boxes()
                .iter()
                .any(|item| item.action.is_some())
        );
        assert!(
            document
                .index
                .boxes()
                .iter()
                .all(|item| item.source_range.is_some() || item.action.is_none())
        );
    }

    #[test]
    fn narrower_width_reflows_without_changing_generation_or_text_projection() {
        let wide = layout(&"中文 English ".repeat(30), 900);
        let narrow = layout(&"中文 English ".repeat(30), 300);
        assert_eq!(wide.generation, narrow.generation);
        assert_eq!(wide.index.text(), narrow.index.text());
        assert!(narrow.height_px > wide.height_px);
    }

    #[test]
    fn wrapped_visual_rows_map_to_disjoint_selectable_byte_ranges() {
        let source = "alpha beta gamma delta epsilon ".repeat(20);
        let document = layout(&source, 220);
        let boxes = document.index.boxes();
        assert!(boxes.len() > 2, "fixture must wrap across visual rows");
        assert!(
            boxes
                .windows(2)
                .all(|pair| pair[0].selection_range.end <= pair[1].selection_range.start),
            "wrapped rows must not each claim the entire Markdown span"
        );
        assert_eq!(
            boxes.first().map(|item| item.selection_range.start),
            Some(0)
        );
        assert_eq!(
            boxes.last().map(|item| item.selection_range.end),
            Some(document.index.text().len())
        );
    }

    #[test]
    fn rendered_copy_uses_one_newline_between_blocks() {
        let document = layout("# Heading\n\nparagraph", 520);
        assert_eq!(document.index.text(), "Heading\nparagraph");
    }

    #[test]
    fn table_layout_keeps_cells_bounded_inside_document_width() {
        let document = layout("| A | B |\n| :- | -: |\n| 中文 | value |", 420);
        assert!(document.index.boxes().iter().all(|item| {
            item.rect.x >= 0.0 && item.rect.right() <= document.width_px as f32 + 0.5
        }));
        assert!(
            document.blocks[0]
                .decorations
                .iter()
                .any(|item| item.role == DecorationRole::TableHeader)
        );
    }
}
