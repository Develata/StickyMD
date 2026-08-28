//! Window presentation and source-surface projection helpers.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::time::Instant;

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::controls::ControlLayout;
use super::search_runtime::{paint_search_overlay, search_field_display};
use super::toolbar_paint::{ToolbarVisual, paint_toolbar};
use super::{CARET_BLINK, StickyApp};
use crate::config::{ContentZoomPercent, ViewMode};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingRedraw {
    None,
    CaretOnly,
    Full,
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

impl StickyApp {
    pub(super) fn request_redraw(&mut self) {
        self.pending_redraw = PendingRedraw::Full;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn reset_caret_blink(&mut self) {
        self.session.caret_visible = true;
        self.next_blink = Instant::now() + CARET_BLINK;
    }

    /// Whether the source caret is both meaningful and visible enough to
    /// justify a periodic redraw. A collapsed edge sensor may retain native
    /// keyboard focus, but it is not an editable source surface.
    pub(super) fn caret_animation_active(&self) -> bool {
        self.session.focused
            && !self.preview_focused
            && !self.search.open
            && !self.session.is_composing()
            && self
                .window_flow
                .as_ref()
                .is_some_and(|flow| flow.state().accepts_editor_mutation())
    }

    pub(super) fn sync_preedit(&mut self) {
        if let Some(projection) = &mut self.projection {
            projection.set_preedit(self.session.preedit_visual());
        }
    }

    pub(super) fn update_ime_area(&mut self) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if self.search.open
            && let Some(layout) = self.search_layout()
        {
            let scale = window.scale_factor() as f32;
            let focused = self.search.focused;
            let (display, cursor) = search_field_display(&self.search, focused);
            let spec = layout.field_spec(focused, scale);
            if let Some(caret) = self
                .projection
                .as_mut()
                .and_then(|projection| projection.ui_text_field_caret(&display, cursor, spec))
            {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(caret.x.round() as i32, caret.y.round() as i32),
                    PhysicalSize::new(
                        caret.width.max(1.0).round() as u32,
                        caret.height.max(1.0).round() as u32,
                    ),
                );
            }
            return;
        }
        let origin = self
            .view_geometry()
            .and_then(|geometry| geometry.source)
            .map_or((0, 0), |pane| (pane.x, pane.y));
        let Some(projection) = &mut self.projection else {
            return;
        };
        if let Some(caret) = projection.ime_caret_rect(self.session.selection.active.byte) {
            window.set_ime_cursor_area(
                PhysicalPosition::new(
                    origin.0 as i32 + caret.x.round() as i32,
                    origin.1 as i32 + caret.y.round() as i32,
                ),
                PhysicalSize::new(
                    caret.width.max(1.0).round() as u32,
                    caret.height.max(1.0).round() as u32,
                ),
            );
        }
    }

    pub(super) fn after_presentation_change(&mut self) {
        if self.config.current().view_mode != ViewMode::Preview
            && let Some(projection) = &mut self.projection
        {
            let _ = projection.ensure_caret_visible(self.session.selection.active.byte);
            let scroll = projection.scroll();
            self.session.scroll.line = scroll.line;
            self.session.scroll.vertical_px = scroll.vertical;
            self.session.scroll.horizontal_px = scroll.horizontal;
        }
        self.sync_preedit();
        self.reset_caret_blink();
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        if let Some(surface) = &mut self.surface
            && let Err(error) = surface.resize(size.width, size.height)
        {
            self.diagnostic = Some(error.to_string());
        }
        // Resize events are projection facts. Phase 8 commits durable geometry
        // once at the stable user move/resize boundary, never once per frame.
        self.configure_viewports();
        self.request_preview_relayout();
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn render(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let pending = std::mem::replace(&mut self.pending_redraw, PendingRedraw::None);
        let Some(geometry) = self.view_geometry() else {
            return;
        };
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor());
        let scales = PresentationScales::new(dpi, self.config.current().content_zoom_percent);
        let scale = scales.document;
        let source_theme = if self.resolved_dark_theme() {
            stickymd_render::source::SourceTheme::Dark
        } else {
            stickymd_render::source::SourceTheme::Light
        };
        if pending == PendingRedraw::CaretOnly && self.render_caret_damage(geometry, source_theme) {
            return;
        }
        let preedit = self.session.preedit_visual();
        if let (Some(_pane), Some(projection), Some(source_frame)) = (
            geometry.source,
            &mut self.projection,
            &mut self.source_frame,
        ) {
            let scroll = projection.scroll();
            let paint_key = SourcePaintKey {
                generation: projection.projected_generation(),
                selection: self.session.selection,
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
                    self.session.selection,
                    self.session.focused,
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
            mode: self.config.current().view_mode,
            topmost: self.config.current().always_on_top,
            dark,
            system_theme: self.config.current().theme == crate::config::ThemeMode::System,
            diagnostic: self.diagnostic.is_some(),
            emphasized: self.session.focused
                || self.pointer_inside_window
                || self.controls.opacity_popup_open,
            opacity_popup: self.controls.opacity_popup_open,
            opacity: self.controls.opacity_preview,
            split_scroll_sync: self.config.current().split_scroll_sync,
        };
        let caret_animation_active = self.caret_animation_active();
        let search_layout = self.search_layout();
        let Some(surface) = &mut self.surface else {
            return;
        };
        surface.pixmap_mut().fill(tiny_skia::Color::from_rgba8(
            if dark { 31 } else { 248 },
            if dark { 31 } else { 246 },
            if dark { 29 } else { 239 },
            255,
        ));
        if let (Some(pane), Some(source_frame)) = (geometry.source, &self.source_frame) {
            blit_pixmap(source_frame, surface.pixmap_mut(), pane.x, pane.y);
        }
        if self.native_caret_failed
            && caret_animation_active
            && self.session.caret_visible
            && let (Some(pane), Some(projection)) = (geometry.source, &self.projection)
            && let Err(error) = projection.paint_caret_overlay(
                surface.pixmap_mut(),
                self.session.selection.active.byte,
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
                frame.blit_to(surface.pixmap_mut(), pane.x, pane.y);
            } else {
                paint_preview_pending(surface.pixmap_mut(), pane, dark);
            }
        }
        paint_search_overlay(
            surface.pixmap_mut(),
            &mut self.projection,
            &self.search,
            search_layout,
            scales.shell as f32,
            dark,
        );
        let surface_size = {
            let pixmap = surface.pixmap_mut();
            PhysicalSize::new(pixmap.width(), pixmap.height())
        };
        let layout = ControlLayout::new(surface_size, scales.shell);
        paint_toolbar(surface.pixmap_mut(), geometry, &layout, toolbar_visual);
        let presented = if let Err(error) = surface.present() {
            self.diagnostic = Some(error.to_string());
            false
        } else {
            match self.startup_diagnostics.editor_ready() {
                Ok(true) => event_loop.exit(),
                Ok(false) => {}
                Err(error) => eprintln!("startup diagnostics failed: {error}"),
            }
            true
        };
        if presented {
            self.native_caret_overlay = None;
            if !self.native_caret_failed
                && let Err(error) = self.sync_native_caret_overlay()
            {
                self.native_caret_failed = true;
                self.diagnostic = Some(format!("caret overlay unavailable: {error}"));
                self.request_redraw();
            }
        }
        self.update_ime_area();
    }
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

fn paint_preview_pending(pixmap: &mut Pixmap, pane: super::preview_runtime::PaneRect, dark: bool) {
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
    use super::*;

    #[test]
    fn phase10_content_zoom_keeps_toolbar_paint_and_hit_test_aligned() {
        let dpi = 1.5;
        let size = PhysicalSize::new(780, 1020);
        let hit_layout = ControlLayout::new(size, dpi);
        let expected_rect = hit_layout.rect(super::super::controls::ControlId::Split);
        let math_rect = hit_layout.rect(super::super::controls::ControlId::ConvertMath);
        let hit_center = PhysicalPosition::new(
            expected_rect.x + expected_rect.width / 2.0,
            expected_rect.y + expected_rect.height / 2.0,
        );
        assert_eq!(
            hit_layout.control_at(hit_center),
            Some(super::super::controls::ControlId::Split)
        );

        for zoom in [50, 100, 300] {
            let scales = PresentationScales::new(dpi, ContentZoomPercent::new_clamped(zoom));
            assert_eq!(scales.shell, dpi, "content zoom {zoom}% moved the shell");
            assert_eq!(scales.document, dpi * f64::from(zoom) / 100.0);

            let paint_layout = ControlLayout::new(size, scales.shell);
            assert_eq!(
                paint_layout.rect(super::super::controls::ControlId::Split),
                expected_rect,
                "content zoom {zoom}% moved the painted Split control away from its hit target"
            );

            let mut pixmap = Pixmap::new(size.width, size.height).expect("test pixmap");
            let geometry =
                super::super::preview_runtime::geometry(ViewMode::Split, size, scales.shell as f32);
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
                    split_scroll_sync: true,
                },
            );
            let sample_x = expected_rect.x.floor() as u32 + 1;
            let sample_y = expected_rect.y.floor() as u32 + 1;
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
