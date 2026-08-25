//! Bounded text-piece construction for mixed inline preview layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::{Align, FontSystem, Metrics, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use crate::source::FontSelection;

use super::RenderSpan;
use super::layout::{InlinePiece, LayoutContent};
use super::text_layout::{TextLayoutCache, make_text_chunk};

// The arguments are one cohesive layout operation; bundling them would hide
// the mutable service lifetimes and make aliasing constraints less explicit.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_text_pieces(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    metrics: Metrics,
    selection_text: &mut String,
    pieces: &mut Vec<InlinePiece>,
    coalesce_short_text: bool,
    coalesce_attributed_short_text: bool,
    max_text_piece_chars: usize,
    text_layout_cache: &mut TextLayoutCache,
) {
    if spans.is_empty() {
        return;
    }
    if coalesce_short_text && coalesce_attributed_short_text {
        append_bounded_attributed_text_pieces(
            font_system,
            fonts,
            spans,
            metrics,
            selection_text,
            pieces,
            max_text_piece_chars,
            text_layout_cache,
        );
        return;
    }
    // Small notes retain the established uniform-only fast path. Cosmic-text
    // can shape mixed attributes in one buffer, but doing so repeatedly during
    // zoom is more expensive for a typical short note.
    if coalesce_short_text
        && text_run_is_short(spans, max_text_piece_chars)
        && text_run_is_uniform(spans)
    {
        pieces.push(text_spans_piece(
            font_system,
            fonts,
            spans,
            metrics,
            selection_text,
            text_layout_cache,
        ));
        return;
    }
    for span in spans {
        append_single_text_span(
            font_system,
            fonts,
            span,
            metrics,
            selection_text,
            pieces,
            text_layout_cache,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn append_bounded_attributed_text_pieces(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    metrics: Metrics,
    selection_text: &mut String,
    pieces: &mut Vec<InlinePiece>,
    max_chars: usize,
    text_layout_cache: &mut TextLayoutCache,
) {
    let mut batch_start = 0;
    let mut batch_chars = 0;
    for (index, span) in spans.iter().enumerate() {
        let span_chars = span.text.chars().count();
        if span_chars > max_chars {
            append_text_batch(
                font_system,
                fonts,
                &spans[batch_start..index],
                metrics,
                selection_text,
                pieces,
                text_layout_cache,
            );
            append_single_text_span(
                font_system,
                fonts,
                span,
                metrics,
                selection_text,
                pieces,
                text_layout_cache,
            );
            batch_start = index + 1;
            batch_chars = 0;
        } else if batch_chars > 0 && batch_chars + span_chars > max_chars {
            append_text_batch(
                font_system,
                fonts,
                &spans[batch_start..index],
                metrics,
                selection_text,
                pieces,
                text_layout_cache,
            );
            batch_start = index;
            batch_chars = span_chars;
        } else {
            batch_chars += span_chars;
        }
    }
    append_text_batch(
        font_system,
        fonts,
        &spans[batch_start..],
        metrics,
        selection_text,
        pieces,
        text_layout_cache,
    );
}

fn append_text_batch(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    metrics: Metrics,
    selection_text: &mut String,
    pieces: &mut Vec<InlinePiece>,
    text_layout_cache: &mut TextLayoutCache,
) {
    if !spans.is_empty() {
        pieces.push(text_spans_piece(
            font_system,
            fonts,
            spans,
            metrics,
            selection_text,
            text_layout_cache,
        ));
    }
}

fn append_single_text_span(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    span: &RenderSpan,
    metrics: Metrics,
    selection_text: &mut String,
    pieces: &mut Vec<InlinePiece>,
    text_layout_cache: &mut TextLayoutCache,
) {
    if span.text == span.copy_text && span.text.chars().count() > 48 {
        for token in span
            .text
            .split_word_bounds()
            .filter(|token| !token.is_empty())
        {
            let token: std::sync::Arc<str> = std::sync::Arc::from(token);
            let token_span = RenderSpan {
                text: std::sync::Arc::clone(&token),
                copy_text: token,
                source_range: span.source_range,
                style: span.style,
                action: span.action.clone(),
                math: None,
                image: None,
                hard_break: false,
            };
            pieces.push(text_piece(
                font_system,
                fonts,
                &token_span,
                metrics,
                selection_text,
                text_layout_cache,
            ));
        }
    } else {
        pieces.push(text_piece(
            font_system,
            fonts,
            span,
            metrics,
            selection_text,
            text_layout_cache,
        ));
    }
}

fn text_run_is_short(spans: &[RenderSpan], max_chars: usize) -> bool {
    let mut count = 0;
    for span in spans {
        for _ in span.text.chars() {
            count += 1;
            if count > max_chars {
                return false;
            }
        }
    }
    true
}

fn text_run_is_uniform(spans: &[RenderSpan]) -> bool {
    let Some(first) = spans.first() else {
        return false;
    };
    spans
        .iter()
        .all(|span| span.style == first.style && span.action == first.action)
}

pub(super) fn text_piece(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    span: &RenderSpan,
    metrics: Metrics,
    selection_text: &mut String,
    text_layout_cache: &mut TextLayoutCache,
) -> InlinePiece {
    text_spans_piece(
        font_system,
        fonts,
        std::slice::from_ref(span),
        metrics,
        selection_text,
        text_layout_cache,
    )
}

fn text_spans_piece(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    spans: &[RenderSpan],
    metrics: Metrics,
    selection_text: &mut String,
    text_layout_cache: &mut TextLayoutCache,
) -> InlinePiece {
    let mut built = make_text_chunk(
        font_system,
        fonts,
        spans,
        0.0,
        0.0,
        1_000_000.0,
        metrics,
        Align::Left,
        Wrap::None,
        selection_text,
        text_layout_cache,
    );
    let mut chunk = built.chunks.remove(0);
    let (width, baseline) = match &chunk.content {
        LayoutContent::Text(buffer) => {
            let mut runs = buffer.layout_runs();
            let first = runs.next();
            let width = first.as_ref().map_or(1.0, |run| run.line_w.max(1.0));
            let baseline = first.map_or(metrics.font_size, |run| run.line_y);
            (width, baseline)
        }
        LayoutContent::Math(_) => (1.0, metrics.font_size),
        LayoutContent::Image(raster) => (raster.width as f32, raster.height as f32),
        LayoutContent::ImagePlaceholder { width, height } => (*width as f32, *height as f32),
    };
    chunk.x = 0.0;
    chunk.y = 0.0;
    InlinePiece {
        chunk,
        boxes: built.boxes,
        decorations: built.decorations,
        width,
        height: built.height,
        baseline,
    }
}
