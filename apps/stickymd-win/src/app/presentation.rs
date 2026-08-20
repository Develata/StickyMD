//! Window presentation and source-surface projection helpers.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::time::Instant;

use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::{CARET_BLINK, StickyApp};

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
        let Some(projection) = &mut self.projection else {
            return;
        };
        if let Some(caret) = projection.ime_caret_rect(self.session.selection.active.byte) {
            window.set_ime_cursor_area(
                PhysicalPosition::new(caret.x.round() as i32, caret.y.round() as i32),
                PhysicalSize::new(
                    caret.width.max(1.0).round() as u32,
                    caret.height.max(1.0).round() as u32,
                ),
            );
        }
    }

    pub(super) fn after_presentation_change(&mut self) {
        if let Some(projection) = &mut self.projection {
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
        if let (Some(window), Some(projection)) = (&self.window, &mut self.projection) {
            projection.set_viewport(size.width, size.height, window.scale_factor() as f32);
            self.config.window.width_dip =
                (size.width as f64 / window.scale_factor()).round().max(1.0) as u32;
            self.config.window.height_dip = (size.height as f64 / window.scale_factor())
                .round()
                .max(1.0) as u32;
        }
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn render(&mut self) {
        let (Some(surface), Some(projection)) = (&mut self.surface, &mut self.projection) else {
            return;
        };
        if let Err(error) = projection.paint(
            surface.pixmap_mut(),
            self.session.selection,
            self.session.focused,
            self.session.caret_visible,
            self.diagnostic.as_deref(),
        ) {
            self.diagnostic = Some(error.to_string());
            return;
        }
        if let Err(error) = surface.present() {
            self.diagnostic = Some(error.to_string());
        }
        self.update_ime_area();
    }
}
