//! Atomic inline/display formula placement inside native preview layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::sync::Arc;

use cosmic_text::{Align, FontSystem, Metrics};

use crate::image::{
    DecodedImageCache, ImageCacheKey, PreviewImageSource, decode_scaled_image_owned,
    inspect_encoded_image,
};
use crate::math::{MAX_DOCUMENT_FORMULAS, MathEngine, MathError, MathRaster};
use crate::source::FontSelection;

use super::image_layout::image_target;
use super::inline_text_layout::{append_text_pieces, text_piece};
use super::layout::{
    ChunkBuild, DecorationRole, InlinePiece, LayoutChunk, LayoutContent, LayoutDecoration,
    math_foreground,
};
use super::text_layout::TextLayoutCache;
use super::{PreviewRect, PreviewTextBox, RenderSpan};

const ATTRIBUTED_TEXT_COALESCE_AFTER_SOURCE_BYTES: usize = 64 * 1024;

#[allow(clippy::too_many_arguments)]
pub(super) fn make_mixed_chunk(
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
    text_layout_cache: &mut TextLayoutCache,
) -> ChunkBuild {
    let mut pieces = Vec::new();
    let mut line_breaks = Vec::new();
    let mut pending_text_start = 0;
    // Coalescing pays for itself in mixed projection runs because it avoids a
    // buffer per short fragment around atomic math/image content. Attributed
    // runs remain fine-grained near the start of typical notes, where repeated
    // zoom relayout is faster. Once source offsets prove the note is large,
    // short attributed runs are merged to bound layout-object proliferation.
    let coalesce_short_text = spans
        .iter()
        .any(|span| span.math.is_some() || span.image.is_some());
    let coalesce_attributed_short_text = spans.iter().any(|span| {
        span.source_range
            .is_some_and(|range| range.end > ATTRIBUTED_TEXT_COALESCE_AFTER_SOURCE_BYTES)
    });
    let max_text_piece_chars = (width / metrics.font_size.max(1.0))
        .floor()
        .clamp(1.0, 48.0) as usize;
    for (index, span) in spans.iter().enumerate() {
        if span.hard_break {
            append_text_pieces(
                font_system,
                fonts,
                &spans[pending_text_start..index],
                metrics,
                selection_text,
                &mut pieces,
                coalesce_short_text,
                coalesce_attributed_short_text,
                max_text_piece_chars,
                text_layout_cache,
            );
            selection_text.push_str(&span.copy_text);
            line_breaks.push(pieces.len());
            pending_text_start = index + 1;
            continue;
        }
        if let Some(math) = &span.math {
            append_text_pieces(
                font_system,
                fonts,
                &spans[pending_text_start..index],
                metrics,
                selection_text,
                &mut pieces,
                coalesce_short_text,
                coalesce_attributed_short_text,
                max_text_piece_chars,
                text_layout_cache,
            );
            *formula_count = formula_count.saturating_add(1);
            let error =
                (*formula_count > MAX_DOCUMENT_FORMULAS).then_some(MathError::TooManyFormulas);
            let rendered = error.map_or_else(
                || {
                    math_engine.render(
                        Arc::clone(&math.source),
                        math.kind,
                        metrics.font_size,
                        math_foreground(theme),
                    )
                },
                Err,
            );
            pieces.push(match rendered {
                Ok(raster) => formula_piece(span, raster, selection_text),
                Err(error) => error_piece(
                    font_system,
                    fonts,
                    span,
                    metrics,
                    selection_text,
                    error,
                    text_layout_cache,
                ),
            });
            pending_text_start = index + 1;
            continue;
        }
        if span.image.is_some() {
            append_text_pieces(
                font_system,
                fonts,
                &spans[pending_text_start..index],
                metrics,
                selection_text,
                &mut pieces,
                coalesce_short_text,
                coalesce_attributed_short_text,
                max_text_piece_chars,
                text_layout_cache,
            );
            if let Some(piece) = image_piece(
                span,
                width,
                y,
                metrics,
                selection_text,
                image_source,
                image_cache,
                image_band,
            ) {
                pieces.push(piece);
                pending_text_start = index + 1;
            } else {
                pending_text_start = index;
            }
        }
    }
    append_text_pieces(
        font_system,
        fonts,
        &spans[pending_text_start..],
        metrics,
        selection_text,
        &mut pieces,
        coalesce_short_text,
        coalesce_attributed_short_text,
        max_text_piece_chars,
        text_layout_cache,
    );

    let mut output = ChunkBuild {
        chunks: Vec::new(),
        height: 0.0,
        boxes: Vec::new(),
        decorations: Vec::new(),
    };
    let mut line = Vec::new();
    let mut line_width = 0.0;
    let mut line_y = y;
    let mut next_break = 0;
    for (piece_index, piece) in pieces.into_iter().enumerate() {
        while line_breaks.get(next_break).copied() == Some(piece_index) {
            line_y += flush_or_advance_line(
                &mut line,
                &mut line_width,
                x,
                line_y,
                width,
                metrics,
                align,
                &mut output,
            );
            next_break += 1;
        }
        if !line.is_empty() && line_width + piece.width > width {
            line_y += flush_line(
                &mut line,
                line_width,
                x,
                line_y,
                width,
                metrics,
                align,
                &mut output,
            );
            line_width = 0.0;
        }
        line_width += piece.width;
        line.push(piece);
    }
    while next_break < line_breaks.len() {
        line_y += flush_or_advance_line(
            &mut line,
            &mut line_width,
            x,
            line_y,
            width,
            metrics,
            align,
            &mut output,
        );
        next_break += 1;
    }
    if !line.is_empty() {
        line_y += flush_line(
            &mut line,
            line_width,
            x,
            line_y,
            width,
            metrics,
            align,
            &mut output,
        );
    }
    output.height = (line_y - y).max(metrics.line_height);
    output
}

