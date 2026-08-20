//! Native painter for RaTeX path display items.
//!
//! Keeping path-command interpretation separate from glyph and primitive painting makes the
//! narrow RaTeX adapter auditable without creating a second math layout implementation.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use ratex_types::color::Color;
use ratex_types::path_command::PathCommand;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

const BASE_FONT_DIP: f32 = 17.0;

pub(super) fn paint_path(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    commands: &[PathCommand],
    fill: bool,
    color: &Color,
    em: f32,
) {
    if fill {
        let mut start = 0;
        for index in 1..commands.len() {
            if matches!(commands[index], PathCommand::MoveTo { .. }) {
                paint_path_segment(pixmap, x, y, &commands[start..index], true, color, em);
                start = index;
            }
        }
        paint_path_segment(pixmap, x, y, &commands[start..], true, color, em);
    } else {
        paint_path_segment(pixmap, x, y, commands, false, color, em);
    }
}

fn paint_path_segment(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    commands: &[PathCommand],
    fill: bool,
    color: &Color,
    em: f32,
) {
    let mut builder = PathBuilder::new();
    for command in commands {
        match command {
            PathCommand::MoveTo { x: px, y: py } => {
                builder.move_to(x + *px as f32 * em, y + *py as f32 * em)
            }
            PathCommand::LineTo { x: px, y: py } => {
                builder.line_to(x + *px as f32 * em, y + *py as f32 * em)
            }
            PathCommand::CubicTo {
                x1,
                y1,
                x2,
                y2,
                x: px,
                y: py,
            } => builder.cubic_to(
                x + *x1 as f32 * em,
                y + *y1 as f32 * em,
                x + *x2 as f32 * em,
                y + *y2 as f32 * em,
                x + *px as f32 * em,
                y + *py as f32 * em,
            ),
            PathCommand::QuadTo {
                x1,
                y1,
                x: px,
                y: py,
            } => builder.quad_to(
                x + *x1 as f32 * em,
                y + *y1 as f32 * em,
                x + *px as f32 * em,
                y + *py as f32 * em,
            ),
            PathCommand::Close => builder.close(),
        }
    }
    let Some(path) = builder.finish() else {
        return;
    };
    let mut paint = paint_for(color);
    if fill {
        paint.anti_alias = true;
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    } else {
        let stroke = Stroke {
            width: (1.5 * em / BASE_FONT_DIP).max(0.75),
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn paint_for(color: &Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        channel(color.r),
        channel(color.g),
        channel(color.b),
        channel(color.a),
    );
    paint
}

fn channel(value: f32) -> u8 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        0
    }
}
