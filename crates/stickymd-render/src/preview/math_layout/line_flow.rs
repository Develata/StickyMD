//! Line filling for mixed inline/display formula layout.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#display-math-in-paragraph

use cosmic_text::{Align, Metrics};

use crate::preview::PreviewRect;
use crate::preview::layout::{
    BODY_SIZE_DIP, ChunkBuild, DecorationRole, InlinePiece, LayoutDecoration,
};

/// Greedy filling of laid-out pieces into lines. A piece marked own-line (a display formula) is
/// centered alone on its line; every other piece fills lines left to right in the block's
/// alignment. `embedded` formulas also get a background and a gap to their neighbouring lines;
/// a standalone display block has its own background and the block gap.
pub(super) struct LineFlow {
    x: f32,
    top: f32,
    width: f32,
    metrics: Metrics,
    align: Align,
    embedded: bool,
    scale: f32,
    line: Vec<InlinePiece>,
    line_width: f32,
    line_y: f32,
    /// The last placed piece was an own-line formula: the next one is separated by the gap.
    after_formula: bool,
    /// The formula's line is already over, so one hard break next to it is that line break and
    /// must not add a blank line. Spent by the first break, or by any further piece.
    break_free: bool,
    output: ChunkBuild,
}

impl LineFlow {
    pub(super) fn new(
        x: f32,
        y: f32,
        width: f32,
        metrics: Metrics,
        align: Align,
        embedded: bool,
    ) -> Self {
        Self {
            x,
            top: y,
            width,
            metrics,
            align,
            embedded,
            scale: metrics.font_size / BODY_SIZE_DIP,
            line: Vec::new(),
            line_width: 0.0,
            line_y: y,
            after_formula: false,
            break_free: false,
            output: ChunkBuild {
                chunks: Vec::new(),
                height: 0.0,
                boxes: Vec::new(),
                decorations: Vec::new(),
            },
        }
    }

    fn gap(&self) -> f32 {
        if self.embedded { 6.0 * self.scale } else { 0.0 }
    }

    /// Ends the current line and returns its height.
    fn flush(&mut self, align: Align) -> f32 {
        let height = flush_line(
            &mut self.line,
            self.line_width,
            self.x,
            self.line_y,
            self.width,
            self.metrics,
            align,
            &mut self.output,
        );
        self.line_width = 0.0;
        self.line_y += height;
        height
    }

    /// A hard break: ends the line, or leaves an empty line when there is none to end.
    pub(super) fn hard_break(&mut self) {
        if std::mem::take(&mut self.break_free) {
            return;
        }
        if self.line.is_empty() {
            self.line_y += self.metrics.line_height;
        } else {
            self.flush(self.align);
        }
    }

    pub(super) fn place_inline(&mut self, piece: InlinePiece) {
        self.start_piece();
        self.after_formula = false;
        if !self.line.is_empty() && self.line_width + piece.width > self.width {
            self.flush(self.align);
        }
        self.line_width += piece.width;
        self.line.push(piece);
    }

    pub(super) fn place_own_line(&mut self, piece: InlinePiece) {
        let after_formula = self.after_formula;
        self.start_piece();
        let gap = self.gap();
        if !self.line.is_empty() {
            self.flush(self.align);
            self.line_y += gap;
        } else if self.line_y > self.top && !after_formula {
            self.line_y += gap;
        }
        let (top, piece_width) = (self.line_y, piece.width);
        self.line.push(piece);
        self.line_width = piece_width;
        let height = self.flush(Align::Center);
        if self.embedded {
            let scale = self.scale;
            self.output.decorations.push(LayoutDecoration {
                rect: PreviewRect {
                    x: self.x - 7.0 * scale,
                    y: top - 3.0 * scale,
                    width: self.width + 14.0 * scale,
                    height: height + 6.0 * scale,
                },
                role: DecorationRole::EmbeddedMathBackground,
            });
        }
        self.after_formula = true;
        self.break_free = true;
    }

    fn start_piece(&mut self) {
        if self.after_formula {
            self.line_y += self.gap();
        }
        self.break_free = false;
    }

    pub(super) fn finish(mut self) -> ChunkBuild {
        if !self.line.is_empty() {
            self.flush(self.align);
        }
        self.output.height = (self.line_y - self.top).max(self.metrics.line_height);
        self.output
    }
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
            text_box.start_x += cursor_x;
            text_box.end_x += cursor_x;
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