#[allow(clippy::too_many_arguments)]
fn flush_or_advance_line(
    line: &mut Vec<InlinePiece>,
    line_width: &mut f32,
    x: f32,
    y: f32,
    available_width: f32,
    metrics: Metrics,
    align: Align,
    output: &mut ChunkBuild,
) -> f32 {
    if line.is_empty() {
        metrics.line_height
    } else {
        let height = flush_line(
            line,
            *line_width,
            x,
            y,
            available_width,
            metrics,
            align,
            output,
        );
        *line_width = 0.0;
        height
    }
}

#[allow(clippy::too_many_arguments)]
fn image_piece(
    span: &RenderSpan,
    available_width: f32,
    block_y: f32,
    metrics: Metrics,
    selection_text: &mut String,
    image_source: Option<&dyn PreviewImageSource>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
) -> Option<InlinePiece> {
    let image = span.image.as_ref()?;
    if !matches!(
        image.kind,
        super::ImageKind::LocalRelative | super::ImageKind::LocalAbsolute
    ) {
        return None;
    }
    let image_source = image_source?;
    let metadata = image_source.inspect(&image.destination).ok().flatten()?;
    let max_width = available_width.floor().max(1.0) as u32;
    // An inline image participates in line layout. Cap it to four body lines;
    // standalone image paragraphs retain the larger viewport-oriented cap.
    let max_height = (metrics.line_height * 4.0).floor().max(1.0) as u32;
    let (mut target_width, mut target_height) = image_target(&metadata, max_width, max_height);
    let in_decode_band = block_y + target_height as f32 >= image_band.0 && block_y <= image_band.1;
    let content = if in_decode_band {
        let bytes = image_source.load(&image.destination).ok().flatten()?;
        let current_metadata = inspect_encoded_image(&bytes).ok()?;
        (target_width, target_height) = image_target(&current_metadata, max_width, max_height);
        let key = ImageCacheKey {
            source_hash: stickymd_core::hash_bytes(&bytes),
            width: target_width,
            height: target_height,
        };
        let raster = if let Some(raster) = image_cache.get(&key) {
            raster
        } else {
            decode_scaled_image_owned(bytes, target_width, target_height)
                .ok()
                .and_then(|raster| image_cache.insert(key, raster))?
        };
        LayoutContent::Image(raster)
    } else {
        LayoutContent::ImagePlaceholder {
            width: target_width,
            height: target_height,
        }
    };
    let selection_start = selection_text.len();
    selection_text.push_str(&span.copy_text);
    let selection_end = selection_text.len();
    let width = target_width as f32;
    let height = target_height as f32;
    Some(InlinePiece {
        chunk: LayoutChunk {
            content,
            x: 0.0,
            y: 0.0,
        },
        boxes: vec![PreviewTextBox {
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            rect: PreviewRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            action: span.action.clone(),
            tooltip: None,
            atomic: true,
        }],
        decorations: Vec::new(),
        width,
        height,
        baseline: height,
    })
}

