//! Read-only Preview keyboard, selection, and activation interaction.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-link-safety

use winit::dpi::PhysicalPosition;
use winit::keyboard::KeyCode;
use winit::window::CursorIcon;

use super::StickyApp;
use crate::config::ViewMode;
use crate::instruction::{AppIntent, PersistenceIntent, PreviewIntent, SaveReason};

const MEANINGFUL_DRAG_DIP: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewCommand {
    None,
    ClearSelection,
    SelectAll,
    Copy,
    Save,
}

impl StickyApp {
    pub(super) fn cursor_icon_for_position(&self, position: PhysicalPosition<f64>) -> CursorIcon {
        let Some(geometry) = self.view_geometry() else {
            return CursorIcon::Default;
        };
        if position.y >= 0.0 && position.y < geometry.toolbar_height as f64 {
            return CursorIcon::Pointer;
        }
        if let Some(pane) = geometry.preview
            && pane.contains(position.x as f32, position.y as f32)
        {
            let Some(frame) = &self.preview_frame else {
                return CursorIcon::Text;
            };
            let x = position.x as f32 - pane.x as f32;
            let y = position.y as f32 - pane.y as f32 + frame.scroll_y();
            return if frame.action_at(x, y).is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Text
            };
        }
        if geometry
            .source
            .is_some_and(|pane| pane.contains(position.x as f32, position.y as f32))
        {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        }
    }

    pub(super) fn handle_preview_key(
        &mut self,
        code: Option<KeyCode>,
        shortcut: bool,
        shift: bool,
    ) {
        match preview_command(code, shortcut, shift) {
            PreviewCommand::None => {}
            PreviewCommand::ClearSelection => {
                self.preview_selection = stickymd_render::preview::PreviewSelection::default();
                self.request_preview_paint();
            }
            PreviewCommand::SelectAll => {
                if let Some(frame) = &self.preview_frame {
                    self.preview_selection = frame.select_all();
                    self.request_preview_paint();
                }
            }
            PreviewCommand::Copy => {
                let copied = self.preview_frame.as_ref().and_then(|frame| {
                    frame
                        .copy_selection(self.preview_selection)
                        .map(str::to_owned)
                });
                if let Some(text) = copied {
                    self.dispatch(AppIntent::WriteClipboard { text });
                }
            }
            PreviewCommand::Save => {
                self.dispatch_persistence_intent(
                    None,
                    PersistenceIntent::SaveNow(SaveReason::Manual),
                );
            }
        }
    }

    pub(super) fn toolbar_mode_at_cursor(&self) -> Option<ViewMode> {
        let geometry = self.view_geometry()?;
        let x = self.cursor_position.x as f32;
        let y = self.cursor_position.y as f32;
        if y < 0.0 || y >= geometry.toolbar_height as f32 {
            return None;
        }
        let scale = (geometry.toolbar_height as f32 / 34.0).max(0.5);
        let index = ((x - 7.0 * scale) / (38.0 * scale)).floor() as i32;
        match index {
            0 => Some(ViewMode::Source),
            1 => Some(ViewMode::Split),
            2 => Some(ViewMode::Preview),
            _ => None,
        }
    }

    pub(super) fn preview_at_cursor(&self) -> Option<(f32, f32)> {
        let pane = self.view_geometry()?.preview?;
        let x = self.cursor_position.x as f32;
        let y = self.cursor_position.y as f32;
        pane.contains(x, y)
            .then_some((x - pane.x as f32, y - pane.y as f32))
    }

    pub(super) fn press_preview_selection(&mut self) {
        let Some((x, y)) = self.preview_at_cursor() else {
            return;
        };
        let Some(frame) = &self.preview_frame else {
            return;
        };
        let document_y = y + frame.scroll_y();
        let hit = frame.hit_test(x, document_y);
        self.preview_selection = if self.modifiers.shift_key() {
            stickymd_render::preview::PreviewSelection {
                anchor: self.preview_selection.anchor,
                active: hit,
            }
        } else {
            stickymd_render::preview::PreviewSelection::caret(hit)
        };
        self.preview_press_action = frame.action_at(x, document_y).cloned();
        self.preview_press_position = Some(self.cursor_position);
        self.preview_dragging = true;
        self.preview_drag_moved = self.modifiers.shift_key();
        self.request_preview_paint();
    }

    pub(super) fn extend_preview_selection(&mut self, position: PhysicalPosition<f64>) {
        let Some(pane) = self.view_geometry().and_then(|geometry| geometry.preview) else {
            return;
        };
        let Some(frame) = &self.preview_frame else {
            return;
        };
        let x = (position.x as f32 - pane.x as f32).clamp(0.0, pane.width as f32);
        let y =
            (position.y as f32 - pane.y as f32).clamp(0.0, pane.height as f32) + frame.scroll_y();
        self.preview_selection.active = frame.hit_test(x, y);
        self.request_preview_paint();
    }

    pub(super) fn release_preview_selection(&mut self) {
        self.preview_dragging = false;
        self.preview_press_position = None;
        let action = self.preview_press_action.take();
        if self.preview_drag_moved {
            return;
        }
        if let Some(action) = action {
            self.dispatch_preview_intent(PreviewIntent::Activate(action));
        }
    }
}

