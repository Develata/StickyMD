//! Shared scrollbar geometry and gesture state for both document panes.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalPosition;

use super::preview_runtime::PaneRect;

pub(super) const GUTTER_DIP: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScrollPane {
    Source,
    Preview,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Scrollbar {
    pub pane: ScrollPane,
    pub track: PaneRect,
    pub thumb_top: f64,
    pub thumb_height: f64,
}

impl Scrollbar {
    pub fn new(
        pane: ScrollPane,
        content: PaneRect,
        dpi: f32,
        fraction: f64,
        visible: f64,
        reserve_split_control: bool,
    ) -> Option<Self> {
        if !fraction.is_finite() || !visible.is_finite() || visible >= 1.0 {
            return None;
        }
        let inset = (8.0 * dpi).round().max(1.0) as u32;
        let top_inset = if reserve_split_control {
            (28.0 * dpi).round() as u32
        } else {
            inset
        };
        let track = PaneRect {
            x: content.x + content.width + (2.0 * dpi).round() as u32,
            y: content.y + top_inset,
            width: (10.0 * dpi).round().max(1.0) as u32,
            height: content.height.saturating_sub(top_inset + inset),
        };
        if track.height < 2 {
            return None;
        }
        let height = f64::from(track.height);
        let thumb_height = (height * visible.clamp(0.0, 1.0))
            .max(f64::from(24.0 * dpi))
            .min(height - 1.0);
        Some(Self {
            pane,
            track,
            thumb_top: f64::from(track.y) + fraction.clamp(0.0, 1.0) * (height - thumb_height),
            thumb_height,
        })
    }

    pub fn contains(self, position: PhysicalPosition<f64>) -> bool {
        self.track.contains(position.x as f32, position.y as f32)
    }

    pub fn press(self, y: f64) -> ScrollbarPress {
        if y < self.thumb_top {
            ScrollbarPress::Page(-1.0)
        } else if y >= self.thumb_top + self.thumb_height {
            ScrollbarPress::Page(1.0)
        } else {
            ScrollbarPress::Drag(ScrollbarDrag {
                pane: self.pane,
                grab: (y - self.thumb_top) / self.thumb_height,
            })
        }
    }

    pub fn fraction_at(self, drag: ScrollbarDrag, y: f64) -> f64 {
        ((y - f64::from(self.track.y) - drag.grab * self.thumb_height)
            / (f64::from(self.track.height) - self.thumb_height))
            .clamp(0.0, 1.0)
    }

    pub fn paint(self, pixmap: &mut Pixmap, emphasized: bool, dark: bool) {
        let track_width = self.track.width as f32;
        let width = track_width * if emphasized { 0.7 } else { 0.3 };
        let Some(rect) = Rect::from_xywh(
            self.track.x as f32 + (track_width - width) / 2.0,
            self.thumb_top as f32,
            width,
            self.thumb_height as f32,
        ) else {
            return;
        };
        let mut paint = Paint::default();
        let shade = match (dark, emphasized) {
            (true, true) => 190,
            (true, false) => 120,
            (false, true) => 106,
            (false, false) => 161,
        };
        paint.set_color_rgba8(shade, shade, shade, 255);
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScrollbarDrag {
    pub pane: ScrollPane,
    grab: f64,
}

pub(super) enum ScrollbarPress {
    Drag(ScrollbarDrag),
    Page(f32),
}

#[derive(Default)]
pub(super) struct ScrollbarInteraction {
    pub drag: Option<ScrollbarDrag>,
    pub hovered: Option<ScrollPane>,
}

/// A track click scrolls by this share of the pane height, toward the click.
pub(super) const PAGE_SHARE: f32 = 0.9;

/// What a left press on a scrollbar asks the runtime to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PressOutcome {
    /// A thumb press started a drag.
    DragStarted,
    /// A track press pages `pane` toward the click (`direction` is -1 or 1).
    Page { pane: ScrollPane, direction: f32 },
}

/// How far one track press scrolls a pane of `height` pixels.
pub(super) fn page_pixels(direction: f32, height: u32) -> f32 {
    direction * height as f32 * PAGE_SHARE
}

/// What a pointer move over the scrollbars asks the runtime to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PointerOutcome {
    pub hover_changed: bool,
    pub drag: DragUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum DragUpdate {
    None,
    /// The drag follows the pointer live.
    ScrollTo {
        pane: ScrollPane,
        fraction: f64,
    },
    /// The dragged pane no longer has a bar (for example a stale Preview frame): the drag ended.
    Lost,
}

/// The bars a frame shows. `source` and `preview` are the panes that have content to scroll;
/// nothing is operable while the window does not accept edits.
pub(super) fn pane_bars(
    accepts_input: bool,
    dpi: f32,
    split: bool,
    source: Option<PaneScroll>,
    preview: Option<PaneScroll>,
) -> [Option<Scrollbar>; 2] {
    if !accepts_input {
        return [None, None];
    }
    let bar = |pane, scroll: PaneScroll, reserve_split_control| {
        Scrollbar::new(
            pane,
            scroll.content,
            dpi,
            scroll.fraction,
            scroll.visible,
            reserve_split_control,
        )
    };
    [
        source.and_then(|scroll| bar(ScrollPane::Source, scroll, split)),
        preview.and_then(|scroll| bar(ScrollPane::Preview, scroll, false)),
    ]
}

/// Where a pane is scrolled: `fraction` of the scrollable range, `visible` share of the content.
#[derive(Debug, Clone, Copy)]
pub(super) struct PaneScroll {
    pub content: PaneRect,
    pub fraction: f64,
    pub visible: f64,
}

/// The Preview pane's scroll, only for a frame laid out for the current document and viewport:
/// a stale frame offers no operable thumb (plan 09).
pub(super) fn preview_scroll(
    content: PaneRect,
    current: bool,
    document_height: f32,
    scroll_y: f32,
) -> Option<PaneScroll> {
    if !current {
        return None;
    }
    let height = f64::from(document_height);
    let viewport = f64::from(content.height);
    Some(PaneScroll {
        content,
        fraction: f64::from(scroll_y) / (height - viewport).max(1.0),
        visible: viewport / height.max(viewport),
    })
}

impl ScrollbarInteraction {
    /// Whether `pane`'s bar is drawn thick: hovered or being dragged.
    pub fn emphasized(&self, pane: ScrollPane) -> bool {
        self.hovered == Some(pane) || self.drag.is_some_and(|drag| drag.pane == pane)
    }

    /// A left press at `position`.
    pub fn press(
        &mut self,
        bars: [Option<Scrollbar>; 2],
        position: PhysicalPosition<f64>,
    ) -> Option<PressOutcome> {
        let bar = bars
            .into_iter()
            .flatten()
            .find(|bar| bar.contains(position))?;
        Some(match bar.press(position.y) {
            ScrollbarPress::Drag(drag) => {
                self.drag = Some(drag);
                PressOutcome::DragStarted
            }
            ScrollbarPress::Page(direction) => PressOutcome::Page {
                pane: bar.pane,
                direction,
            },
        })
    }

    /// Ends a drag (a left release, focus loss, a view switch, a resize); returns whether one
    /// was in progress.
    pub fn end_drag(&mut self) -> bool {
        self.drag.take().is_some()
    }

    /// The pointer moved to height `y` with `gutter` the gutter under it, over the visible `bars`.
    pub fn pointer_moved(
        &mut self,
        bars: [Option<Scrollbar>; 2],
        gutter: Option<ScrollPane>,
        y: f64,
    ) -> PointerOutcome {
        let hovered = bars
            .iter()
            .flatten()
            .find(|bar| gutter == Some(bar.pane))
            .map(|bar| bar.pane);
        let hover_changed = self.hovered != hovered;
        self.hovered = hovered;
        let drag = match self.drag {
            None => DragUpdate::None,
            Some(drag) => match bars.into_iter().flatten().find(|bar| bar.pane == drag.pane) {
                Some(bar) => DragUpdate::ScrollTo {
                    pane: drag.pane,
                    fraction: bar.fraction_at(drag, y),
                },
                None => {
                    self.drag = None;
                    DragUpdate::Lost
                }
            },
        };
        PointerOutcome {
            hover_changed,
            drag,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollbar_drag_keeps_grab_offset_and_clamps_outside_window() {
        let pane = PaneRect {
            x: 0,
            y: 34,
            width: 500,
            height: 646,
        };
        let bar = Scrollbar::new(ScrollPane::Source, pane, 1.0, 0.4, 0.001, false).unwrap();
        assert_eq!(bar.thumb_height, 24.0);
        let y = bar.thumb_top + 3.0;
        let ScrollbarPress::Drag(drag) = bar.press(y) else {
            panic!("thumb drag")
        };
        assert!((bar.fraction_at(drag, y) - 0.4).abs() < 1e-9);
        assert_eq!(bar.fraction_at(drag, -500.0), 0.0);
        assert_eq!(bar.fraction_at(drag, 5000.0), 1.0);
        assert!(matches!(
            bar.press(bar.thumb_top - 1.0),
            ScrollbarPress::Page(-1.0)
        ));
        assert!(matches!(
            bar.press(bar.thumb_top + bar.thumb_height),
            ScrollbarPress::Page(1.0)
        ));
        assert!(Scrollbar::new(ScrollPane::Source, pane, 1.0, 0.0, 1.0, false).is_none());
    }

    #[test]
    fn scrollbar_paints_inside_gutter_and_avoids_resize_border_at_each_dpi() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            let width = (520.0_f32 * dpi).round() as u32;
            let height = (680.0_f32 * dpi).round() as u32;
            let pane = PaneRect {
                x: 0,
                y: 0,
                width: width - (GUTTER_DIP * dpi).round() as u32,
                height,
            };
            let bar = Scrollbar::new(ScrollPane::Preview, pane, dpi, 1.0, 0.1, false).unwrap();
            let split_bar = Scrollbar::new(ScrollPane::Source, pane, dpi, 0.0, 0.1, true).unwrap();
            assert!(split_bar.track.y as f32 > pane.y as f32 + 26.0 * dpi);
            assert!(bar.track.x + bar.track.width < width - (6.0 * dpi) as u32);
            let mut thin = Pixmap::new(width, height).unwrap();
            let mut thick = thin.clone();
            bar.paint(&mut thin, false, false);
            bar.paint(&mut thick, true, false);
            let thin_count = thin.pixels().iter().filter(|p| p.alpha() > 0).count();
            let thick_count = thick.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(thick_count > thin_count);
            for y in 0..height {
                assert!(
                    thick.pixels()[(y * width) as usize..(y * width + pane.width) as usize]
                        .iter()
                        .all(|p| p.alpha() == 0)
                );
            }
        }
    }

    fn content() -> PaneRect {
        PaneRect {
            x: 0,
            y: 34,
            width: 500,
            height: 646,
        }
    }

    fn scroll(fraction: f64) -> PaneScroll {
        PaneScroll {
            content: content(),
            fraction,
            visible: 0.2,
        }
    }

    fn bars() -> [Option<Scrollbar>; 2] {
        pane_bars(true, 1.0, false, Some(scroll(0.5)), None)
    }

    fn thumb_center(bar: Scrollbar) -> PhysicalPosition<f64> {
        PhysicalPosition::new(
            f64::from(bar.track.x) + 1.0,
            bar.thumb_top + bar.thumb_height / 2.0,
        )
    }

    #[test]
    fn hovering_or_dragging_a_bar_emphasizes_only_that_bar() {
        let mut interaction = ScrollbarInteraction::default();
        let outcome = interaction.pointer_moved(bars(), Some(ScrollPane::Source), 100.0);
        assert!(outcome.hover_changed);
        assert!(interaction.emphasized(ScrollPane::Source));
        assert!(!interaction.emphasized(ScrollPane::Preview));
        // Leaving the gutter drops the hover emphasis.
        assert!(interaction.pointer_moved(bars(), None, 100.0).hover_changed);
        assert!(!interaction.emphasized(ScrollPane::Source));
        // A drag keeps its bar emphasized away from the gutter.
        let bar = bars()[0].unwrap();
        assert_eq!(
            interaction.press(bars(), thumb_center(bar)),
            Some(PressOutcome::DragStarted)
        );
        interaction.pointer_moved(bars(), None, 100.0);
        assert!(interaction.emphasized(ScrollPane::Source));
    }

    #[test]
    fn a_drag_follows_the_pointer_live_and_any_end_stops_it() {
        let mut interaction = ScrollbarInteraction::default();
        let bar = bars()[0].unwrap();
        interaction.press(bars(), thumb_center(bar));
        let mut fractions = Vec::new();
        for y in [100.0, 300.0, 5000.0] {
            match interaction.pointer_moved(bars(), None, y).drag {
                DragUpdate::ScrollTo { pane, fraction } => {
                    assert_eq!(pane, ScrollPane::Source);
                    fractions.push(fraction);
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(
            fractions[0] < fractions[1] && fractions[2] == 1.0,
            "{fractions:?}"
        );
        // A release (or focus loss, a view switch, a resize) ends it; later moves scroll nothing.
        assert!(interaction.end_drag());
        assert!(!interaction.end_drag());
        assert_eq!(
            interaction.pointer_moved(bars(), None, 300.0).drag,
            DragUpdate::None
        );
    }

    #[test]
    fn a_drag_whose_bar_vanishes_ends() {
        let mut interaction = ScrollbarInteraction::default();
        let bar = bars()[0].unwrap();
        interaction.press(bars(), thumb_center(bar));
        assert_eq!(
            interaction.pointer_moved([None, None], None, 300.0).drag,
            DragUpdate::Lost
        );
        assert!(interaction.drag.is_none());
    }

    #[test]
    fn a_track_press_pages_toward_the_click_by_most_of_the_pane() {
        let mut interaction = ScrollbarInteraction::default();
        let bar = bars()[0].unwrap();
        let x = f64::from(bar.track.x) + 1.0;
        assert_eq!(
            interaction.press(bars(), PhysicalPosition::new(x, bar.thumb_top - 1.0)),
            Some(PressOutcome::Page {
                pane: ScrollPane::Source,
                direction: -1.0
            })
        );
        assert_eq!(
            interaction.press(
                bars(),
                PhysicalPosition::new(x, bar.thumb_top + bar.thumb_height + 1.0)
            ),
            Some(PressOutcome::Page {
                pane: ScrollPane::Source,
                direction: 1.0
            })
        );
        assert_eq!(page_pixels(-1.0, 600), -540.0);
        assert_eq!(page_pixels(1.0, 600), 540.0);
        assert!(interaction.drag.is_none());
        // A press outside every bar is not a scrollbar press.
        assert_eq!(
            interaction.press(bars(), PhysicalPosition::new(10.0, 300.0)),
            None
        );
    }

    #[test]
    fn nothing_is_operable_without_input_or_with_a_stale_preview() {
        assert!(
            pane_bars(false, 1.0, false, Some(scroll(0.5)), Some(scroll(0.5)))
                .iter()
                .all(Option::is_none)
        );
        assert!(preview_scroll(content(), false, 5000.0, 100.0).is_none());
        let current = preview_scroll(content(), true, 5000.0, 100.0).unwrap();
        let [source, preview] = pane_bars(true, 1.0, true, Some(scroll(0.5)), Some(current));
        assert_eq!(source.unwrap().pane, ScrollPane::Source);
        assert_eq!(preview.unwrap().pane, ScrollPane::Preview);
        // Split reserves the sync control above the Source track only.
        let plain = pane_bars(true, 1.0, false, Some(scroll(0.5)), None)[0].unwrap();
        assert!(source.unwrap().track.y > plain.track.y);
        assert_eq!(preview.unwrap().track.y, plain.track.y);
    }
}
