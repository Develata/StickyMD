//! Source find/replace overlay geometry and native rendering.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use tiny_skia::{Paint, Pixmap, PixmapPaint, Rect, Transform};
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

/// The panel's widest extent: a narrow strip at the Source pane's top-right, as VS Code's find
/// widget (plan 07); a narrower pane gets the panel at its full width.
const PANEL_MAX_WIDTH_DIP: f32 = 300.0;

/// The panel's one opacity (USER decision 2026-10-21): its surface — background, fields and
/// buttons — is painted opaque into a layer and laid over the note at this opacity, so every part
/// is equally transparent and parts differ only in colour; text and markers stay opaque on top.
/// It blends over the note in the frame, so the window's own opacity applies on top of it.
const PANEL_OPACITY: f32 = 0.4;

/// The panel surface's colours, all opaque within the layer (`PANEL_OPACITY` applies to it whole).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanelColors {
    background: (u8, u8, u8),
    field: (u8, u8, u8),
    active: (u8, u8, u8),
}

fn panel_colors(dark: bool) -> PanelColors {
    if dark {
        PanelColors {
            background: (48, 48, 45),
            field: (33, 33, 31),
            active: (74, 91, 112),
        }
    } else {
        PanelColors {
            background: (244, 241, 232),
            field: (255, 254, 249),
            active: (205, 220, 236),
        }
    }
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
        let popup_width = (pane.width as f32).min(PANEL_MAX_WIDTH_DIP * scale);
        let popup = SearchRect {
            x: pane.x as f32 + pane.width as f32 - popup_width,
            y: pane.y as f32,
            width: popup_width,
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
    paint_panel_surface(pixmap, layout, search, scale, dark);
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

/// Paints the panel surface opaque into a layer the panel's size and lays it over the frame at
/// `PANEL_OPACITY`, so overlapping parts (a field on the background) do not stack their alpha.
fn paint_panel_surface(
    pixmap: &mut Pixmap,
    layout: SearchLayout,
    search: &crate::interaction::SearchSession,
    scale: f32,
    dark: bool,
) {
    let popup = layout.popup;
    let (left, top) = (popup.x.floor(), popup.y.floor());
    let width = (popup.x + popup.width - left).ceil().max(1.0) as u32;
    let height = (popup.y + popup.height - top).ceil().max(1.0) as u32;
    let Some(mut layer) = Pixmap::new(width, height) else {
        return;
    };
    let at = |rect: SearchRect| SearchRect {
        x: rect.x - left,
        y: rect.y - top,
        ..rect
    };
    let opaque = |(r, g, b): (u8, u8, u8)| (r, g, b, 255);
    let PanelColors {
        background,
        field,
        active,
    } = panel_colors(dark);
    let field_color = |which: SearchField| {
        opaque(if search.focused == which {
            active
        } else {
            field
        })
    };
    fill(&mut layer, at(popup), opaque(background));
    fill(
        &mut layer,
        at(layout.query),
        field_color(SearchField::Query),
    );
    for rect in [
        layout.case_toggle,
        layout.previous,
        layout.next,
        layout.close,
    ] {
        fill(&mut layer, at(rect), opaque(field));
    }
    if search.case_sensitive {
        fill(
            &mut layer,
            at(inset(layout.case_toggle, 5.0 * scale)),
            opaque(active),
        );
    }
    if search.replace_visible {
        fill(
            &mut layer,
            at(layout.replacement),
            field_color(SearchField::Replacement),
        );
        fill(&mut layer, at(layout.replace), opaque(field));
        fill(&mut layer, at(layout.replace_all), opaque(field));
    }
    pixmap.draw_pixmap(
        left as i32,
        top as i32,
        layer.as_ref(),
        &PixmapPaint {
            opacity: PANEL_OPACITY,
            ..PixmapPaint::default()
        },
        Transform::identity(),
        None,
    );
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

    /// Plan 07: the panel is a strip at the Source pane's top-right, at most 300 DIP wide and
    /// the pane's full width when the pane is narrower; in every view, size and DPI it stays
    /// inside the Source pane, so it never meets the Source scrollbar gutter or the Split sync
    /// button on the divider.
    #[test]
    fn the_panel_is_a_narrow_strip_at_the_source_pane_top_right() {
        use super::super::controls::split_sync_rect;
        use super::super::preview_runtime::geometry;
        use crate::config::ViewMode;
        use winit::dpi::PhysicalSize;
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for (width, height) in [
                (520.0, 680.0),
                (220.0, 120.0),
                (330.0, 360.0),
                (1200.0, 900.0),
            ] {
                let size = PhysicalSize::new(
                    (width * scale).round() as u32,
                    (height * scale).round() as u32,
                );
                for mode in [ViewMode::Source, ViewMode::Split] {
                    let view = geometry(mode, size, scale);
                    let pane = view.source.unwrap();
                    let label = format!("{mode:?} {size:?} at {scale}");
                    for replace in [false, true] {
                        let layout = SearchLayout::new(pane, scale, replace);
                        let popup = layout.popup;
                        let (left, right) = (pane.x as f32, (pane.x + pane.width) as f32);
                        assert!(
                            (popup.x + popup.width - right).abs() < 0.01,
                            "{label}: right edge"
                        );
                        assert!(popup.x >= left - 0.01, "{label}: left edge");
                        let expected = (pane.width as f32).min(PANEL_MAX_WIDTH_DIP * scale);
                        assert!((popup.width - expected).abs() < 0.01, "{label}: width");
                        assert_eq!(popup.y, pane.y as f32, "{label}: top");
                        if let Some(divider) = view.divider_x {
                            let sync =
                                split_sync_rect(divider, view.toolbar_height, f64::from(scale));
                            assert!(
                                f64::from(popup.x + popup.width) <= sync.x,
                                "{label}: the panel meets the sync button {sync:?}"
                            );
                        }
                    }
                }
            }
        }
        // A wide pane leaves the note's left part uncovered.
        let layout = SearchLayout::new(
            super::super::preview_runtime::PaneRect {
                x: 0,
                y: 34,
                width: 500,
                height: 646,
            },
            1.0,
            true,
        );
        assert_eq!((layout.popup.x, layout.popup.width), (200.0, 300.0));
    }

    /// The whole panel surface has one opacity: over a black frame, the background, an
    /// unfocused field and the focused field each come out as their own colour times
    /// `PANEL_OPACITY`, so a field on the background does not stack alpha; in both themes.
    #[test]
    fn the_panel_surface_has_one_fixed_opacity() {
        use super::super::preview_runtime::PaneRect;
        assert!((0.25..=0.6).contains(&PANEL_OPACITY), "{PANEL_OPACITY}");
        let layout = SearchLayout::new(
            PaneRect {
                x: 0,
                y: 34,
                width: 500,
                height: 646,
            },
            1.0,
            true,
        );
        let expected = |channel: u8| (f32::from(channel) * PANEL_OPACITY).round() as i32;
        let pixel = |pixmap: &Pixmap, rect: SearchRect, dx: f32| {
            let color = pixmap
                .pixel((rect.x + dx) as u32, (rect.y + rect.height * 0.5) as u32)
                .unwrap();
            [color.red(), color.green(), color.blue()].map(i32::from)
        };
        for dark in [false, true] {
            let mut search = crate::interaction::SearchSession::default();
            search.open = true;
            search.replace_visible = true;
            search.focused = SearchField::Replacement;
            let mut pixmap = Pixmap::new(520, 200).unwrap();
            pixmap.fill(tiny_skia::Color::BLACK);
            paint_search_overlay(&mut pixmap, &mut None, &search, Some(layout), 1.0, dark);
            let colors = panel_colors(dark);
            // A point of the background between the query field and the panel's left edge.
            let background = SearchRect {
                x: layout.popup.x,
                y: layout.query.y,
                width: layout.query.x - layout.popup.x,
                height: layout.query.height,
            };
            for (rect, dx, color, part) in [
                (background, 1.0, colors.background, "background"),
                (
                    layout.query,
                    layout.query.width * 0.5,
                    colors.field,
                    "field",
                ),
                (
                    layout.replacement,
                    layout.replacement.width * 0.5,
                    colors.active,
                    "focused field",
                ),
            ] {
                let got = pixel(&pixmap, rect, dx);
                let want = [color.0, color.1, color.2].map(expected);
                for (g, w) in got.iter().zip(want) {
                    assert!(
                        (g - w).abs() <= 1,
                        "dark {dark}: {part} {got:?} != {want:?}"
                    );
                }
            }
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
