//! Atomic inline/display formula placement inside native preview layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::sync::Arc;

use cosmic_text::{Align, FontSystem, Metrics, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use crate::math::{MAX_DOCUMENT_FORMULAS, MathEngine, MathError, MathRaster};
use crate::source::FontSelection;

use super::layout::{
    ChunkBuild, DecorationRole, LayoutChunk, LayoutContent, LayoutDecoration, make_text_chunk,
    math_foreground,
};
use super::{PreviewRect, PreviewTextBox, RenderSpan};

struct InlinePiece {
    chunk: LayoutChunk,
    boxes: Vec<PreviewTextBox>,
    decorations: Vec<LayoutDecoration>,
    width: f32,
    height: f32,
    baseline: f32,
}

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
) -> ChunkBuild {
    let mut pieces = Vec::new();
    for span in spans {
        if let Some(math) = &span.math {
            *formula_count = formula_count.saturating_add(1);
            let error =
                (*formula_count > MAX_DOCUMENT_FORMULAS).then_some(MathError::TooManyFormulas);
            let rendered = error.map_or_else(
                || {
                    math_engine.render(
                        &math.source,
                        math.kind,
                        metrics.font_size,
                        math_foreground(theme),
                    )
                },
                Err,
            );
            pieces.push(match rendered {
                Ok(raster) => formula_piece(span, raster, selection_text),
                Err(error) => error_piece(font_system, fonts, span, metrics, selection_text, error),
            });
            continue;
        }
        // Short text adjacent to a formula is one shaped run. Building a
        // cosmic-text buffer per word is measurably expensive and provides no
        // wrapping benefit while the run itself comfortably fits a line.
        if span.text == span.copy_text && span.text.chars().count() > 48 {
            for token in span
                .text
                .split_word_bounds()
                .filter(|token| !token.is_empty())
            {
                let token_span = RenderSpan {
                    text: token.to_owned(),
                    copy_text: token.to_owned(),
                    source_range: span.source_range,
                    style: span.style,
                    action: span.action.clone(),
                    math: None,
                };
                pieces.push(text_piece(
                    font_system,
                    fonts,
                    &token_span,
                    metrics,
                    selection_text,
                ));
            }
        } else {
            pieces.push(text_piece(
                font_system,
                fonts,
                span,
                metrics,
                selection_text,
            ));
        }
    }

    let mut output = ChunkBuild {
        chunks: Vec::new(),
        height: 0.0,
        boxes: Vec::new(),
        decorations: Vec::new(),
    };
    let mut line = Vec::new();
    let mut line_width = 0.0;
    let mut line_y = y;
    for piece in pieces {
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

fn text_piece(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    span: &RenderSpan,
    metrics: Metrics,
    selection_text: &mut String,
) -> InlinePiece {
    let mut built = make_text_chunk(
        font_system,
        fonts,
        std::slice::from_ref(span),
        0.0,
        0.0,
        1_000_000.0,
        metrics,
        Align::Left,
        Wrap::None,
        selection_text,
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
) -> InlinePiece {
    let mut fallback = span.clone();
    fallback.math = None;
    fallback.style.math_placeholder = true;
    let mut piece = text_piece(font_system, fonts, &fallback, metrics, selection_text);
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
