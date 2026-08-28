//! Source find/replace overlay geometry and native rendering.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalPosition;

use crate::interaction::SearchField;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SearchRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl SearchRect {
    fn contains(self, position: PhysicalPosition<f64>) -> bool {
        let x = position.x as f32;
        let y = position.y as f32;
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SearchLayout {
    popup: SearchRect,
    query: SearchRect,
    replacement: SearchRect,
    case_toggle: SearchRect,
    previous: SearchRect,
    next: SearchRect,
    close: SearchRect,
    replace: SearchRect,
    replace_all: SearchRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SearchHit {
    Query,
    Replacement,
    ToggleCase,
    Previous,
    Next,
    Close,
    Replace,
    ReplaceAll,
}

pub(super) fn search_field_display(
    search: &crate::interaction::SearchSession,
    field: SearchField,
) -> (String, usize) {
    let (mut display, cursor) = search.composed_field(field);
    if field == SearchField::Query {
        let summary = search.match_summary();
        display.push_str(&format!(
            "  [{}/{}{}]",
            summary.0,
            summary.1,
            if summary.2 { "+" } else { "" }
        ));
    }
    (display, cursor)
}

impl SearchLayout {
    pub(super) fn new(
        pane: super::preview_runtime::PaneRect,
        scale: f32,
        replace_visible: bool,
    ) -> Self {
        let scale = scale.max(0.5);
        let margin = 6.0 * scale;
        let row = 28.0 * scale;
        let gap = 4.0 * scale;
        let popup_height = margin * 2.0 + row + if replace_visible { row + gap } else { 0.0 };
        let popup = SearchRect {
            x: pane.x as f32,
            y: pane.y as f32,
            width: pane.width as f32,
            height: popup_height,
        };
        let inner_width = (popup.width - margin * 2.0).max(1.0);
        let button = ((inner_width - gap * 4.0) / 6.0).clamp(12.0 * scale, 26.0 * scale);
        let controls_width = button * 4.0 + gap * 3.0;
        let query = SearchRect {
            x: popup.x + margin,
            y: popup.y + margin,
            width: (inner_width - controls_width - gap).max(1.0),
            height: row,
        };
        let mut x = query.x + query.width + gap;
        let case_toggle = button_rect(&mut x, query.y, button, gap);
        let previous = button_rect(&mut x, query.y, button, gap);
        let next = button_rect(&mut x, query.y, button, gap);
        let close = button_rect(&mut x, query.y, button, gap);
        let replacement_y = query.y + row + gap;
        let action_width = ((inner_width - gap * 2.0) / 3.0).clamp(18.0 * scale, 54.0 * scale);
        let replacement = SearchRect {
            x: query.x,
            y: replacement_y,
            width: (inner_width - action_width * 2.0 - gap * 2.0).max(1.0),
            height: row,
        };
        let replace = SearchRect {
            x: replacement.x + replacement.width + gap,
            y: replacement_y,
            width: action_width,
            height: row,
        };
        let replace_all = SearchRect {
            x: replace.x + replace.width + gap,
            y: replacement_y,
            width: action_width,
            height: row,
        };
        Self {
            popup,
            query,
            replacement,
            case_toggle,
            previous,
            next,
            close,
            replace,
            replace_all,
        }
    }

    pub(super) fn hit(
        self,
        position: PhysicalPosition<f64>,
        replace_visible: bool,
    ) -> Option<SearchHit> {
        for (rect, hit) in [
            (self.query, SearchHit::Query),
            (self.case_toggle, SearchHit::ToggleCase),
            (self.previous, SearchHit::Previous),
            (self.next, SearchHit::Next),
            (self.close, SearchHit::Close),
        ] {
            if rect.contains(position) {
                return Some(hit);
            }
        }
        if replace_visible {
            for (rect, hit) in [
                (self.replacement, SearchHit::Replacement),
                (self.replace, SearchHit::Replace),
                (self.replace_all, SearchHit::ReplaceAll),
            ] {
                if rect.contains(position) {
                    return Some(hit);
                }
            }
        }
        self.popup.contains(position).then_some(SearchHit::Query)
    }

    pub(super) fn field_spec(
        self,
        focused: SearchField,
        scale: f32,
    ) -> stickymd_render::source::UiTextSpec {
        let rect = match focused {
            SearchField::Query => self.query,
            SearchField::Replacement => self.replacement,
        };
        let inset = 5.0 * scale.max(0.5);
        stickymd_render::source::UiTextSpec {
            x: rect.x + inset,
            y: rect.y + 3.0 * scale.max(0.5),
            width: (rect.width - inset * 2.0).max(1.0),
            scale,
        }
    }
}

pub(super) fn paint_search_overlay(
    pixmap: &mut Pixmap,
    projection: &mut Option<stickymd_render::source::SourceProjection>,
    search: &crate::interaction::SearchSession,
    layout: Option<SearchLayout>,
    scale: f32,
    dark: bool,
) {
    if !search.open {
        return;
    }
    let Some(layout) = layout else { return };
    let background = if dark {
        (48, 48, 45, 248)
    } else {
        (244, 241, 232, 248)
    };
    let field = if dark {
        (33, 33, 31, 255)
    } else {
        (255, 254, 249, 255)
    };
    let active = if dark {
        (74, 91, 112, 255)
    } else {
        (205, 220, 236, 255)
    };
    fill(pixmap, layout.popup, background);
    fill(
        pixmap,
        layout.query,
        if search.focused == SearchField::Query {
            active
        } else {
            field
        },
    );
    for rect in [
        layout.case_toggle,
        layout.previous,
        layout.next,
        layout.close,
    ] {
        fill(pixmap, rect, field);
    }
    if search.case_sensitive {
        fill(pixmap, inset(layout.case_toggle, 5.0 * scale), active);
    }
    if search.replace_visible {
        fill(
            pixmap,
            layout.replacement,
            if search.focused == SearchField::Replacement {
                active
            } else {
                field
            },
        );
        fill(pixmap, layout.replace, field);
        fill(pixmap, layout.replace_all, field);
    }
    let (query_display, query_cursor) = search_field_display(search, SearchField::Query);
    let (replacement, replacement_cursor) = search_field_display(search, SearchField::Replacement);
    let source_theme = if dark {
        stickymd_render::source::SourceTheme::Dark
    } else {
        stickymd_render::source::SourceTheme::Light
    };
    if let Some(projection) = projection {
        let query_spec = layout.field_spec(SearchField::Query, scale);
        if search.focused == SearchField::Query {
            projection.paint_ui_text_field(
                pixmap,
                &query_display,
                query_cursor,
                query_spec,
                source_theme,
            );
        } else {
            projection.paint_ui_text(pixmap, &query_display, query_spec, source_theme);
        }
        if search.replace_visible {
            let replacement_spec = layout.field_spec(SearchField::Replacement, scale);
            if search.focused == SearchField::Replacement {
                projection.paint_ui_text_field(
                    pixmap,
                    &replacement,
                    replacement_cursor,
                    replacement_spec,
                    source_theme,
                );
            } else {
                projection.paint_ui_text(pixmap, &replacement, replacement_spec, source_theme);
            }
        }
        for (label, rect) in [
            ("Aa", layout.case_toggle),
            ("<", layout.previous),
            (">", layout.next),
            ("x", layout.close),
        ] {
            projection.paint_ui_text(
                pixmap,
                label,
                stickymd_render::source::UiTextSpec {
                    x: rect.x + 4.0 * scale,
                    y: rect.y + 3.0 * scale,
                    width: (rect.width - 8.0 * scale).max(1.0),
                    scale,
                },
                source_theme,
            );
        }
        if search.replace_visible {
            for (label, rect) in [("R", layout.replace), ("All", layout.replace_all)] {
                projection.paint_ui_text(
                    pixmap,
                    label,
                    stickymd_render::source::UiTextSpec {
                        x: rect.x + 4.0 * scale,
                        y: rect.y + 3.0 * scale,
                        width: (rect.width - 8.0 * scale).max(1.0),
                        scale,
                    },
                    source_theme,
                );
            }
        }
    } else {
        paint_markers(pixmap, layout, scale, dark, search.replace_visible);
    }
}

fn button_rect(x: &mut f32, y: f32, size: f32, gap: f32) -> SearchRect {
    let rect = SearchRect {
        x: *x,
        y,
        width: size,
        height: size,
    };
    *x += size + gap;
    rect
}

fn inset(rect: SearchRect, amount: f32) -> SearchRect {
    SearchRect {
        x: rect.x + amount,
        y: rect.y + amount,
        width: (rect.width - amount * 2.0).max(1.0),
        height: (rect.height - amount * 2.0).max(1.0),
    }
}

fn paint_markers(
    pixmap: &mut Pixmap,
    layout: SearchLayout,
    scale: f32,
    dark: bool,
    replace_visible: bool,
) {
    let ink = if dark {
        (225, 222, 213, 255)
    } else {
        (58, 57, 53, 255)
    };
    let line = scale.max(1.0);
    fill(
        pixmap,
        SearchRect {
            x: layout.previous.x + 8.0 * scale,
            y: layout.previous.y + 13.0 * scale,
            width: 10.0 * scale,
            height: line,
        },
        ink,
    );
    fill(
        pixmap,
        SearchRect {
            x: layout.next.x + 8.0 * scale,
            y: layout.next.y + 13.0 * scale,
            width: 10.0 * scale,
            height: line,
        },
        ink,
    );
    fill(pixmap, inset(layout.close, 9.0 * scale), ink);
    if replace_visible {
        fill(pixmap, inset(layout.replace, 8.0 * scale), ink);
        fill(pixmap, inset(layout.replace_all, 6.0 * scale), ink);
    }
}

fn fill(pixmap: &mut Pixmap, rect: SearchRect, color: (u8, u8, u8, u8)) {
    let Some(rect) = Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) else {
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
    fn search_overlay_keeps_fields_and_actions_inside_normal_source_pane() {
        let layout = SearchLayout::new(
            super::super::preview_runtime::PaneRect {
                x: 0,
                y: 34,
                width: 520,
                height: 646,
            },
            1.0,
            true,
        );
        for rect in [
            layout.query,
            layout.replacement,
            layout.case_toggle,
            layout.previous,
            layout.next,
            layout.close,
            layout.replace,
            layout.replace_all,
        ] {
            assert!(rect.x >= layout.popup.x);
            assert!(rect.y >= layout.popup.y);
            assert!(rect.x + rect.width <= layout.popup.x + layout.popup.width + 0.01);
            assert!(rect.y + rect.height <= layout.popup.y + layout.popup.height + 0.01);
        }
    }

    #[test]
    fn search_overlay_stays_inside_compact_split_source_pane() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let width = (109.0_f32 * scale).round() as u32;
            let layout = SearchLayout::new(
                super::super::preview_runtime::PaneRect {
                    x: 0,
                    y: (34.0_f32 * scale).round() as u32,
                    width,
                    height: (86.0_f32 * scale).round() as u32,
                },
                scale,
                true,
            );
            for rect in [
                layout.query,
                layout.replacement,
                layout.case_toggle,
                layout.previous,
                layout.next,
                layout.close,
                layout.replace,
                layout.replace_all,
            ] {
                assert!(rect.x >= layout.popup.x);
                assert!(rect.x + rect.width <= layout.popup.x + layout.popup.width + 0.01);
            }
        }
    }
}
