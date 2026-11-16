//! The notice bar: one line across the bottom of the window, over every pane, with the buttons
//! of a waiting decision. It is laid out in window DIP and does not follow Content Zoom; it takes
//! no part in the panes' layout or in their redraw keys. What it shows comes from the session's
//! notice view; the bar decides nothing about it.
//!
//! plan_ref: docs/plan/09_windows_shell.md#notices

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalSize;

use stickymd_app::flow::notice::{Choice, Choices, NoticeTone, NoticeView};
use stickymd_render::source::{SourceProjection, UiTextSpec};

use crate::geometry::GUTTER_DIP;

/// The bar's height in window DIP (plan 09 #notices): the same as the toolbar.
pub const NOTICE_BAR_HEIGHT_DIP: f32 = 34.0;
/// Where the buttons start, as a share of the window's width.
const ACTIONS_START: f32 = 0.58;
const TEXT_INSET_DIP: f32 = 12.0;
const TEXT_TOP_DIP: f32 = 7.0;
const BUTTON_INSET_DIP: f32 = 1.0;
const LABEL_INSET_DIP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoticeRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl NoticeRect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        let (x, y) = (x as f32, y as f32);
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    /// Whether the two rectangles share any pixel.
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// Where the bar, its text and its buttons go in one window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoticeBarLayout {
    pub bar: NoticeRect,
    pub text: NoticeRect,
    /// F6 in the left slot, F7 in the right one; an option that is not offered has no button.
    pub buttons: [Option<(Choice, NoticeRect)>; 2],
    scale: f32,
}

impl NoticeBarLayout {
    /// The bar of a window of `size` at `scale` DPI, with a button per offered choice.
    pub fn new(size: PhysicalSize<u32>, scale: f32, choices: Option<Choices>) -> Self {
        let scale = scale.max(0.5);
        let width = size.width as f32;
        let height = (NOTICE_BAR_HEIGHT_DIP * scale).min(size.height as f32);
        let bar = NoticeRect {
            x: 0.0,
            y: size.height as f32 - height,
            width,
            height,
        };
        // The buttons end before the rightmost pane's scrollbar gutter, which stays the scrollbar's.
        let actions_end = (width - (GUTTER_DIP * scale).round()).max(0.0);
        let actions_start = (width * ACTIONS_START).min(actions_end);
        let slot = (actions_end - actions_start) * 0.5;
        let inset = BUTTON_INSET_DIP * scale;
        let button = |index: usize, choice: Option<Choice>| {
            choice.map(|choice| {
                (
                    choice,
                    NoticeRect {
                        x: actions_start + index as f32 * slot,
                        y: bar.y + inset,
                        width: (slot - inset).max(0.0),
                        height: (height - 2.0 * inset).max(0.0),
                    },
                )
            })
        };
        let buttons = match choices {
            Some(choices) => [button(0, choices.primary), button(1, choices.secondary)],
            None => [None, None],
        };
        let text_right = if buttons.iter().any(Option::is_some) {
            actions_start
        } else {
            actions_end
        };
        let text_x = TEXT_INSET_DIP * scale;
        Self {
            bar,
            text: NoticeRect {
                x: text_x,
                y: bar.y + TEXT_TOP_DIP * scale,
                width: (text_right - text_x - TEXT_INSET_DIP * scale).max(1.0),
                height: height - TEXT_TOP_DIP * scale,
            },
            buttons,
            scale,
        }
    }

    /// The choice whose button is under (`x`, `y`).
    pub fn choice_at(&self, x: f64, y: f64) -> Option<Choice> {
        self.buttons
            .iter()
            .flatten()
            .find_map(|(choice, rect)| rect.contains(x, y).then_some(*choice))
    }

    fn text_spec(&self, rect: NoticeRect) -> UiTextSpec {
        UiTextSpec {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            scale: self.scale,
        }
    }
}

/// The colours of one tone, light or dark.
struct ToneColors {
    background: (u8, u8, u8, u8),
    primary: (u8, u8, u8, u8),
    secondary: (u8, u8, u8, u8),
    text: (u8, u8, u8),
}

fn tone_colors(tone: NoticeTone, dark: bool) -> ToneColors {
    // Waiting decisions and reminders keep the colours of the status line they replace.
    let amber = if dark {
        ToneColors {
            background: (78, 62, 37, 248),
            primary: (102, 77, 38, 248),
            secondary: (91, 69, 36, 248),
            text: (245, 224, 184),
        }
    } else {
        ToneColors {
            background: (246, 224, 184, 245),
            primary: (232, 201, 145, 245),
            secondary: (239, 211, 163, 245),
            text: (72, 52, 20),
        }
    };
    let (background, text) = match (tone, dark) {
        (NoticeTone::Pending | NoticeTone::Reminder, _) => return amber,
        (NoticeTone::Important, false) => ((240, 205, 170, 245), (80, 40, 15)),
        (NoticeTone::Important, true) => ((92, 58, 34, 248), (250, 220, 190)),
        (NoticeTone::Fault, false) => ((244, 204, 196, 245), (110, 30, 20)),
        (NoticeTone::Fault, true) => ((96, 44, 38, 248), (250, 210, 200)),
        (NoticeTone::Success, false) => ((214, 234, 210, 245), (30, 70, 35)),
        (NoticeTone::Success, true) => ((44, 70, 48, 248), (210, 240, 210)),
        (NoticeTone::InProgress, false) => ((228, 226, 220, 245), (60, 58, 54)),
        (NoticeTone::InProgress, true) => ((58, 57, 54, 248), (220, 218, 212)),
    };
    ToneColors {
        background,
        text,
        ..amber
    }
}

/// Paints `view` into `layout` over whatever the window already holds.
pub fn paint_notice_bar(
    pixmap: &mut Pixmap,
    projection: &mut Option<SourceProjection>,
    layout: &NoticeBarLayout,
    view: &NoticeView<'_>,
    dark: bool,
) {
    let colors = tone_colors(view.tone, dark);
    fill(pixmap, layout.bar, colors.background);
    for (index, (_, rect)) in layout
        .buttons
        .iter()
        .enumerate()
        .filter_map(|(index, button)| button.map(|button| (index, button)))
    {
        fill(
            pixmap,
            rect,
            if index == 0 {
                colors.primary
            } else {
                colors.secondary
            },
        );
    }
    let Some(projection) = projection else {
        return;
    };
    projection.paint_ui_text_in(
        pixmap,
        &view.text,
        layout.text_spec(layout.text),
        colors.text,
    );
    for (choice, rect) in layout.buttons.iter().flatten() {
        let inset = LABEL_INSET_DIP * layout.scale;
        let label = NoticeRect {
            x: rect.x + inset,
            y: layout.text.y,
            width: (rect.width - 2.0 * inset).max(1.0),
            height: layout.text.height,
        };
        projection.paint_ui_text_in(pixmap, choice.label(), layout.text_spec(label), colors.text);
    }
}

fn fill(pixmap: &mut Pixmap, rect: NoticeRect, color: (u8, u8, u8, u8)) {
    let Some(rect) = Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, color.3);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

#[cfg(test)]
mod tests;
