//! Window presentation and source-surface projection helpers.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::time::Instant;

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::{CARET_BLINK, StickyApp};
use crate::config::ViewMode;

impl StickyApp {
    pub(super) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn reset_caret_blink(&mut self) {
        self.session.caret_visible = true;
        self.next_blink = Instant::now() + CARET_BLINK;
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
        if self.config.view_mode != ViewMode::Preview
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
        if let Some(window) = &self.window {
            self.config.window.width_dip =
                (size.width as f64 / window.scale_factor()).round().max(1.0) as u32;
            self.config.window.height_dip = (size.height as f64 / window.scale_factor())
                .round()
                .max(1.0) as u32;
        }
        self.configure_viewports();
        self.request_preview_relayout();
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn render(&mut self) {
        let Some(geometry) = self.view_geometry() else {
            return;
        };
        if let (Some(_pane), Some(projection), Some(source_frame)) = (
            geometry.source,
            &mut self.projection,
            &mut self.source_frame,
        ) && let Err(error) = projection.paint(
            source_frame,
            self.session.selection,
            self.session.focused,
            self.session.caret_visible,
            self.diagnostic.as_deref(),
        ) {
            self.diagnostic = Some(error.to_string());
        }

        let Some(surface) = &mut self.surface else {
            return;
        };
        surface
            .pixmap_mut()
            .fill(tiny_skia::Color::from_rgba8(248, 246, 239, 255));
        if let (Some(pane), Some(source_frame)) = (geometry.source, &self.source_frame) {
            blit_pixmap(source_frame, surface.pixmap_mut(), pane.x, pane.y);
        }
        if let Some(pane) = geometry.preview {
            if let Some(frame) = &self.preview_frame
                && frame.width() == pane.width
                && frame.height() == pane.height
            {
                frame.blit_to(surface.pixmap_mut(), pane.x, pane.y);
            } else {
                paint_preview_pending(surface.pixmap_mut(), pane);
            }
        }
        paint_toolbar(
            surface.pixmap_mut(),
            geometry,
            self.config.view_mode,
            self.diagnostic.is_some(),
        );
        if let Err(error) = surface.present() {
            self.diagnostic = Some(error.to_string());
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

fn paint_toolbar(
    pixmap: &mut Pixmap,
    geometry: super::preview_runtime::ViewGeometry,
    mode: ViewMode,
    diagnostic: bool,
) {
    fill_rect(
        pixmap,
        0.0,
        0.0,
        pixmap.width() as f32,
        geometry.toolbar_height as f32,
        (238, 235, 226, 255),
    );
    let scale = (geometry.toolbar_height as f32 / 34.0).max(0.5);
    for (index, candidate) in [ViewMode::Source, ViewMode::Split, ViewMode::Preview]
        .into_iter()
        .enumerate()
    {
        let x = 7.0 * scale + index as f32 * 38.0 * scale;
        if mode == candidate {
            fill_rect(
                pixmap,
                x,
                4.0 * scale,
                32.0 * scale,
                26.0 * scale,
                (210, 215, 218, 255),
            );
        }
        paint_mode_icon(pixmap, candidate, x + 7.0 * scale, 9.0 * scale, scale);
    }
    if diagnostic {
        fill_rect(
            pixmap,
            (pixmap.width() as f32 - 10.0 * scale).max(0.0),
            7.0 * scale,
            4.0 * scale,
            20.0 * scale,
            (190, 116, 35, 255),
        );
    }
    if let Some(divider_x) = geometry.divider_x {
        fill_rect(
            pixmap,
            divider_x as f32,
            geometry.toolbar_height as f32,
            1.0f32.max(scale),
            (pixmap.height().saturating_sub(geometry.toolbar_height)) as f32,
            (190, 186, 175, 255),
        );
    }
}

fn paint_mode_icon(pixmap: &mut Pixmap, mode: ViewMode, x: f32, y: f32, scale: f32) {
    let ink = (64, 63, 59, 255);
    match mode {
        ViewMode::Source => {
            for row in 0..3 {
                fill_rect(
                    pixmap,
                    x,
                    y + row as f32 * 5.0 * scale,
                    (18.0 - row as f32 * 2.0) * scale,
                    1.5 * scale,
                    ink,
                );
            }
        }
        ViewMode::Split => {
            fill_rect(pixmap, x, y, 8.0 * scale, 13.0 * scale, ink);
            fill_rect(pixmap, x + 10.0 * scale, y, 8.0 * scale, 13.0 * scale, ink);
        }
        ViewMode::Preview => {
            fill_rect(pixmap, x, y, 18.0 * scale, 2.0 * scale, ink);
            fill_rect(
                pixmap,
                x + 3.0 * scale,
                y + 5.0 * scale,
                12.0 * scale,
                1.5 * scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 3.0 * scale,
                y + 10.0 * scale,
                10.0 * scale,
                1.5 * scale,
                ink,
            );
        }
    }
}

fn paint_preview_pending(pixmap: &mut Pixmap, pane: super::preview_runtime::PaneRect) {
    fill_rect(
        pixmap,
        pane.x as f32,
        pane.y as f32,
        pane.width as f32,
        pane.height as f32,
        (248, 246, 239, 255),
    );
    for row in 0..3 {
        fill_rect(
            pixmap,
            pane.x as f32 + 24.0,
            pane.y as f32 + 28.0 + row as f32 * 18.0,
            (pane.width as f32 * (0.55 - row as f32 * 0.08)).max(16.0),
            3.0,
            (220, 216, 205, 255),
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
