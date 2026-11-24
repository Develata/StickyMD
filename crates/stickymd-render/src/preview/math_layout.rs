//! Atomic inline/display formula placement inside native preview layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{Align, FontSystem, Metrics};

use crate::image::DecodedImageCache;
use crate::math::{MAX_DOCUMENT_FORMULAS, MathEngine, MathError, MathKind, MathMetrics};
use crate::source::FontSelection;

use super::copy::SelectionText;
use super::image_requests::ImageRequests;
use super::image_resources::{Admission, LaidOutImage};
use super::inline_text_layout::{append_text_pieces, text_piece};
use super::layout::{
    ChunkBuild, DecorationRole, InlinePiece, LayoutChunk, LayoutContent, LayoutDecoration,
};
use super::text_layout::TextLayoutCache;
use super::{PreviewRect, PreviewTextBox, RenderSpan};

mod line_flow;

use line_flow::LineFlow;

const ATTRIBUTED_TEXT_COALESCE_AFTER_SOURCE_BYTES: usize = 64 * 1024;

/// Full-document geometry never owns pixels or a pinned RaTeX DisplayList.
pub(super) struct LaidOutMath {
    pub source: Arc<str>,
    pub kind: MathKind,
    pub font_size: f32,
    pub metrics: MathMetrics,
    /// The selection range of this formula's atomic box: its identity when paint reports a
    /// failure back onto that box.
    pub selection: std::ops::Range<usize>,
}

/// What final admission decided about an image span, fixed for the next layout pass.
enum ImageFate {
    /// The bytes decode to these dimensions: lay the image out at them.
    Sized { width: u32, height: u32 },
    /// The bytes cannot be read or decoded: the span falls back to text.
    Failed,
}

