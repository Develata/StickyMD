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
    /// `None` in the compact tier, which leaves navigation to the keyboard.
    previous: Option<SearchRect>,
    next: Option<SearchRect>,
    close: SearchRect,
    replace: SearchRect,
    replace_all: SearchRect,
    /// The inset of field text from its field's edges, in physical pixels.
    text_inset: f32,
    /// The inset of a button's label from the button's left and right edges, in physical pixels.
    label_pad: f32,
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

/// The panel's fill colours. The background lets the note show through it (about half
/// transparent); fields and buttons stay nearly opaque so typed text reads clearly. They blend
/// over the note in the frame, so the window's own opacity applies on top and does not change
/// these ratios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PanelColors {
    background: (u8, u8, u8, u8),
    field: (u8, u8, u8, u8),
    active: (u8, u8, u8, u8),
}

fn panel_colors(dark: bool) -> PanelColors {
    if dark {
        PanelColors {
            background: (48, 48, 45, 136),
            field: (33, 33, 31, 224),
            active: (74, 91, 112, 232),
        }
    } else {
        PanelColors {
            background: (244, 241, 232, 136),
            field: (255, 254, 249, 224),
            active: (205, 220, 236, 232),
        }
    }
}

/// The least query text room, in DIP, the full tier must leave; below it the panel takes the
/// compact tier (plan 07).
const FULL_QUERY_TEXT_MIN_DIP: f32 = 40.0;

/// One tier of the panel's spacing and control sizes, in physical pixels (plan 07). The full tier
/// lays out `Aa < > x` beside the query; the compact tier, for a narrow Source pane, tightens the
/// spacing and drops `< >`, whose Enter, Shift+Enter, Up/Down and F3 routes remain.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Tier {
    margin: f32,
    gap: f32,
    text_inset: f32,
    label_pad: f32,
    button: f32,
    replace: f32,
    replace_all: f32,
    navigation: bool,
}

impl Tier {
    fn for_panel(popup_width: f32, scale: f32) -> Self {
        let full = Self::full(popup_width, scale);
        let full_text = full.query_width(popup_width) - full.text_inset * 2.0;
        if full_text >= FULL_QUERY_TEXT_MIN_DIP * scale {
            full
        } else {
            Self::compact(scale)
        }
    }

    fn full(popup_width: f32, scale: f32) -> Self {
        let (margin, gap) = (6.0 * scale, 4.0 * scale);
        let inner = (popup_width - margin * 2.0).max(1.0);
        let action = ((inner - gap * 2.0) / 3.0).clamp(18.0 * scale, 54.0 * scale);
        Self {
            margin,
            gap,
            text_inset: 5.0 * scale,
            label_pad: 4.0 * scale,
            button: ((inner - gap * 4.0) / 6.0).clamp(12.0 * scale, 26.0 * scale),
            replace: action,
            replace_all: action,
            navigation: true,
        }
    }

    /// Fixed sizes, each button just wide enough for its label in every UI font candidate
    /// inside the label padding (`SearchLayout::label_spec`), so the fields keep the rest.
    fn compact(scale: f32) -> Self {
        Self {
            margin: 4.0 * scale,
            gap: 2.0 * scale,
            text_inset: 3.0 * scale,
            label_pad: 2.0 * scale,
            button: 22.0 * scale,
            replace: 22.0 * scale,
            replace_all: 28.0 * scale,
            navigation: false,
        }
    }

    fn inner_width(self, popup_width: f32) -> f32 {
        (popup_width - self.margin * 2.0).max(1.0)
    }

    /// The query field's width: the inner width less each button beside it and the gap before it.
    fn query_width(self, popup_width: f32) -> f32 {
        let buttons = if self.navigation { 4.0 } else { 2.0 };
        (self.inner_width(popup_width) - buttons * (self.button + self.gap)).max(1.0)
    }
}

