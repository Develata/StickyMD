//! Paint already-shaped viewport rows using cosmic-text's glyph and decoration renderer.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::{Color, FontSystem, LayoutRun, LegacyRenderer, Renderer, SwashCache};

use super::TextLayout;

impl TextLayout {
    #[cfg(test)]
    pub(in crate::preview) fn draw_visible(
        &self,
        font_system: &mut FontSystem,
        cache: &mut SwashCache,
        color: Color,
        viewport: std::ops::Range<f32>,
        callback: impl FnMut(i32, i32, u32, u32, Color),
    ) {
        self.draw_in_rect(
            font_system,
            cache,
            color,
            crate::preview::PreviewRect {
                x: -f32::MAX / 2.0,
                y: viewport.start,
                width: f32::MAX,
                height: viewport.end - viewport.start,
            },
            callback,
        );
    }

    pub(in crate::preview) fn draw_in_rect(
        &self,
        font_system: &mut FontSystem,
        cache: &mut SwashCache,
        color: Color,
        viewport: crate::preview::PreviewRect,
        callback: impl FnMut(i32, i32, u32, u32, Color),
    ) {
        let horizontal = viewport.x..viewport.right();
        let viewport = viewport.y..viewport.bottom();
        let mut font_bounds = std::collections::HashMap::new();
        // Account for shaping offsets before retaining one adjacent row for
        // glyph ink crossing a line box. Combining marks can span many rows.
        // The locator visits vertically visible rows only. Unwrapped rows still
        // scan their glyphs, but off-screen glyph pixels are not painted.
        let viewport = (viewport.start - self.shaped.max_glyph_y_offset)
            ..(viewport.end + self.shaped.max_glyph_y_offset);
        let first = self
            .shaped
            .rows
            .partition_point(|row| row.bottom() < viewport.start)
            .saturating_sub(1);
        let last = (self
            .shaped
            .rows
            .partition_point(|row| row.top <= viewport.end)
            + 1)
        .min(self.shaped.rows.len());
        let mut renderer = LegacyRenderer {
            font_system,
            cache,
            callback,
        };
        for row in &self.shaped.rows[first..last] {
            let line = &self.shaped.buffer.lines[row.logical_line];
            let layout =
                &line.layout_opt().expect("TextLayout contains shaped rows")[row.layout_row];
            let run = LayoutRun {
                line_i: row.logical_line,
                text: line.text(),
                rtl: line
                    .shape_opt()
                    .expect("TextLayout contains shaped lines")
                    .rtl,
                glyphs: &layout.glyphs,
                decorations: &layout.decorations,
                line_y: row.baseline,
                line_top: row.top,
                line_height: row.height,
                line_w: layout.w,
            };
            for glyph in run.glyphs {
                // Most glyphs are inside the clip and need no extra lookup. For
                // long unwrapped lines use conservative font ink bounds; color,
                // bitmap and variable fonts fall back to actual raster placement.
                if !glyph_intersects(glyph, &horizontal, &mut renderer, &mut font_bounds) {
                    continue;
                }
                renderer.glyph(
                    glyph.physical((0.0, run.line_y), 1.0),
                    glyph.color_opt.unwrap_or(color),
                );
            }
            cosmic_text::render_decoration(&mut renderer, &run, color);
        }
    }
}

