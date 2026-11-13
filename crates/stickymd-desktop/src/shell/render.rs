//! The frame the shell presents: one fill, the panes, the caret, the scrollbars, Search and the
//! toolbar, then what the host makes of the present.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalSize;

use super::DesktopShell;
use super::presentation::PendingRedraw;
use crate::controls::{ToolbarVisual, paint_toolbar};
use crate::host::{Host, PresentFacts};
use crate::search::paint_search_overlay;
use stickymd_app::config::ContentZoomPercent;

#[derive(Debug, Clone, Copy, PartialEq)]
struct PresentationScales {
    document: f64,
    shell: f64,
}

impl PresentationScales {
    fn new(dpi: f64, content_zoom: ContentZoomPercent) -> Self {
        let document = dpi * f64::from(content_zoom.factor());
        // Content zoom belongs only to document projections. Native shell
        // geometry (toolbar, resize border, and hit testing) stays in window
        // DPI coordinates so painting and pointer input cannot drift apart.
        Self {
            document,
            shell: dpi,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourcePaintKey {
    generation: stickymd_core::Generation,
    selection: stickymd_core::Selection,
    preedit: Option<stickymd_render::source::PreeditVisual>,
    diagnostic: Option<String>,
    theme: stickymd_render::source::SourceTheme,
    width: u32,
    height: u32,
    scale_bits: u64,
    scroll_line: usize,
    scroll_vertical_bits: u32,
    scroll_horizontal_bits: u32,
}

impl<H: Host> DesktopShell<H> {
    pub(super) fn render(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // Hidden with its storage released: nothing to paint into, and nothing on screen.
        if self.frames_released {
            return;
        }
        self.draw_frame(Some(event_loop));
    }

    /// Paints and presents one frame; `true` when a full frame reached the window. Without an
    /// event loop (the frame drawn before a show), a host probe that asks to end is asked again on
    /// the next ordinary frame.
    pub(super) fn draw_frame(
        &mut self,
        event_loop: Option<&winit::event_loop::ActiveEventLoop>,
    ) -> bool {
        let draw_started = std::time::Instant::now();
        let pending = std::mem::replace(&mut self.pending_redraw, PendingRedraw::None);
        let Some(geometry) = self.view_geometry() else {
            return false;
        };
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor());
        let scales = PresentationScales::new(dpi, self.app.config().current().content_zoom_percent);
        let scale = scales.document;
        let source_theme = if self.resolved_dark_theme() {
            stickymd_render::source::SourceTheme::Dark
        } else {
            stickymd_render::source::SourceTheme::Light
        };
        if pending == PendingRedraw::CaretOnly && self.render_caret_damage(geometry, source_theme) {
            return true;
        }
        let preedit = self.app.editor.preedit_visual();
        if let (Some(_pane), Some(projection), Some(source_frame)) = (
            geometry.source,
            &mut self.projection,
            &mut self.source_frame,
        ) {
            let scroll = projection.scroll();
            let paint_key = SourcePaintKey {
                generation: projection.projected_generation(),
                selection: self.app.editor.selection,
                preedit,
                diagnostic: self.diagnostic.clone(),
                theme: source_theme,
                width: source_frame.width(),
                height: source_frame.height(),
                scale_bits: scale.to_bits(),
                scroll_line: scroll.line,
                scroll_vertical_bits: scroll.vertical.to_bits(),
                scroll_horizontal_bits: scroll.horizontal.to_bits(),
            };
            if self.source_paint_key.as_ref() != Some(&paint_key) {
                match projection.paint(
                    source_frame,
                    self.app.editor.selection,
                    self.app.editor.focused,
                    false,
                    self.diagnostic.as_deref(),
                    source_theme,
                ) {
                    Ok(()) => self.source_paint_key = Some(paint_key),
                    Err(error) => {
                        self.source_paint_key = None;
                        self.diagnostic = Some(error.to_string());
                    }
                }
            }
        }

        let dark = self.resolved_dark_theme();
        let toolbar_visual = ToolbarVisual {
            mode: self.app.config().current().view_mode,
            topmost: self.app.config().current().always_on_top,
            dark,
            system_theme: self.app.config().current().theme
                == stickymd_app::config::ThemeMode::System,
            diagnostic: self.diagnostic.is_some(),
            emphasized: self.app.editor.focused
                || self.pointer_inside_window
                || self.controls.opacity_popup_open,
            opacity_popup: self.controls.opacity_popup_open,
            opacity: self.controls.opacity_preview,
            opacity_field: self.controls.focused_field(),
            split_scroll_sync: self.app.config().current().split_scroll_sync,
        };
        let caret_animation_active = self.caret_animation_active();
        let search_layout = self.search_layout();
        let scrollbars = self.scrollbar_geometry();
        let Some(surface) = &mut self.surface else {
            return false;
        };
        surface.pixmap_mut().fill(tiny_skia::Color::from_rgba8(
            if dark { 31 } else { 248 },
            if dark { 31 } else { 246 },
            if dark { 29 } else { 239 },
            255,
        ));
        let window_bytes = surface.pixmap_mut().data().len() as u64;
        surface.ledger_mut().fill_bytes += window_bytes;
        if let (Some(pane), Some(source_frame)) = (geometry.source, &self.source_frame) {
            surface.ledger_mut().pane_blit_source_bytes += pixmap_bytes(source_frame);
            blit_pixmap(source_frame, surface.pixmap_mut(), pane.x, pane.y);
        }
        if self.native_caret_failed
            && caret_animation_active
            && self.app.editor.caret_visible
            && let (Some(pane), Some(projection)) = (geometry.source, &self.projection)
            && let Err(error) = projection.paint_caret_overlay(
                surface.pixmap_mut(),
                self.app.editor.selection.active.byte,
                pane.x as f32,
                pane.y as f32,
                source_theme,
            )
        {
            self.diagnostic = Some(error.to_string());
        }
        if let Some(pane) = geometry.preview {
            if let Some(frame) = &self.preview_frame
                && frame.width() == pane.width
                && frame.height() == pane.height
            {
                surface.ledger_mut().pane_blit_preview_bytes +=
                    u64::from(pane.width) * u64::from(pane.height) * 4;
                frame.blit_to(surface.pixmap_mut(), pane.x, pane.y);
            } else {
                paint_preview_pending(surface.pixmap_mut(), pane, dark);
            }
        }
        for bar in scrollbars.into_iter().flatten() {
            bar.paint(
                surface.pixmap_mut(),
                self.scrollbars.emphasized(bar.pane),
                dark,
            );
        }
        paint_search_overlay(
            surface.pixmap_mut(),
            &mut self.projection,
            &self.app.search,
            search_layout,
            scales.shell as f32,
            dark,
        );
        let surface_size = {
            let pixmap = surface.pixmap_mut();
            PhysicalSize::new(pixmap.width(), pixmap.height())
        };
        if let Some(layout) = &self.control_layout {
            paint_toolbar(surface.pixmap_mut(), geometry, layout, toolbar_visual);
        }
        let presented = if let Err(error) = surface.present() {
            self.diagnostic = Some(error.to_string());
            false
        } else {
            let facts = PresentFacts {
                width: surface_size.width,
                height: surface_size.height,
                view_complete: geometry.preview.is_none() || self.preview_frame.is_some(),
                ledger: surface.ledger_mut(),
                draw: draw_started.elapsed(),
            };
            if self.host.after_present(&facts) {
                match event_loop {
                    Some(event_loop) => self.finish_host_probe(|| event_loop.exit()),
                    None => self.request_redraw(),
                }
            }
            true
        };
        if presented {
            self.forget_native_caret();
            if !self.native_caret_failed
                && let Err(error) = self.sync_native_caret_overlay()
            {
                self.native_caret_failed = true;
                self.diagnostic = Some(format!("caret overlay unavailable: {error}"));
                self.request_redraw();
            }
        }
        self.update_ime_area();
        presented
    }
}

fn pixmap_bytes(pixmap: &Pixmap) -> u64 {
    u64::from(pixmap.width()) * u64::from(pixmap.height()) * 4
}

fn blit_pixmap(source: &Pixmap, target: &mut Pixmap, origin_x: u32, origin_y: u32) {
    let width = source.width().min(target.width().saturating_sub(origin_x));
    let height = source
        .height()
        .min(target.height().saturating_sub(origin_y));
    let source_stride = source.width() as usize * 4;
    let target_stride = target.width() as usize * 4;
    let row_bytes = width as usize * 4;
    for row in 0..height as usize {
        let source_start = row * source_stride;
        let target_start = (origin_y as usize + row) * target_stride + origin_x as usize * 4;
        target.data_mut()[target_start..target_start + row_bytes]
            .copy_from_slice(&source.data()[source_start..source_start + row_bytes]);
    }
}

fn paint_preview_pending(pixmap: &mut Pixmap, pane: crate::geometry::PaneRect, dark: bool) {
    fill_rect(
        pixmap,
        pane.x as f32,
        pane.y as f32,
        pane.width as f32,
        pane.height as f32,
        if dark {
            (31, 31, 29, 255)
        } else {
            (248, 246, 239, 255)
        },
    );
    for row in 0..3 {
        fill_rect(
            pixmap,
            pane.x as f32 + 24.0,
            pane.y as f32 + 28.0 + row as f32 * 18.0,
            (pane.width as f32 * (0.55 - row as f32 * 0.08)).max(16.0),
            3.0,
            if dark {
                (75, 74, 70, 255)
            } else {
                (220, 216, 205, 255)
            },
        );
    }
}

fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: (u8, u8, u8, u8),
) {
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, color.3);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalPosition;