/// Lays the spans out. An inline image is probed for its size while the pieces are built and
/// admitted (read, decoded) only at its final position, after its line is placed. When that
/// shows other dimensions or a failure, the chunk is laid out again with the image's fate fixed,
/// from the same starting selection state. The first image whose admission changes the geometry
/// ends the pass, since every position after it is stale: later images are decided only on the
/// re-placed lines. Each pass fixes one more image, so the loop ends.
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
    selection_text: &mut SelectionText,
    formula_count: &mut usize,
    theme: super::PreviewTheme,
    mut image_source: Option<&mut ImageRequests<'_>>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
    text_layout_cache: &mut TextLayoutCache,
    embedded_math: bool,
) -> ChunkBuild {
    let mark = selection_text.checkpoint();
    let formulas_before = *formula_count;
    let mut fates: HashMap<usize, ImageFate> = HashMap::new();
    loop {
        let (built, pending) = layout_mixed(
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
            image_source.as_deref_mut(),
            image_cache,
            image_band,
            text_layout_cache,
            embedded_math,
            &fates,
        );
        if pending.is_empty() {
            return built;
        }
        selection_text.rollback(mark);
        *formula_count = formulas_before;
        fates.extend(pending);
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_mixed(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    math_engine: &mut MathEngine,
    spans: &[RenderSpan],
    x: f32,
    y: f32,
    width: f32,
    metrics: Metrics,
    align: Align,
    selection_text: &mut SelectionText,
    formula_count: &mut usize,
    _theme: super::PreviewTheme,
    mut image_source: Option<&mut ImageRequests<'_>>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
    text_layout_cache: &mut TextLayoutCache,
    embedded_math: bool,
    fates: &HashMap<usize, ImageFate>,
) -> (ChunkBuild, Vec<(usize, ImageFate)>) {
    let mut image_spans = Vec::new();
    let mut pieces = Vec::new();
    let mut line_breaks = Vec::new();
    let mut own_line_pieces = Vec::new();
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
            selection_text.push_span(span);
            line_breaks.push(pieces.len());
            pending_text_start = index + 1;
            continue;
        }
        if let Some(math) = &span.math {
            *formula_count = formula_count.saturating_add(1);
            let error =
                (*formula_count > MAX_DOCUMENT_FORMULAS).then_some(MathError::TooManyFormulas);
            let rendered = error.map_or_else(
                || math_engine.measure(Arc::clone(&math.source), math.kind, metrics.font_size),
                Err,
            );
            // A display formula in a paragraph owns its line; a formula that failed to measure
            // stays an inline error text (plan 06 #display-math-in-paragraph).
            let own_line = math.kind == MathKind::Display && rendered.is_ok();
            // The soft break next to such a line is a whitespace-only span: it still enters the
            // copy text, but no piece is laid out for it, like a hard break.
            let text_end =
                if own_line && index > pending_text_start && is_soft_break(&spans[index - 1]) {
                    index - 1
                } else {
                    index
                };
            append_text_pieces(
                font_system,
                fonts,
                &spans[pending_text_start..text_end],
                metrics,
                selection_text,
                &mut pieces,
                coalesce_short_text,
                coalesce_attributed_short_text,
                max_text_piece_chars,
                text_layout_cache,
            );
            if text_end < index {
                selection_text.push_span(&spans[text_end]);
            }
            if own_line {
                own_line_pieces.push(pieces.len());
            }
            pieces.push(match rendered {
                Ok(measured) => formula_piece(
                    span,
                    LaidOutMath {
                        source: Arc::clone(&math.source),
                        kind: math.kind,
                        font_size: metrics.font_size,
                        metrics: measured,
                        selection: 0..0,
                    },
                    selection_text,
                ),
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
            if own_line && spans.get(index + 1).is_some_and(is_soft_break) {
                selection_text.push_span(&spans[index + 1]);
                pending_text_start = index + 2;
            }
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
            let fate = fates.get(&index);
            if let Some(piece) = image_piece(
                span,
                width,
                metrics,
                selection_text,
                image_source.as_deref_mut(),
                fate,
            ) {
                image_spans.push(index);
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

    let mut flow = LineFlow::new(x, y, width, metrics, align, embedded_math);
    let (mut next_break, mut next_own_line) = (0, 0);
    for (piece_index, piece) in pieces.into_iter().enumerate() {
        while line_breaks.get(next_break).copied() == Some(piece_index) {
            flow.hard_break();
            next_break += 1;
        }
        if own_line_pieces.get(next_own_line).copied() == Some(piece_index) {
            next_own_line += 1;
            flow.place_own_line(piece);
        } else {
            flow.place_inline(piece);
        }
    }
    for _ in next_break..line_breaks.len() {
        flow.hard_break();
    }
    let mut built = flow.finish();
    let mut pending = Vec::new();
    if let Some(requests) = image_source {
        let mut spans_of_images = image_spans.iter();
        for chunk in &mut built.chunks {
            let LayoutContent::Image(image) = &mut chunk.content else {
                continue;
            };
            let Some(&span_index) = spans_of_images.next() else {
                break;
            };
            // The first change invalidates every position after it: stop admitting at these
            // coordinates, lay the lines out again, and decide the later images there.
            match image.admit(chunk.y, image_band, requests, image_cache) {
                Admission::Settled => {}
                Admission::Resized { width, height } => {
                    pending.push((span_index, ImageFate::Sized { width, height }));
                    break;
                }
                Admission::Failed => {
                    pending.push((span_index, ImageFate::Failed));
                    break;
                }
            }
        }
    }
    (built, pending)
}

fn is_soft_break(span: &RenderSpan) -> bool {
    span.soft_break
}

fn image_piece(
    span: &RenderSpan,
    available_width: f32,
    metrics: Metrics,
    selection_text: &mut SelectionText,
    image_source: Option<&mut ImageRequests<'_>>,
    fate: Option<&ImageFate>,
) -> Option<InlinePiece> {
    let image = span.image.as_ref()?;
    let max_width = available_width.floor().max(1.0) as u32;
    // An inline image participates in line layout. Cap it to four body lines;
    // standalone image paragraphs retain the larger viewport-oriented cap.
    let max_height = (metrics.line_height * 4.0).floor().max(1.0) as u32;
    let slot = match fate {
        Some(ImageFate::Failed) => return None,
        Some(&ImageFate::Sized { width, height }) => {
            LaidOutImage::sized(image, max_width, max_height, width, height)?
        }
        None => LaidOutImage::probe(image, max_width, max_height, image_source)?,
    };
    let (target_width, target_height) = (slot.width, slot.height);
    let content = LayoutContent::Image(slot);
    let selection = selection_text.push_span(span);
    let width = target_width as f32;
    let height = target_height as f32;
    Some(InlinePiece {
        chunk: LayoutChunk {
            scroll_with_block: true,
            content,
            x: 0.0,
            y: 0.0,
        },
        boxes: vec![PreviewTextBox {
            selection_range: selection.clone(),
            source_range: span.source_range,
            rect: PreviewRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            action: span.action.clone().map(Arc::new),
            tooltip: None,
            atomic: true,
            start_x: 0.0,
            end_x: width,
        }],
        decorations: Vec::new(),
        width,
        height,
        baseline: height,
    })
}

fn formula_piece(
    span: &RenderSpan,
    mut formula: LaidOutMath,
    selection_text: &mut SelectionText,
) -> InlinePiece {
    let selection = selection_text.push_span(span);
    formula.selection = selection.clone();
    let width = formula.metrics.width as f32;
    let height = formula.metrics.height as f32;
    let baseline = formula.metrics.baseline;
    InlinePiece {
        chunk: LayoutChunk {
            scroll_with_block: true,
            content: LayoutContent::Math(formula),
            x: 0.0,
            y: 0.0,
        },
        boxes: vec![PreviewTextBox {
            selection_range: selection,
            source_range: span.source_range,
            rect: PreviewRect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
            action: span.action.clone().map(Arc::new),
            tooltip: None,
            atomic: true,
            start_x: 0.0,
            end_x: width,
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
    selection_text: &mut SelectionText,
    error: MathError,
    text_layout_cache: &mut TextLayoutCache,
) -> InlinePiece {
    // The fallback keeps `math`: text shaping ignores it, and copying still knows this text is a
    // formula whose TeX rule 1 copies (plan 06 #preview-copy-semantics).
    let mut fallback = span.clone();
    fallback.style.math_placeholder = true;
    let mut piece = text_piece(
        font_system,
        fonts,
        &fallback,
        metrics,
        selection_text,
        text_layout_cache,
    );
    if let LayoutContent::Text(layout) = &mut piece.chunk.content {
        layout.mark_atomic_with_tooltip(Arc::from(error.to_string()));
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

#[cfg(test)]
mod tests {
    use crate::preview::copy::SelectionText;
    use cosmic_text::{Align, Metrics};

    use super::make_mixed_chunk;
    use crate::math::{MathEngine, MathKind};
    use crate::preview::layout::LayoutContent;
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
            soft_break: false,
        }
    }

    #[test]
    fn phase11_short_adjacent_text_around_math_uses_one_buffer_per_side() {
        let mut font_system = crate::system_fonts::new_font_system();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut selection_text = SelectionText::default();
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
            false,
        );

        assert_eq!(built.chunks.len(), 3);
        assert_eq!(selection_text.as_str(), "before plain$x^2$ after tail");
    }

    #[test]
    fn phase11_large_document_policy_coalesces_short_attributed_runs() {
        let mut font_system = crate::system_fonts::new_font_system();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut selection_text = SelectionText::default();
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
            false,
        );

        assert_eq!(built.chunks.len(), 2);
        assert_eq!(selection_text.as_str(), "before link$x$");
        assert!(built.chunks.iter().any(|chunk| {
            let LayoutContent::Text(layout) = &chunk.content else {
                return false;
            };
            crate::preview::text_layout::project_visible_text_boxes(
                layout,
                chunk.x,
                chunk.y,
                0.0,
                f32::MAX,
            )
            .iter()
            .any(|text_box| text_box.action.is_some())
        }));
    }
}
