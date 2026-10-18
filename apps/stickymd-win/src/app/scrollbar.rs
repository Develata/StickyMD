//! Shared scrollbar geometry and gesture state for both document panes.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalPosition;

use super::preview_runtime::PaneRect;

pub(super) const GUTTER_DIP: f32 = 20.0;

/// The pane whose gutter is under `cursor`: the band `GUTTER_DIP` DIP wide at `dpi` just right
/// of the pane, over the pane's height (plan 09 vertical scrollbars). Panes are tried in order.
pub(super) fn gutter_at(
    cursor: PhysicalPosition<f64>,
    panes: [(ScrollPane, Option<PaneRect>); 2],
    dpi: f64,
) -> Option<ScrollPane> {
    panes.into_iter().find_map(|(side, pane)| {
        let pane = pane?;
        let right = f64::from(pane.x + pane.width);
        (cursor.x >= right
            && cursor.x < right + f64::from(GUTTER_DIP) * dpi
            && cursor.y >= f64::from(pane.y)
            && cursor.y < f64::from(pane.y + pane.height))
        .then_some(side)
    })
}

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
        // Inside the track the grabbed point stays under the pointer: moving the pointer by `dy`
        // moves the thumb top by `dy`, so the fraction is that top over the free track length.
        let tall = Scrollbar::new(ScrollPane::Source, pane, 1.0, 0.4, 0.2, false).unwrap();
        let free = f64::from(tall.track.height) - tall.thumb_height;
        for grab in [3.0, tall.thumb_height - 2.0] {
            let press = tall.thumb_top + grab;
            let ScrollbarPress::Drag(drag) = tall.press(press) else {
                panic!("thumb drag at {grab}")
            };
            for dy in [-97.3, -41.0, 17.6, 113.9] {
                let top = tall.thumb_top + dy;
                let expected = (top - f64::from(tall.track.y)) / free;
                assert!((0.0..=1.0).contains(&expected), "{grab} {dy}");
                let got = tall.fraction_at(drag, press + dy);
                assert!(
                    (got - expected).abs() < 1e-9,
                    "{grab} {dy}: {got} vs {expected}"
                );
            }
        }
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
            // A thin thumb is always shown; hover or drag thickens it: wider over the same rows,
            // inside the track.
            let thin_span = painted_span(&thin).expect("the resting thumb is painted");
            let thick_span = painted_span(&thick).expect("the emphasized thumb is painted");
            assert_eq!(thick_span.rows, thin_span.rows, "dpi {dpi}");
            assert!(
                thick_span.columns.0 < thin_span.columns.0
                    && thick_span.columns.1 > thin_span.columns.1,
                "dpi {dpi}: {thin_span:?} -> {thick_span:?}"
            );
            assert!(
                thick_span.columns.0 >= bar.track.x
                    && thick_span.columns.1 < bar.track.x + bar.track.width,
                "dpi {dpi}: {thick_span:?} inside {:?}",
                bar.track
            );
            for y in 0..height {
                assert!(
                    thick.pixels()[(y * width) as usize..(y * width + pane.width) as usize]
                        .iter()
                        .all(|p| p.alpha() == 0)
                );
            }
        }
    }

    /// The columns and rows, first and last inclusive, that hold painted pixels.
    #[derive(Debug, PartialEq)]
    struct Span {
        columns: (u32, u32),
        rows: (u32, u32),
    }

    fn painted_span(pixmap: &Pixmap) -> Option<Span> {
        let width = pixmap.width();
        let mut span: Option<Span> = None;
        for (index, pixel) in pixmap.pixels().iter().enumerate() {
            if pixel.alpha() == 0 {
                continue;
            }
            let (x, y) = (index as u32 % width, index as u32 / width);
            let next = span.get_or_insert(Span {
                columns: (x, x),
                rows: (y, y),
            });
            next.columns = (next.columns.0.min(x), next.columns.1.max(x));
            next.rows = (next.rows.0.min(y), next.rows.1.max(y));
        }
        span
    }

    fn content() -> PaneRect {
        PaneRect {
            x: 0,
            y: 34,
            width: 500,
            height: 646,
        }
    }

    /// A Split: the Source bar and, right of it, a current Preview bar.
    fn split_bars() -> [Option<Scrollbar>; 2] {
        let preview_pane = PaneRect {
            x: 520,
            ..content()
        };
        pane_bars(
            true,
            1.0,
            true,
            Some(scroll(0.5)),
            preview_scroll(preview_pane, true, 5000.0, 2500.0),
        )
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

    /// In a Split, for each pane in turn: hovering its gutter, or dragging its thumb away from
    /// any gutter, emphasizes that bar and never the other.
    #[test]
    fn hovering_or_dragging_a_bar_emphasizes_only_that_bar() {
        let bars = split_bars();
        for (index, pane, other) in [
            (0, ScrollPane::Source, ScrollPane::Preview),
            (1, ScrollPane::Preview, ScrollPane::Source),
        ] {
            let mut interaction = ScrollbarInteraction::default();
            let outcome = interaction.pointer_moved(bars, Some(pane), 100.0);
            assert!(outcome.hover_changed);
            assert!(interaction.emphasized(pane), "{pane:?} hovered");
            assert!(!interaction.emphasized(other), "{other:?} beside a hover");
            // Leaving the gutter drops the hover emphasis.
            assert!(interaction.pointer_moved(bars, None, 100.0).hover_changed);
            assert!(!interaction.emphasized(pane) && !interaction.emphasized(other));
            // A drag keeps its own bar, and only it, emphasized away from the gutter.
            let bar = bars[index].unwrap();
            assert_eq!(
                interaction.press(bars, thumb_center(bar)),
                Some(PressOutcome::DragStarted)
            );
            interaction.pointer_moved(bars, None, 100.0);
            assert!(interaction.emphasized(pane), "{pane:?} dragged");
            assert!(!interaction.emphasized(other), "{other:?} beside a drag");
            // Hovering the other gutter mid-drag emphasizes both; ending the drag keeps the hover.
            interaction.pointer_moved(bars, Some(other), 100.0);
            assert!(
                interaction.emphasized(pane),
                "{pane:?} dragged, {other:?} hovered"
            );
            assert!(interaction.emphasized(other), "{other:?} hovered mid-drag");
            assert!(interaction.end_drag());
            assert!(!interaction.emphasized(pane), "{pane:?} after the drag");
            assert!(interaction.emphasized(other), "{other:?} still hovered");
        }
        // Every state: no, either or both bars hovered (at most one) and dragged (at most one).
        // A bar is emphasized exactly when it is hovered or dragged (plan 09).
        let panes = [None, Some(ScrollPane::Source), Some(ScrollPane::Preview)];
        for hovered in panes {
            for dragged in panes {
                let mut interaction = ScrollbarInteraction::default();
                if let Some(dragged) = dragged {
                    let bar = bars[usize::from(dragged == ScrollPane::Preview)].unwrap();
                    assert_eq!(
                        interaction.press(bars, thumb_center(bar)),
                        Some(PressOutcome::DragStarted)
                    );
                }
                interaction.pointer_moved(bars, hovered, 100.0);
                for pane in [ScrollPane::Source, ScrollPane::Preview] {
                    assert_eq!(
                        interaction.emphasized(pane),
                        hovered == Some(pane) || dragged == Some(pane),
                        "{pane:?} with hovered {hovered:?} and dragged {dragged:?}"
                    );
                }
            }
        }
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
        // In Split each pane has its bar; a press on the Preview track pages the Preview pane.
        let split = split_bars();
        let preview = split[1].unwrap();
        let px = f64::from(preview.track.x) + 1.0;
        assert_eq!(
            interaction.press(split, PhysicalPosition::new(px, preview.thumb_top - 1.0)),
            Some(PressOutcome::Page {
                pane: ScrollPane::Preview,
                direction: -1.0
            })
        );
        assert_eq!(
            interaction.press(
                split,
                PhysicalPosition::new(px, preview.thumb_top + preview.thumb_height + 1.0)
            ),
            Some(PressOutcome::Page {
                pane: ScrollPane::Preview,
                direction: 1.0
            })
        );
        assert!(interaction.drag.is_none());
        // A press outside every bar is not a scrollbar press.
        assert_eq!(
            interaction.press(bars(), PhysicalPosition::new(10.0, 300.0)),
            None
        );
        // Each track is the half-open pixel rectangle `[left, right) x [top, bottom)`: on every
        // edge the last pixel inside pages the bar's pane, the first pixel outside (gutter
        // included) hits nothing. Edge presses on the left and right lie above the thumb.
        for bar in split.into_iter().flatten() {
            let track = bar.track;
            let (left, top) = (f64::from(track.x), f64::from(track.y));
            let (right, bottom) = (left + f64::from(track.width), top + f64::from(track.height));
            let middle_x = (left + right) / 2.0;
            assert!(top + 1.0 < bar.thumb_top && bar.thumb_top + bar.thumb_height < bottom - 1.0);
            let page = |direction| {
                Some(PressOutcome::Page {
                    pane: bar.pane,
                    direction,
                })
            };
            for (inside, outside, expected) in [
                ((left, top + 1.0), (left - 1.0, top + 1.0), page(-1.0)),
                ((right - 1.0, top + 1.0), (right, top + 1.0), page(-1.0)),
                ((middle_x, top), (middle_x, top - 1.0), page(-1.0)),
                ((middle_x, bottom - 1.0), (middle_x, bottom), page(1.0)),
            ] {
                let inside = PhysicalPosition::new(inside.0, inside.1);
                let outside = PhysicalPosition::new(outside.0, outside.1);
                assert_eq!(interaction.press(split, inside), expected, "{inside:?}");
                assert_eq!(interaction.press(split, outside), None, "{outside:?}");
            }
            // The thumb is the half-open span `[thumb_top, thumb_top + thumb_height)`: its first
            // and last rows start a drag, the track just above and at its end pages.
            let thumb_end = bar.thumb_top + bar.thumb_height;
            for (y, expected) in [
                (bar.thumb_top - 0.5, page(-1.0)),
                (bar.thumb_top, Some(PressOutcome::DragStarted)),
                (thumb_end - 0.5, Some(PressOutcome::DragStarted)),
                (thumb_end, page(1.0)),
            ] {
                let position = PhysicalPosition::new(middle_x, y);
                assert_eq!(interaction.press(split, position), expected, "{position:?}");
                assert_eq!(
                    interaction.end_drag(),
                    expected == Some(PressOutcome::DragStarted)
                );
            }
        }
        // From here every press is checked against `classify`, which knows only the half-open
        // track `[left, right) x [top, bottom)` and the stored thumb `[thumb_top, thumb_top +
        // thumb_height)`, and every press is followed by `end_drag`, which must report exactly
        // the presses that started a drag.
        fn classify(bar: Scrollbar, x: f64, y: f64) -> Option<PressOutcome> {
            let track = bar.track;
            let (left, top) = (f64::from(track.x), f64::from(track.y));
            let (right, bottom) = (left + f64::from(track.width), top + f64::from(track.height));
            let page = |direction| {
                Some(PressOutcome::Page {
                    pane: bar.pane,
                    direction,
                })
            };
            if !(left..right).contains(&x) || !(top..bottom).contains(&y) {
                None
            } else if y < bar.thumb_top {
                page(-1.0)
            } else if y < bar.thumb_top + bar.thumb_height {
                Some(PressOutcome::DragStarted)
            } else {
                page(1.0)
            }
        }
        let mut check = |bar: Scrollbar, x: f64, y: f64| {
            let position = PhysicalPosition::new(x, y);
            let want = classify(bar, x, y);
            assert_eq!(
                interaction.press([Some(bar), None], position),
                want,
                "{bar:?} at {position:?}"
            );
            assert_eq!(
                interaction.end_drag(),
                want == Some(PressOutcome::DragStarted),
                "{bar:?} at {position:?}"
            );
        };
        let quarters = |from: f64, to: f64| {
            (0..)
                .map(move |step| from + f64::from(step) * 0.25)
                .take_while(move |value| *value <= to)
        };
        // Every quarter pixel down the middle of each track and across it at three heights, two
        // pixels beyond every edge, for the Split bars and for bars whose thumb starts and ends
        // between pixels, or ends on a pixel although its top and height do not.
        let mut swept: Vec<Scrollbar> = split.into_iter().flatten().collect();
        for (fraction, visible) in [(0.0, 0.2), (0.3137, 0.2), (0.6421, 0.0333), (1.0, 0.0713)] {
            swept.push(
                Scrollbar::new(ScrollPane::Source, content(), 1.0, fraction, visible, false)
                    .unwrap(),
            );
        }
        let preview_pane = PaneRect {
            x: 520,
            ..content()
        };
        let exact_end = pane_bars(
            true,
            1.0,
            true,
            None,
            preview_scroll(preview_pane, true, 780.0, 4.0),
        )[1]
        .unwrap();
        assert_eq!(exact_end.thumb_top + exact_end.thumb_height, 567.0);
        assert_ne!(exact_end.thumb_top.fract(), 0.0);
        swept.push(exact_end);
        assert!(swept.iter().any(|bar| bar.thumb_top.fract() != 0.0));
        assert!(
            swept
                .iter()
                .any(|bar| (bar.thumb_top + bar.thumb_height).fract() != 0.0)
        );
        for bar in swept {
            let track = bar.track;
            let (left, top) = (f64::from(track.x), f64::from(track.y));
            let (right, bottom) = (left + f64::from(track.width), top + f64::from(track.height));
            for y in quarters(top - 2.0, bottom + 2.0) {
                check(bar, (left + right) / 2.0, y);
            }
            for y in [
                top + 0.25,
                bar.thumb_top + bar.thumb_height / 2.0,
                bottom - 0.25,
            ] {
                for x in quarters(left - 2.0, right + 2.0) {
                    check(bar, x, y);
                }
            }
        }
        // A grid of DPIs (the track's width and insets scale with it), with and without the Split
        // control's inset, thumb sizes and positions. Across each track at every quarter pixel two
        // pixels beyond both sides, through the thumb; down the middle at every quarter pixel
        // within two pixels of the track's ends and of the thumb's; and at the thumb's stored
        // boundaries themselves with the float just below each, so a classification that rounds
        // differently (say, comparing `y - thumb_top` with the height) fails. The float below a
        // thumb boundary that coincides with a track end is left to the whole- and
        // quarter-pixel presses there: the hit test reads the pointer as `f32`, so a hair
        // outside the track rounds onto it.
        for dpi in [1.0_f32, 1.25, 1.5, 2.0] {
            for split_control in [false, true] {
                for visible in [0.02, 0.1, 0.37, 0.8] {
                    for step in 0..=40 {
                        let fraction = f64::from(step) / 40.0;
                        let bar = Scrollbar::new(
                            ScrollPane::Preview,
                            content(),
                            dpi,
                            fraction,
                            visible,
                            split_control,
                        )
                        .unwrap();
                        let track = bar.track;
                        let (left, top) = (f64::from(track.x), f64::from(track.y));
                        let right = left + f64::from(track.width);
                        let bottom = top + f64::from(track.height);
                        let (thumb_top, thumb_end) =
                            (bar.thumb_top, bar.thumb_top + bar.thumb_height);
                        let middle = (left + right) / 2.0;
                        for x in quarters(left - 2.0, right + 2.0) {
                            check(bar, x, (thumb_top + thumb_end) / 2.0);
                        }
                        for edge in [top, thumb_top, thumb_end, bottom] {
                            for y in quarters(edge - 2.0, edge + 2.0) {
                                check(bar, middle, y);
                            }
                        }
                        check(bar, middle, thumb_top);
                        if thumb_top > top {
                            check(bar, middle, thumb_top.next_down());
                        }
                        if thumb_end < bottom {
                            check(bar, middle, thumb_end.next_down());
                            check(bar, middle, thumb_end);
                        }
                    }
                }
            }
        }
        assert!(interaction.drag.is_none());
    }

    /// The gutter is the `GUTTER_DIP` band right of each pane over the pane's height, scaled
    /// by DPI; outside it, or beside a missing pane, there is none.
    #[test]
    fn the_gutter_is_the_band_right_of_each_pane() {
        let source = content();
        let preview = PaneRect {
            x: 540,
            ..content()
        };
        for dpi in [1.0, 1.5] {
            let band = f64::from(GUTTER_DIP) * dpi;
            let panes = [
                (ScrollPane::Source, Some(source)),
                (ScrollPane::Preview, Some(preview)),
            ];
            let at = |x: f64, y: f64| gutter_at(PhysicalPosition::new(x, y), panes, dpi);
            // The band is `[right, right + band) x [top, bottom)`: just inside each edge it is
            // that pane's gutter; just outside (the pane itself, past the band, above or below
            // the pane) it is none.
            for (side, pane) in panes {
                let pane = pane.unwrap();
                let right = f64::from(pane.x + pane.width);
                let (top, bottom) = (f64::from(pane.y), f64::from(pane.y + pane.height));
                let (middle_x, middle_y) = (right + band / 2.0, (top + bottom) / 2.0);
                for (inside, outside) in [
                    ((right, middle_y), (right - 0.5, middle_y)),
                    ((right + band - 0.5, middle_y), (right + band, middle_y)),
                    ((middle_x, top), (middle_x, top - 0.5)),
                    ((middle_x, bottom - 0.5), (middle_x, bottom)),
                ] {
                    assert_eq!(at(inside.0, inside.1), Some(side), "{side:?} {inside:?}");
                    assert_eq!(at(outside.0, outside.1), None, "{side:?} {outside:?}");
                }
            }
        }
        let single = [
            (ScrollPane::Source, None),
            (ScrollPane::Preview, Some(preview)),
        ];
        let x = f64::from(source.x + source.width) + 1.0;
        assert_eq!(
            gutter_at(PhysicalPosition::new(x, 100.0), single, 1.0),
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