fn formula_piece(
    span: &RenderSpan,
    raster: Arc<MathRaster>,
    selection_text: &mut String,
) -> InlinePiece {
    let selection_start = selection_text.len();
    selection_text.push_str(&span.copy_text);
    let selection_end = selection_text.len();
    let width = raster.width as f32;
    let height = raster.height as f32;
    let baseline = raster.baseline;
    InlinePiece {
        chunk: LayoutChunk {
            content: LayoutContent::Math(raster),
            x: 0.0,
            y: 0.0,
        },
        boxes: vec![PreviewTextBox {
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            rect: PreviewRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            action: span.action.clone(),
            tooltip: None,
            atomic: true,
        }],
        decorations: Vec::new(),
        width,
        height,
        baseline,
    }
}

fn error_piece(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    span: &RenderSpan,
    metrics: Metrics,
    selection_text: &mut String,
    error: MathError,
    text_layout_cache: &mut TextLayoutCache,
) -> InlinePiece {
    let mut fallback = span.clone();
    fallback.math = None;
    fallback.style.math_placeholder = true;
    let mut piece = text_piece(
        font_system,
        fonts,
        &fallback,
        metrics,
        selection_text,
        text_layout_cache,
    );
    for text_box in &mut piece.boxes {
        text_box.atomic = true;
        text_box.tooltip = Some(Arc::from(error.to_string()));
    }
    piece.decorations.push(LayoutDecoration {
        rect: PreviewRect {
            x: 0.0,
            y: 0.0,
            width: piece.width,
            height: piece.height,
        },
        role: DecorationRole::MathError,
    });
    piece
}

#[allow(clippy::too_many_arguments)]
fn flush_line(
    line: &mut Vec<InlinePiece>,
    line_width: f32,
    x: f32,
    y: f32,
    available_width: f32,
    metrics: Metrics,
    align: Align,
    output: &mut ChunkBuild,
) -> f32 {
    let baseline = line
        .iter()
        .map(|piece| piece.baseline)
        .fold(metrics.font_size, f32::max);
    let descent = line
        .iter()
        .map(|piece| (piece.height - piece.baseline).max(0.0))
        .fold((metrics.line_height - metrics.font_size).max(0.0), f32::max);
    let line_height = (baseline + descent).max(metrics.line_height);
    let align_offset = match align {
        Align::Center => ((available_width - line_width) * 0.5).max(0.0),
        Align::Right | Align::End => (available_width - line_width).max(0.0),
        _ => 0.0,
    };
    let mut cursor_x = x + align_offset;
    for mut piece in line.drain(..) {
        let offset_y = y + baseline - piece.baseline;
        piece.chunk.x = cursor_x;
        piece.chunk.y = offset_y;
        for text_box in &mut piece.boxes {
            text_box.rect.x += cursor_x;
            text_box.rect.y += offset_y;
        }
        for decoration in &mut piece.decorations {
            decoration.rect.x += cursor_x;
            decoration.rect.y += offset_y;
        }
        cursor_x += piece.width;
        output.chunks.push(piece.chunk);
        output.boxes.extend(piece.boxes);
        output.decorations.extend(piece.decorations);
    }
    line_height
}