impl SearchLayout {
    pub(super) fn new(
        pane: super::preview_runtime::PaneRect,
        scale: f32,
        replace_visible: bool,
    ) -> Self {
        let scale = scale.max(0.5);
        let row = 28.0 * scale;
        let popup_width = (pane.width as f32).min(PANEL_MAX_WIDTH_DIP * scale);
        let tier = Tier::for_panel(popup_width, scale);
        let (margin, gap) = (tier.margin, tier.gap);
        let popup_height = margin * 2.0 + row + if replace_visible { row + gap } else { 0.0 };
        let popup = SearchRect {
            x: pane.x as f32 + pane.width as f32 - popup_width,
            y: pane.y as f32,
            width: popup_width,
            height: popup_height,
        };
        let query = SearchRect {
            x: popup.x + margin,
            y: popup.y + margin,
            width: tier.query_width(popup.width),
            height: row,
        };
        let button = tier.button;
        let mut x = query.x + query.width + gap;
        let case_toggle = button_rect(&mut x, query.y, button, gap);
        let previous = tier
            .navigation
            .then(|| button_rect(&mut x, query.y, button, gap));
        let next = tier
            .navigation
            .then(|| button_rect(&mut x, query.y, button, gap));
        let close = button_rect(&mut x, query.y, button, gap);
        let replacement_y = query.y + row + gap;
        let replacement = SearchRect {
            x: query.x,
            y: replacement_y,
            width: (tier.inner_width(popup.width) - tier.replace - tier.replace_all - gap * 2.0)
                .max(1.0),
            height: row,
        };
        let replace = SearchRect {
            x: replacement.x + replacement.width + gap,
            y: replacement_y,
            width: tier.replace,
            height: row,
        };
        let replace_all = SearchRect {
            x: replace.x + replace.width + gap,
            y: replacement_y,
            width: tier.replace_all,
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
            text_inset: tier.text_inset,
            label_pad: tier.label_pad,
        }
    }

