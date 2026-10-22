//! Transient Preview block scrolling, independent of document and vertical scroll state.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#preview-view-session
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-horizontal-scroll

use stickymd_render::preview::{PreviewHorizontalOffsets, PreviewHorizontalTarget};
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

use super::StickyApp;

#[derive(Default)]
pub(super) struct HorizontalSession {
    pub offsets: PreviewHorizontalOffsets,
    pub drag: Option<HorizontalDrag>,
}

pub(super) struct HorizontalDrag {
    target: PreviewHorizontalTarget,
    start_x: f64,
}

impl StickyApp {
    pub(super) fn preview_hit_geometry_is_current(&self) -> bool {
        self.preview_layout_is_current()
            && self
                .preview_frame_viewport
                .as_ref()
                .zip(self.preview_viewport().as_ref())
                .is_some_and(|(applied, current)| applied.horizontal == current.horizontal)
    }

    pub(super) fn scroll_preview_horizontally(&mut self, delta: MouseScrollDelta) -> bool {
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (-x * 48.0, -y * 48.0),
            MouseScrollDelta::PixelDelta(position) => (-position.x as f32, -position.y as f32),
        };
        let delta = horizontal_delta(dx, dy, self.modifiers.shift_key());
        if delta == 0.0 || !self.preview_layout_is_current() {
            return false;
        }
        let Some((x, y)) = self.preview_at_cursor() else {
            return false;
        };
        let Some(target) = self
            .preview_frame
            .as_ref()
            .and_then(|frame| frame.horizontal_target_at(x, y + frame.scroll_y()))
        else {
            return false;
        };
        // Accumulate against requested state, even when the worker is still painting.
        let offset = self.preview_horizontal.offsets.offset(target) + delta;
        self.apply_preview_horizontal_offset(target, offset);
        true
    }

    pub(super) fn apply_preview_horizontal_offset(
        &mut self,
        target: PreviewHorizontalTarget,
        offset: f32,
    ) {
        let next = self.preview_horizontal.offsets.with_offset(target, offset);
        if next != self.preview_horizontal.offsets {
            self.preview_horizontal.offsets = next;
            self.request_preview_paint();
        }
    }

    pub(super) fn handle_horizontal_button(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        if state == ElementState::Released {
            if self.preview_horizontal.drag.is_some() {
                self.cancel_scrollbar_drag();
                return true;
            }
            return false;
        }
        if !self.preview_hit_geometry_is_current() {
            return false;
        }
        let Some((x, y)) = self.preview_at_cursor() else {
            return false;
        };
        let Some(frame) = &self.preview_frame else {
            return false;
        };
        let y = y + frame.scroll_y();
        let Some(target) = frame
            .horizontal_target_at(x, y)
            .filter(|target| target.track().contains(x, y))
        else {
            return false;
        };
        if target.thumb().contains(x, y) {
            self.preview_horizontal.drag = Some(HorizontalDrag {
                target,
                start_x: self.cursor_position.x,
            });
            self.refresh_window_guards(None);
        } else {
            self.apply_preview_horizontal_offset(target, target.offset_after_page(x));
        }
        true
    }

    pub(super) fn handle_horizontal_cursor(&mut self) -> bool {
        let Some(drag) = &self.preview_horizontal.drag else {
            return false;
        };
        let valid = self.window_accepts_editor_mutation()
            && self.preview_viewport().is_some_and(|viewport| {
                drag.target.matches_layout(
                    self.coordinator.view().generation,
                    viewport.width_px,
                    viewport.scale,
                )
            });
        if !valid {
            self.cancel_scrollbar_drag();
            return true;
        }
        let target = drag.target;
        let offset = target.offset_after_drag((self.cursor_position.x - drag.start_x) as f32);
        self.apply_preview_horizontal_offset(target, offset);
        true
    }
}

fn horizontal_delta(dx: f32, dy: f32, shift: bool) -> f32 {
    if shift && dx.abs() < dy.abs() {
        dy
    } else if shift || dx.abs() > dy.abs() {
        dx
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