#[cfg(test)]
mod tests {
    use cosmic_text::{Align, FontSystem, Metrics};

    use super::make_mixed_chunk;
    use crate::math::{MathEngine, MathKind};
    use crate::preview::render_tree::{RenderMath, SpanAction};
    use crate::preview::text_layout::TextLayoutCache;
    use crate::preview::{LinkKind, PreviewTheme, RenderSpan, RenderStyle, SourceRange};
    use crate::source::FontSelection;

    fn text_span(text: &str, style: RenderStyle) -> RenderSpan {
        let text: std::sync::Arc<str> = std::sync::Arc::from(text);
        let text_len = text.len();
        RenderSpan {
            text: std::sync::Arc::clone(&text),
            copy_text: text,
            source_range: SourceRange::new(0, text_len),
            style,
            action: None,
            math: None,
            image: None,
            hard_break: false,
        }
    }

    #[test]
    fn phase11_short_adjacent_text_around_math_uses_one_buffer_per_side() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut selection_text = String::new();
        let mut formula_count = 0;
        let mut math = text_span("$x^2$", RenderStyle::default());
        math.math = Some(RenderMath {
            source: std::sync::Arc::from("x^2"),
            kind: MathKind::Inline,
        });
        let spans = vec![
            text_span("before ", RenderStyle::default()),
            text_span("plain", RenderStyle::default()),
            math,
            text_span(" after ", RenderStyle::default()),
            text_span("tail", RenderStyle::default()),
        ];

        let built = make_mixed_chunk(
            &mut font_system,
            &fonts,
            &mut math_engine,
            &spans,
            0.0,
            0.0,
            1_000.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            &mut selection_text,
            &mut formula_count,
            PreviewTheme::Light,
            None,
            &mut crate::image::DecodedImageCache::default(),
            (0.0, f32::MAX),
            &mut TextLayoutCache::default(),
        );

        assert_eq!(built.chunks.len(), 3);
        assert_eq!(selection_text, "before plain$x^2$ after tail");
    }

    #[test]
    fn phase11_large_document_policy_coalesces_short_attributed_runs() {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut selection_text = String::new();
        let mut formula_count = 0;
        let mut linked = text_span("link", RenderStyle::default());
        linked.action = Some(SpanAction::OpenLink {
            destination: "https://example.com".to_owned(),
            kind: LinkKind::Https,
        });
        let mut math = text_span("$x$", RenderStyle::default());
        math.math = Some(RenderMath {
            source: std::sync::Arc::from("x"),
            kind: MathKind::Inline,
        });
        let mut before = text_span("before ", RenderStyle::default());
        before.source_range = SourceRange::new(70_000, 70_007);
        linked.source_range = SourceRange::new(70_007, 70_011);
        math.source_range = SourceRange::new(70_011, 70_014);
        let spans = vec![before, linked, math];

        let built = make_mixed_chunk(
            &mut font_system,
            &fonts,
            &mut math_engine,
            &spans,
            0.0,
            0.0,
            1_000.0,
            Metrics::new(17.0, 26.35),
            Align::Left,
            &mut selection_text,
            &mut formula_count,
            PreviewTheme::Light,
            None,
            &mut crate::image::DecodedImageCache::default(),
            (0.0, f32::MAX),
            &mut TextLayoutCache::default(),
        );

        assert_eq!(built.chunks.len(), 2);
        assert_eq!(selection_text, "before link$x$");
        assert!(built.boxes.iter().any(|text_box| text_box.action.is_some()));
    }
}