    /// The panel's buttons, each with its label and what a press on it does; `None` where the
    /// tier or a hidden replacement row omits it. Paint and hit testing read this one list.
    fn buttons(self, replace_visible: bool) -> [Option<(&'static str, SearchRect, SearchHit)>; 6] {
        [
            Some(("Aa", self.case_toggle, SearchHit::ToggleCase)),
            self.previous.map(|rect| ("<", rect, SearchHit::Previous)),
            self.next.map(|rect| (">", rect, SearchHit::Next)),
            Some(("x", self.close, SearchHit::Close)),
            replace_visible.then_some(("R", self.replace, SearchHit::Replace)),
            replace_visible.then_some(("All", self.replace_all, SearchHit::ReplaceAll)),
        ]
    }

    pub(super) fn hit(
        self,
        position: PhysicalPosition<f64>,
        replace_visible: bool,
    ) -> Option<SearchHit> {
        let fields = [
            Some((self.query, SearchHit::Query)),
            replace_visible.then_some((self.replacement, SearchHit::Replacement)),
        ];
        let buttons = self.buttons(replace_visible);
        let buttons = buttons
            .into_iter()
            .flatten()
            .map(|(_, rect, hit)| (rect, hit));
        for (rect, hit) in fields.into_iter().flatten().chain(buttons) {
            if rect.contains(position) {
                return Some(hit);
            }
        }
        self.popup.contains(position).then_some(SearchHit::Query)
    }

    /// Where a button's label is painted: padded inside the button and clipped to it.
    fn label_spec(self, rect: SearchRect, scale: f32) -> stickymd_render::source::UiTextSpec {
        stickymd_render::source::UiTextSpec {
            x: rect.x + self.label_pad,
            y: rect.y + 3.0 * scale,
            width: (rect.width - self.label_pad * 2.0).max(1.0),
            scale,
        }
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
        let inset = self.text_inset;
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
    let PanelColors {
        background,
        field,
        active,
    } = panel_colors(dark);
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
    let buttons = layout.buttons(search.replace_visible).into_iter().flatten();
    for (_, rect, _) in buttons.clone() {
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
        for (label, rect, _) in buttons {
            let spec = layout.label_spec(rect, scale);
            projection.paint_ui_text(pixmap, label, spec, source_theme);
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
    for rect in [layout.previous, layout.next].into_iter().flatten() {
        fill(
            pixmap,
            SearchRect {
                x: rect.x + 8.0 * scale,
                y: rect.y + 13.0 * scale,
                width: 10.0 * scale,
                height: line,
            },
            ink,
        );
    }
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

    use super::super::preview_runtime::{ViewGeometry, geometry};
    use crate::config::ViewMode;

    /// The least room field text gets in any view, in DIP: about one and a half CJK glyphs, or
    /// three Latin ones, at the 13-DIP UI font.
    const FIELD_TEXT_MIN_DIP: f32 = 20.0;

    /// The real Source and Split views at six window sizes in DIP, and four DPIs: (label, view,
    /// scale). Among the sizes are the 220x120 minimum and, about the compact threshold, a
    /// 396-DIP Split whose Source pane is just below 178 DIP and a 398-DIP one just above it,
    /// where the full tier's buttons and label room are narrowest.
    fn views() -> Vec<(String, ViewGeometry, f32)> {
        let mut views = Vec::new();
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            for (width, height) in [
                (520.0, 680.0),
                (220.0, 120.0),
                (330.0, 360.0),
                (396.0, 360.0),
                (398.0, 360.0),
                (1200.0, 900.0),
            ] {
                let size = winit::dpi::PhysicalSize::new(
                    (width * scale).round() as u32,
                    (height * scale).round() as u32,
                );
                for mode in [ViewMode::Source, ViewMode::Split] {
                    let label = format!("{mode:?} {width}x{height} DIP at {scale}");
                    views.push((label, geometry(mode, size, scale), scale));
                }
            }
        }
        views
    }

    /// `text`'s advance at the 13-DIP UI size in each installed UI font candidate, in physical
    /// pixels, shaped as `SourceProjection::paint_ui_text` shapes a label: the machine's
    /// first candidate is what it paints with, and any later one is what another machine
    /// without the earlier ones paints with.
    fn label_advances(
        fonts: &mut cosmic_text::FontSystem,
        text: &str,
        scale: f32,
    ) -> Vec<(&'static str, f32)> {
        use cosmic_text::fontdb::{Family, Query, Stretch, Style, Weight};
        use cosmic_text::{Attrs, Buffer, Metrics, Shaping, Wrap};

        let mut advances = Vec::new();
        for &family in stickymd_render::source::CJK_CANDIDATES {
            let query = Query {
                families: &[Family::Name(family)],
                weight: Weight::NORMAL,
                stretch: Stretch::Normal,
                style: Style::Normal,
            };
            if fonts.db().query(&query).is_none() {
                continue;
            }
            let mut buffer = Buffer::new_empty(Metrics::new(13.0 * scale, 20.0 * scale));
            buffer.set_wrap(Wrap::None);
            let attrs = Attrs::new().family(cosmic_text::Family::Name(family));
            buffer.set_text(text, &attrs, Shaping::Advanced, None);
            buffer.shape_until_scroll(fonts, false);
            let advance = buffer
                .layout_runs()
                .map(|run| run.line_w)
                .fold(0.0, f32::max);
            advances.push((family, advance));
        }
        advances
    }

    fn overlap(a: SearchRect, b: SearchRect) -> bool {
        a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
    }

    /// Plan 07: in every real view, size and DPI each field leaves at least
    /// `FIELD_TEXT_MIN_DIP` for its text, and each button's label, as Cosmic Text shapes it in
    /// every installed UI font candidate, fits inside the button's label padding. Fields and buttons stay inside the panel without
    /// overlapping, and a press on each button reaches it. A Source pane whose full tier would
    /// leave the query less than 40 DIP of text takes the compact tier: no `< >` buttons, so
    /// no press reaches Previous or Next.
    #[test]
    fn every_view_leaves_readable_fields_and_whole_button_labels() {
        let mut fonts = cosmic_text::FontSystem::new();
        let mut measured = 0;
        let mut tiers_seen = [false; 2];
        for (label, view, scale) in views() {
            let pane = view.source.unwrap();
            for replace in [false, true] {
                let layout = SearchLayout::new(pane, scale, replace);
                let label = format!("{label}, replace {replace}");
                let popup = layout.popup;
                let compact = layout.previous.is_none();
                assert_eq!(layout.next.is_none(), compact, "{label}");
                tiers_seen[usize::from(compact)] = true;
                let mut rects = vec![layout.query];
                for field in [SearchField::Query, SearchField::Replacement] {
                    if field == SearchField::Replacement && !replace {
                        continue;
                    }
                    let room = layout.field_spec(field, scale).width / scale;
                    assert!(
                        room >= FIELD_TEXT_MIN_DIP,
                        "{label}: {field:?} leaves {room} DIP for text"
                    );
                }
                if replace {
                    rects.push(layout.replacement);
                }
                for (text, rect, hit) in layout.buttons(replace).into_iter().flatten() {
                    let spec = layout.label_spec(rect, scale);
                    for (family, advance) in label_advances(&mut fonts, text, scale) {
                        assert!(
                            advance > 0.0 && advance <= spec.width,
                            "{label}: {text:?} in {family} needs {} DIP of {}",
                            advance / scale,
                            spec.width / scale
                        );
                        measured += 1;
                    }
                    let center = PhysicalPosition::new(
                        f64::from(rect.x + rect.width / 2.0),
                        f64::from(rect.y + rect.height / 2.0),
                    );
                    assert_eq!(layout.hit(center, replace), Some(hit), "{label}: {text:?}");
                    rects.push(rect);
                }
                for (index, rect) in rects.iter().enumerate() {
                    assert!(
                        rect.x >= popup.x - 0.01
                            && rect.y >= popup.y - 0.01
                            && rect.x + rect.width <= popup.x + popup.width + 0.01
                            && rect.y + rect.height <= popup.y + popup.height + 0.01,
                        "{label}: {rect:?} outside {popup:?}"
                    );
                    for other in &rects[index + 1..] {
                        assert!(!overlap(*rect, *other), "{label}: {rect:?} meets {other:?}");
                    }
                }
                if compact {
                    for step_x in 0..=40 {
                        for step_y in 0..=20 {
                            let position = PhysicalPosition::new(
                                f64::from(popup.x + popup.width * step_x as f32 / 40.0),
                                f64::from(popup.y + popup.height * step_y as f32 / 20.0),
                            );
                            let hit = layout.hit(position, replace);
                            assert!(
                                !matches!(hit, Some(SearchHit::Previous | SearchHit::Next)),
                                "{label}: {position:?} reaches {hit:?}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(tiers_seen, [true, true], "both tiers are exercised");
        assert!(measured > 0, "no UI font candidate is installed");
    }

    /// The tier each real view takes: the minimum, 330-DIP and 396-DIP Splits are compact; the
    /// 398-DIP and default 520-DIP Splits and every Source view at these sizes keep the full tier.
    #[test]
    fn narrow_source_panes_take_the_compact_tier() {
        for (label, view, scale) in views() {
            let compact = SearchLayout::new(view.source.unwrap(), scale, true)
                .previous
                .is_none();
            let expected = ["Split 220x", "Split 330x", "Split 396x"]
                .iter()
                .any(|prefix| label.starts_with(prefix));
            assert_eq!(compact, expected, "{label}");
        }
    }

    /// Plan 07: the panel is a strip at the Source pane's top-right, at most 300 DIP wide and
    /// the pane's full width when the pane is narrower; in every view, size and DPI it stays
    /// inside the Source pane, so it never meets the Source scrollbar gutter or the Split sync
    /// button on the divider.
    #[test]
    fn the_panel_is_a_narrow_strip_at_the_source_pane_top_right() {
        use super::super::controls::split_sync_rect;
        for (label, view, scale) in views() {
            let pane = view.source.unwrap();
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
                assert!(
                    popup.y + popup.height <= (pane.y + pane.height) as f32 + 0.01,
                    "{label}: bottom"
                );
                if let Some(divider) = view.divider_x {
                    let sync = split_sync_rect(divider, view.toolbar_height, f64::from(scale));
                    assert!(
                        f64::from(popup.x + popup.width) <= sync.x,
                        "{label}: the panel meets the sync button {sync:?}"
                    );
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

    /// The background is about half transparent; fields, buttons and the focused field are
    /// nearly opaque and more opaque than it, in both themes.
    #[test]
    fn the_panel_is_half_transparent_with_readable_fields() {
        for dark in [false, true] {
            let colors = panel_colors(dark);
            let background = colors.background.3;
            assert!(
                (112..=144).contains(&background),
                "dark {dark}: {background}"
            );
            for alpha in [colors.field.3, colors.active.3] {
                assert!(alpha >= 208 && alpha > background, "dark {dark}: {alpha}");
            }
        }
    }
}