fn preview_command(code: Option<KeyCode>, shortcut: bool, shift: bool) -> PreviewCommand {
    match (shortcut, shift, code) {
        (false, false, Some(KeyCode::Escape)) => PreviewCommand::ClearSelection,
        (true, _, Some(KeyCode::KeyA)) => PreviewCommand::SelectAll,
        (true, _, Some(KeyCode::KeyC | KeyCode::Insert)) => PreviewCommand::Copy,
        (true, _, Some(KeyCode::KeyS)) => PreviewCommand::Save,
        _ => PreviewCommand::None,
    }
}

pub(super) fn meaningful_preview_drag(
    start: PhysicalPosition<f64>,
    current: PhysicalPosition<f64>,
    scale: f64,
) -> bool {
    let threshold = MEANINGFUL_DRAG_DIP * scale.max(0.5);
    let dx = current.x - start.x;
    let dy = current.y - start.y;
    dx.mul_add(dx, dy * dy) >= threshold * threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_click_tolerates_jitter_but_selection_drag_crosses_dip_threshold() {
        let start = PhysicalPosition::new(100.0, 100.0);
        assert!(!meaningful_preview_drag(
            start,
            PhysicalPosition::new(102.0, 101.0),
            1.0
        ));
        assert!(meaningful_preview_drag(
            start,
            PhysicalPosition::new(104.0, 100.0),
            1.0
        ));
        assert!(!meaningful_preview_drag(
            start,
            PhysicalPosition::new(104.0, 100.0),
            2.0
        ));
    }

    #[test]
    fn preview_shortcuts_are_read_only_and_reserve_copy_select_all_and_save() {
        assert_eq!(
            preview_command(Some(KeyCode::KeyA), true, false),
            PreviewCommand::SelectAll
        );
        assert_eq!(
            preview_command(Some(KeyCode::KeyC), true, false),
            PreviewCommand::Copy
        );
        assert_eq!(
            preview_command(Some(KeyCode::KeyS), true, false),
            PreviewCommand::Save
        );
        for blocked in [KeyCode::KeyX, KeyCode::KeyV, KeyCode::Backspace] {
            assert_eq!(
                preview_command(Some(blocked), true, false),
                PreviewCommand::None
            );
        }
        assert_eq!(
            preview_command(Some(KeyCode::Insert), true, false),
            PreviewCommand::Copy
        );
        assert_eq!(
            preview_command(Some(KeyCode::Delete), false, true),
            PreviewCommand::None
        );
        assert_eq!(
            preview_command(Some(KeyCode::Insert), false, true),
            PreviewCommand::None
        );
    }
}
