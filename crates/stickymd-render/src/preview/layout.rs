//! Native block layout derived from `RenderTree`.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::sync::Arc;

use crate::image::{DecodedImageCache, PreviewImageSource};
use crate::math::{MathEngine, MathRaster};
use crate::source::FontSelection;
use cosmic_text::{Align, Buffer, FontSystem, Metrics, Wrap};
use stickymd_core::Generation;

use super::scroll::PreviewScrollAnchor;
use super::{
    PreviewRect, PreviewTextBox, PreviewTextIndex, RenderBlock, RenderBlockKind, RenderSpan,
    RenderTree,
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

pub(super) struct InlinePiece {
    pub chunk: LayoutChunk,
    pub boxes: Vec<PreviewTextBox>,
    pub decorations: Vec<LayoutDecoration>,
    pub width: f32,
    pub height: f32,
    pub baseline: f32,
}

pub(super) enum LayoutContent {
    Text(Buffer),
    Math(Arc<MathRaster>),
    Image(Arc<crate::image::DecodedImageRaster>),
    ImagePlaceholder { width: u32, height: u32 },
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

pub(super) struct ChunkBuild {
    pub chunks: Vec<LayoutChunk>,
    pub height: f32,
    pub boxes: Vec<PreviewTextBox>,
    pub decorations: Vec<LayoutDecoration>,
}

/// Mutable layout services are grouped as one short-lived capability bundle;
/// the document inputs remain explicit values.
pub(super) struct LayoutResources<'a> {
    pub font_system: &'a mut FontSystem,
    pub fonts: &'a FontSelection,
    pub math_engine: &'a mut MathEngine,
    pub image_source: Option<&'a dyn PreviewImageSource>,
    pub image_cache: &'a mut DecodedImageCache,
    pub image_band: (f32, f32),
}

pub(super) fn layout_document(
    resources: LayoutResources<'_>,
    tree: &RenderTree,
    width_px: u32,
    scale: f32,
    theme: super::PreviewTheme,
) -> LaidOutDocument {
    let LayoutResources {
        font_system,
        fonts,
        math_engine,
        image_source,
        image_cache,
        image_band,
    } = resources;
    let scale = scale.max(0.5);
    let padding = PADDING_DIP * scale;
    let gap = BLOCK_GAP_DIP * scale;
    let content_width = (width_px as f32 - padding * 2.0).max(1.0);
    let mut y = padding;
    let mut blocks = Vec::with_capacity(tree.blocks.len());
    let mut selection_text = String::new();
    let mut boxes = Vec::new();
    let mut scroll_anchors = Vec::with_capacity(tree.blocks.len());
    let mut formula_count = 0usize;
    let mut text_layout_cache = super::text_layout::TextLayoutCache::default();
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
                image_source,
                image_cache,
                image_band,
                &mut text_layout_cache,
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
                image_source,
                image_cache,
                image_band,
                &mut text_layout_cache,
            ),
        };
        boxes.append(&mut laid_out.boxes);
        y += laid_out.height;
        if let Some(source_range) = block.source_range {
            scroll_anchors.push(PreviewScrollAnchor {
                source_range,
                top,
                bottom: y,
            });
        }
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
            scroll_anchors,
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
    image_source: Option<&dyn PreviewImageSource>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
    text_layout_cache: &mut super::text_layout::TextLayoutCache,
) -> BlockBuild {
    let indent = block.indent as f32 * INDENT_DIP * scale;
    let quote_extra = matches!(block.kind, RenderBlockKind::Quote)
        .then_some(8.0 * scale)
        .unwrap_or(0.0);
    let x = padding + indent + quote_extra;
    let width = (content_width - indent - quote_extra).max(1.0);
    let metrics = metrics_for(&block.kind, scale);
    if let Some(image) = super::image_layout::layout_image_block(
        block,
        x,
        y,
        width,
        scale,
        selection_text,
        image_source,
        image_cache,
        image_band,
    ) {
        return image;
    }
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
        image_source,
        image_cache,
        image_band,
        text_layout_cache,
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
    image_source: Option<&dyn PreviewImageSource>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
    text_layout_cache: &mut super::text_layout::TextLayoutCache,
) -> ChunkBuild {
    if spans
        .iter()
        .all(|span| span.math.is_none() && span.image.is_none())
    {
        return super::text_layout::make_text_chunk(
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
            text_layout_cache,
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
        image_source,
        image_cache,
        image_band,
        text_layout_cache,
    )
}

pub(super) fn math_foreground(theme: super::PreviewTheme) -> [u8; 4] {
    match theme {
        super::PreviewTheme::Light => [40, 38, 34, 255],
        super::PreviewTheme::Dark => [226, 223, 214, 255],
    }
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
            LayoutResources {
                font_system: &mut font_system,
                fonts: &fonts,
                math_engine: &mut MathEngine::new(),
                image_source: None,
                image_cache: &mut DecodedImageCache::default(),
                image_band: (0.0, f32::MAX),
            },
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
