//! Caret-only presentation scheduling and bounded damage updates.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use tiny_skia::Pixmap;

use super::StickyApp;
use super::presentation::PendingRedraw;
use super::preview_runtime::{PaneRect, ViewGeometry};
use crate::surface::DamageRect;

impl StickyApp {
    pub(super) fn request_caret_redraw(&mut self) {
        if self.pending_redraw == PendingRedraw::None {
            self.pending_redraw = PendingRedraw::CaretOnly;
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    pub(super) fn advance_caret_blink(&mut self) {
        self.session.caret_visible = !self.session.caret_visible;
        if self.native_caret_failed {
            self.request_caret_redraw();
            return;
        }
        if let Err(error) = self.sync_native_caret_overlay() {
            self.native_caret_failed = true;
            self.native_caret_drawn = false;
            self.diagnostic = Some(format!("caret overlay unavailable: {error}"));
            self.request_caret_redraw();
        }
    }

    pub(super) fn render_caret_damage(
        &mut self,
        geometry: ViewGeometry,
        source_theme: stickymd_render::source::SourceTheme,
    ) -> bool {
        let Some(pane) = geometry.source else {
            return false;
        };
        let Some(caret) = self
            .projection
            .as_ref()
            .and_then(|projection| projection.caret_rect(self.session.selection.active.byte))
        else {
            return false;
        };
        let Some(damage) = caret_damage_rect(pane, caret) else {
            return false;
        };
        let (Some(source_frame), Some(surface)) = (&self.source_frame, &mut self.surface) else {
            return false;
        };
        blit_damage(source_frame, surface.pixmap_mut(), pane, damage);
        if self.session.caret_visible
            && let Some(projection) = &self.projection
            && let Err(error) = projection.paint_caret_overlay(
                surface.pixmap_mut(),
                self.session.selection.active.byte,
                pane.x as f32,
                pane.y as f32,
                source_theme,
            )
        {
            self.diagnostic = Some(error.to_string());
            return false;
        }
        if let Err(error) = surface.present_damage(damage) {
            self.diagnostic = Some(error.to_string());
            return false;
        }
        self.native_caret_drawn = false;
        true
    }

    pub(super) fn sync_native_caret_overlay(&mut self) -> Result<(), String> {
        let desired = self.caret_animation_active() && self.session.caret_visible;
        if desired == self.native_caret_drawn {
            return Ok(());
        }
        let pane = self
            .view_geometry()
            .and_then(|geometry| geometry.source)
            .ok_or_else(|| "source pane is unavailable".to_owned())?;
        let caret = self
            .projection
            .as_ref()
            .and_then(|projection| projection.caret_rect(self.session.selection.active.byte))
            .ok_or_else(|| "caret rectangle is unavailable".to_owned())?;
        let damage = caret_damage_rect(pane, caret)
            .ok_or_else(|| "caret rectangle is outside the source pane".to_owned())?;
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| "window is unavailable".to_owned())?;
        crate::platform::windows::caret_overlay::toggle_caret_overlay(
            window.as_ref(),
            crate::platform::windows::caret_overlay::CaretOverlayRect {
                x: i32::try_from(damage.x)
                    .map_err(|_| "caret x coordinate exceeds Win32 range".to_owned())?,
                y: i32::try_from(damage.y)
                    .map_err(|_| "caret y coordinate exceeds Win32 range".to_owned())?,
                width: i32::try_from(damage.width)
                    .map_err(|_| "caret width exceeds Win32 range".to_owned())?,
                height: i32::try_from(damage.height)
                    .map_err(|_| "caret height exceeds Win32 range".to_owned())?,
            },
        )
        .map_err(|error| error.to_string())?;
        self.native_caret_drawn = desired;
        Ok(())
    }
}

fn caret_damage_rect(
    pane: PaneRect,
    caret: stickymd_render::source::EditorRect,
) -> Option<DamageRect> {
    let pane_right = pane.x.saturating_add(pane.width);
    let pane_bottom = pane.y.saturating_add(pane.height);
    let left = (pane.x as f32 + caret.x).floor().max(pane.x as f32) as u32;
    let top = (pane.y as f32 + caret.y).floor().max(pane.y as f32) as u32;
    let right = (pane.x as f32 + caret.x + caret.width.max(1.0))
        .ceil()
        .min(pane_right as f32) as u32;
    let bottom = (pane.y as f32 + caret.y + caret.height)
        .ceil()
        .min(pane_bottom as f32) as u32;
    let width = right.saturating_sub(left);
    let height = bottom.saturating_sub(top);
    (width != 0 && height != 0).then_some(DamageRect::new(left, top, width, height))
}

fn blit_damage(source: &Pixmap, target: &mut Pixmap, pane: PaneRect, damage: DamageRect) {
    let source_stride = source.width() as usize * 4;
    let target_stride = target.width() as usize * 4;
    let source_x = damage.x.saturating_sub(pane.x) as usize;
    let source_y = damage.y.saturating_sub(pane.y) as usize;
    let target_x = damage.x as usize;
    let target_y = damage.y as usize;
    let row_bytes = damage.width as usize * 4;
    for row in 0..damage.height as usize {
        let source_start = (source_y + row) * source_stride + source_x * 4;
        let target_start = (target_y + row) * target_stride + target_x * 4;
        target.data_mut()[target_start..target_start + row_bytes]
            .copy_from_slice(&source.data()[source_start..source_start + row_bytes]);
    }
}

#[cfg(test)]
mod tests {
    use super::caret_damage_rect;
    use crate::app::preview_runtime::PaneRect;
    use crate::surface::DamageRect;
    use stickymd_render::source::EditorRect;

    #[test]
    fn phase9_caret_damage_is_clipped_to_the_source_pane() {
        let pane = PaneRect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };
        let damage = caret_damage_rect(
            pane,
            EditorRect {
                x: 98.5,
                y: 45.0,
                width: 4.0,
                height: 10.0,
            },
        )
        .expect("partially visible caret");
        assert_eq!(damage, DamageRect::new(108, 65, 2, 5));
    }
}