fn glyph_intersects<F: FnMut(i32, i32, u32, u32, Color)>(
    glyph: &cosmic_text::LayoutGlyph,
    horizontal: &std::ops::Range<f32>,
    renderer: &mut LegacyRenderer<'_, F>,
    bounds: &mut std::collections::HashMap<
        (cosmic_text::fontdb::ID, cosmic_text::fontdb::Weight),
        Option<(f32, f32)>,
    >,
) -> bool {
    let x = glyph.x + glyph.font_size * glyph.x_offset;
    if x >= horizontal.start && x <= horizontal.end {
        return true;
    }
    let bound = bounds
        .entry((glyph.font_id, glyph.font_weight))
        .or_insert_with(|| {
            let font = renderer
                .font_system
                .get_font(glyph.font_id, glyph.font_weight)?;
            let swash = font.as_swash();
            if swash.variations().next().is_some()
                || swash.color_palettes().next().is_some()
                || swash.color_strikes().next().is_some()
                || swash.alpha_strikes().next().is_some()
            {
                return None;
            }
            let metrics = font.metrics();
            let bbox = metrics.bounds?;
            if metrics.units_per_em == 0 {
                return None;
            }
            let em = f32::from(metrics.units_per_em);
            // Cover synthetic italic shear as well as normal outline extents.
            let shear = bbox.y_min.abs().max(bbox.y_max.abs()) * 0.25;
            Some(((bbox.x_min - shear) / em, (bbox.x_max + shear) / em))
        });
    if let Some((left, right)) = *bound
        && (x + right * glyph.font_size + 2.0 < horizontal.start
            || x + left * glyph.font_size - 2.0 > horizontal.end)
    {
        return false;
    }
    let physical = glyph.physical((0.0, 0.0), 1.0);
    renderer
        .cache
        .get_image(renderer.font_system, physical.cache_key)
        .as_ref()
        .is_some_and(|image| {
            let left = (physical.x + image.placement.left) as f32;
            left + image.placement.width as f32 >= horizontal.start && left <= horizontal.end
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, UnderlineStyle, Wrap};

    #[test]
    fn horizontal_clip_matches_full_draw_for_bidi_emoji_italic_and_combining_ink() {
        let mut fonts = FontSystem::new();
        let mut cache = SwashCache::new();
        for scale in [0.75, 1.25, 3.0] {
            let mut buffer = Buffer::new_empty(Metrics::new(17.0 * scale, 26.0 * scale));
            buffer.set_wrap(Wrap::None);
            let text = format!(
                "{} a{} 中🙂👩‍💻 עברית العربية italic {}",
                "mixed ".repeat(15),
                "\u{301}".repeat(60),
                "end ".repeat(20)
            );
            buffer.set_text(
                &text,
                &Attrs::new()
                    .family(Family::Monospace)
                    .style(cosmic_text::Style::Italic)
                    .underline(UnderlineStyle::Single),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut fonts, false);
            let mut layout = TextLayout::new(buffer, Vec::new());
            for left in [0.0_f32, 137.25, 310.5, 580.75, 900.0] {
                let mut render = |clipped: bool| {
                    let mut pixels = vec![0u8; 184 * 160 * 4];
                    let origin = (-left).round() as i32;
                    let mut callback = |x: i32, y: i32, w: u32, h: u32, color: Color| {
                        for py in y.max(0)..(y + h as i32).min(160) {
                            for px in (x + origin).max(0)..(x + origin + w as i32).min(184) {
                                let offset = (py as usize * 184 + px as usize) * 4;
                                pixels[offset..offset + 4].copy_from_slice(&color.as_rgba());
                            }
                        }
                    };
                    let color = Color::rgb(31, 35, 42);
                    if clipped {
                        layout.draw_in_rect(
                            &mut fonts,
                            &mut cache,
                            color,
                            crate::preview::PreviewRect {
                                x: -origin as f32,
                                y: 0.0,
                                width: 184.0,
                                height: 160.0,
                            },
                            &mut callback,
                        );
                    } else {
                        std::sync::Arc::get_mut(&mut layout.shaped)
                            .unwrap()
                            .buffer
                            .draw(&mut fonts, &mut cache, color, &mut callback);
                    }
                    pixels
                };
                let full = render(false);
                assert_eq!(render(true), full, "scale={scale} left={left}");
            }
        }
    }

    #[test]
    fn viewport_pixels_include_combining_marks_from_distant_rows() {
        let mut fonts = FontSystem::new();
        let mut cache = SwashCache::new();
        let mut buffer = Buffer::new(&mut fonts, Metrics::new(17.0, 26.0));
        buffer.set_size(Some(260.0), None);
        let text = format!(
            "{}a{}{}\n{}",
            "plain\n".repeat(12),
            "\u{301}".repeat(80),
            "\u{316}".repeat(80),
            "plain\n".repeat(12),
        );
        buffer.set_text(
            &text,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut fonts, false);
        let mut layout = TextLayout::new(buffer, Vec::new());
        let mut render = |visible: bool, scroll: i32| {
            let mut pixels = vec![0u8; 260 * 104 * 4];
            let mut callback = |x: i32, y: i32, w: u32, h: u32, color: Color| {
                for py in (y - scroll).max(0)..(y - scroll + h as i32).min(104) {
                    for px in x.max(0)..(x + w as i32).min(260) {
                        let offset = (py as usize * 260 + px as usize) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&color.as_rgba());
                    }
                }
            };
            let color = Color::rgb(31, 35, 42);
            if visible {
                layout.draw_visible(
                    &mut fonts,
                    &mut cache,
                    color,
                    scroll as f32..(scroll + 104) as f32,
                    &mut callback,
                );
            } else {
                std::sync::Arc::get_mut(&mut layout.shaped)
                    .unwrap()
                    .buffer
                    .draw(&mut fonts, &mut cache, color, &mut callback);
            }
            pixels
        };
        for scroll in [104, 416] {
            let full = render(false, scroll);
            let visible = render(true, scroll);
            assert_eq!(
                visible
                    .iter()
                    .zip(&full)
                    .filter(|(left, right)| left != right)
                    .count(),
                0,
                "viewport culling omitted ink from stacked combining marks at {scroll}",
            );
        }
    }

    #[test]
    fn viewport_pixels_match_full_buffer_draw_at_wrap_and_fractional_boundaries() {
        let mut fonts = FontSystem::new();
        let mut cache = SwashCache::new();
        for scale in [0.75, 1.0, 1.25, 2.0] {
            let width = (260.0 * scale) as u32;
            let height = 173;
            let mut buffer = Buffer::new(&mut fonts, Metrics::new(17.0 * scale, 26.0 * scale));
            buffer.set_size(Some(width as f32), None);
            buffer.set_wrap(Wrap::WordOrGlyph);
            let base = Attrs::new().family(Family::SansSerif);
            let plain = "中文 mixed 👩‍💻 e\u{301} عَرَبِيّ עברית\n".repeat(12);
            let decorated = "Link underline and strike 中文 עברית\n".repeat(12);
            buffer.set_rich_text(
                [
                    (plain.as_str(), base.clone()),
                    (
                        decorated.as_str(),
                        base.clone()
                            .underline(UnderlineStyle::Single)
                            .strikethrough(),
                    ),
                    (plain.as_str(), base.clone()),
                ],
                &base,
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut fonts, false);
            let mut layout = TextLayout::new(buffer, Vec::new());
            for scroll in [0.0, 25.5 * scale, 312.75 * scale, 805.5 * scale] {
                let render = |visible: bool,
                              layout: &mut TextLayout,
                              fonts: &mut FontSystem,
                              cache: &mut SwashCache| {
                    let mut pixels = vec![0u8; (width * height * 4) as usize];
                    let origin_y = (-scroll).round() as i32;
                    let mut callback = |x: i32, y: i32, w: u32, h: u32, color: Color| {
                        for py in
                            (y + origin_y).max(0)..(y + origin_y + h as i32).min(height as i32)
                        {
                            for px in x.max(0)..(x + w as i32).min(width as i32) {
                                let offset = (py as usize * width as usize + px as usize) * 4;
                                pixels[offset..offset + 4].copy_from_slice(&color.as_rgba());
                            }
                        }
                    };
                    let color = Color::rgb(31, 35, 42);
                    if visible {
                        layout.draw_visible(
                            fonts,
                            cache,
                            color,
                            scroll..scroll + height as f32,
                            &mut callback,
                        );
                    } else {
                        std::sync::Arc::get_mut(&mut layout.shaped)
                            .unwrap()
                            .buffer
                            .draw(fonts, cache, color, &mut callback);
                    }
                    pixels
                };
                let full = render(false, &mut layout, &mut fonts, &mut cache);
                let visible = render(true, &mut layout, &mut fonts, &mut cache);
                assert!(full.iter().any(|byte| *byte != 0));
                assert_eq!(visible, full, "scale={scale}, scroll={scroll}");
            }
        }
    }
}