    use super::*;
    use crate::controls::ControlId;
    use crate::controls::ControlLayout;
    use stickymd_app::config::ViewMode;

    #[test]
    fn phase10_content_zoom_keeps_toolbar_paint_and_hit_test_aligned() {
        let dpi = 1.5;
        let size = PhysicalSize::new(780, 1020);
        let hit_layout = ControlLayout::new(ViewMode::Split, size, dpi);
        let expected_rect = hit_layout.rect(ControlId::Split);
        let math_rect = hit_layout.rect(ControlId::ConvertMath);
        let hit_center = PhysicalPosition::new(
            expected_rect.x + expected_rect.width / 2.0,
            expected_rect.y + expected_rect.height / 2.0,
        );
        assert_eq!(hit_layout.control_at(hit_center), Some(ControlId::Split));

        for zoom in [50, 100, 300] {
            let scales = PresentationScales::new(dpi, ContentZoomPercent::new_clamped(zoom));
            assert_eq!(scales.shell, dpi, "content zoom {zoom}% moved the shell");
            assert_eq!(scales.document, dpi * f64::from(zoom) / 100.0);

            let paint_layout = ControlLayout::new(ViewMode::Split, size, scales.shell);
            assert_eq!(
                paint_layout.rect(ControlId::Split),
                expected_rect,
                "content zoom {zoom}% moved the painted Split control away from its hit target"
            );

            let mut pixmap = Pixmap::new(size.width, size.height).expect("test pixmap");
            let geometry = crate::geometry::geometry(ViewMode::Split, size, scales.shell as f32);
            paint_toolbar(
                &mut pixmap,
                geometry,
                &paint_layout,
                ToolbarVisual {
                    mode: ViewMode::Split,
                    topmost: false,
                    dark: false,
                    system_theme: false,
                    diagnostic: false,
                    emphasized: true,
                    opacity_popup: false,
                    opacity: 96,
                    opacity_field: None,
                    split_scroll_sync: true,
                },
            );
            // Below the resize border, which keeps the background out of its rows.
            let sample_x = expected_rect.x.floor() as u32 + 1;
            let sample_y = (expected_rect.y + expected_rect.height).floor() as u32 - 1;
            let painted = pixmap
                .pixel(sample_x, sample_y)
                .map(|color| (color.red(), color.green(), color.blue(), color.alpha()));
            assert_eq!(
                painted,
                Some((210, 215, 218, 255)),
                "content zoom {zoom}% painted the active Split control outside its hit rectangle"
            );

            let top = math_rect.y.floor() as u32;
            let bottom = (math_rect.y + math_rect.height).ceil() as u32;
            let left = math_rect.x.floor() as u32;
            let right = (math_rect.x + math_rect.width).ceil() as u32;
            let mut ink = (top..bottom)
                .flat_map(|y| (left..right).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    pixmap.pixel(*x, *y).is_some_and(|color| {
                        (color.red(), color.green(), color.blue()) == (64, 63, 59)
                    })
                });
            let first = ink.next().expect("dollar glyph must contain ink");
            let (min_x, max_x, min_y, max_y) = ink.fold(
                (first.0, first.0, first.1, first.1),
                |(min_x, max_x, min_y, max_y), (x, y)| {
                    (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
                },
            );
            assert!(
                f64::from(min_x) >= math_rect.x + math_rect.width * 0.25
                    && f64::from(max_x) <= math_rect.x + math_rect.width * 0.75,
                "content zoom {zoom}% did not keep the dollar glyph centered"
            );
            assert!(
                f64::from(max_y - min_y) >= math_rect.height * 0.45,
                "content zoom {zoom}% clipped the dollar glyph vertically"
            );
        }
    }
}
