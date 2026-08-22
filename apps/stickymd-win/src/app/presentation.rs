//! Window presentation and source-surface projection helpers.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::time::Instant;

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::controls::ControlLayout;
use super::toolbar_paint::{ToolbarVisual, paint_toolbar};
use super::{CARET_BLINK, StickyApp};
use crate::config::ViewMode;

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
        let scale = f64::from(self.document_scale_factor());
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
        };
        let caret_animation_active = self.caret_animation_active();
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
        let surface_size = {
            let pixmap = surface.pixmap_mut();
            PhysicalSize::new(pixmap.width(), pixmap.height())
        };
        let layout = ControlLayout::new(surface_size, scale);
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
            self.native_caret_drawn = false;
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
