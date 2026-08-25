//! Paints the fixed native toolbar and opacity popup from an immutable projection.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use tiny_skia::{Paint, Pixmap, Rect, Transform};

use super::controls::{ControlId, ControlLayout, ControlRect, split_sync_rect};
use super::preview_runtime::ViewGeometry;
use crate::config::ViewMode;

#[derive(Clone, Copy)]
pub(super) struct ToolbarVisual {
    pub(super) mode: ViewMode,
    pub(super) topmost: bool,
    pub(super) dark: bool,
    pub(super) system_theme: bool,
    pub(super) diagnostic: bool,
    pub(super) emphasized: bool,
    pub(super) opacity_popup: bool,
    pub(super) opacity: u8,
    pub(super) split_scroll_sync: bool,
}

pub(super) fn paint_toolbar(
    pixmap: &mut Pixmap,
    geometry: ViewGeometry,
    layout: &ControlLayout,
    visual: ToolbarVisual,
) {
    let scale = (geometry.toolbar_height as f32 / 34.0).max(0.5);
    let toolbar = if visual.dark {
        (43, 43, 40, 255)
    } else {
        (238, 235, 226, 255)
    };
    fill_rect(
        pixmap,
        0.0,
        0.0,
        pixmap.width() as f32,
        geometry.toolbar_height as f32,
        toolbar,
    );
    let ink = if visual.dark {
        (226, 223, 214, if visual.emphasized { 255 } else { 150 })
    } else {
        (64, 63, 59, if visual.emphasized { 255 } else { 135 })
    };
    let selected = if visual.dark {
        (73, 76, 79, 255)
    } else {
        (210, 215, 218, 255)
    };
    for (id, active) in [
        (ControlId::Source, visual.mode == ViewMode::Source),
        (ControlId::Split, visual.mode == ViewMode::Split),
        (ControlId::Preview, visual.mode == ViewMode::Preview),
        (ControlId::ConvertMath, false),
        (ControlId::Topmost, visual.topmost),
        (ControlId::Theme, visual.system_theme),
        (ControlId::Opacity, visual.opacity_popup),
        (ControlId::Collapse, false),
        (ControlId::Close, false),
    ] {
        let rect = layout.rect(id);
        if active {
            fill_control_rect(pixmap, rect, selected);
        }
        paint_control_icon(pixmap, id, rect, ink, visual);
    }
    if visual.diagnostic {
        fill_rect(
            pixmap,
            (pixmap.width() as f32 - 10.0 * scale).max(0.0),
            7.0 * scale,
            4.0 * scale,
            20.0 * scale,
            (190, 116, 35, 255),
        );
    }
    if let Some(divider_x) = geometry.divider_x {
        fill_rect(
            pixmap,
            divider_x as f32,
            geometry.toolbar_height as f32,
            1.0f32.max(scale),
            (pixmap.height().saturating_sub(geometry.toolbar_height)) as f32,
            if visual.dark {
                (84, 82, 76, 255)
            } else {
                (190, 186, 175, 255)
            },
        );
        paint_split_sync(
            pixmap,
            split_sync_rect(divider_x, geometry.toolbar_height, f64::from(scale)),
            visual.split_scroll_sync,
            visual.dark,
            ink,
        );
    }
    if visual.opacity_popup {
        paint_opacity_popup(pixmap, layout, visual);
    }
}

fn fill_control_rect(pixmap: &mut Pixmap, rect: ControlRect, color: (u8, u8, u8, u8)) {
    fill_rect(
        pixmap,
        rect.x as f32,
        rect.y as f32,
        rect.width as f32,
        rect.height as f32,
        color,
    );
}

fn paint_control_icon(
    pixmap: &mut Pixmap,
    id: ControlId,
    rect: ControlRect,
    ink: (u8, u8, u8, u8),
    visual: ToolbarVisual,
) {
    let scale = (rect.height as f32 / 28.0).max(0.5);
    let x = rect.x as f32 + 5.0 * scale;
    let y = rect.y as f32 + 7.0 * scale;
    match id {
        ControlId::Source => {
            for row in 0..3 {
                fill_rect(
                    pixmap,
                    x,
                    y + row as f32 * 5.0 * scale,
                    (18.0 - row as f32 * 2.0) * scale,
                    1.5 * scale,
                    ink,
                );
            }
        }
        ControlId::Split => {
            fill_rect(pixmap, x, y, 8.0 * scale, 13.0 * scale, ink);
            fill_rect(pixmap, x + 10.0 * scale, y, 8.0 * scale, 13.0 * scale, ink);
        }
        ControlId::Preview => {
            fill_rect(pixmap, x, y, 18.0 * scale, 2.0 * scale, ink);
            fill_rect(
                pixmap,
                x + 3.0 * scale,
                y + 5.0 * scale,
                12.0 * scale,
                1.5 * scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 3.0 * scale,
                y + 10.0 * scale,
                10.0 * scale,
                1.5 * scale,
                ink,
            );
        }
        ControlId::ConvertMath => {
            // Pixel label `\(->$`: both source delimiter and target are
            // visible instead of relying on an ambiguous abstract icon.
            for offset in 0..5 {
                fill_rect(
                    pixmap,
                    x + offset as f32 * 0.55 * scale,
                    y + offset as f32 * 2.4 * scale,
                    scale.max(1.0),
                    scale.max(1.0),
                    ink,
                );
            }
            fill_rect(
                pixmap,
                x + 4.0 * scale,
                y + 2.0 * scale,
                scale,
                10.0 * scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 5.0 * scale,
                y + 1.0 * scale,
                2.0 * scale,
                scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 5.0 * scale,
                y + 12.0 * scale,
                2.0 * scale,
                scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 8.0 * scale,
                y + 7.0 * scale,
                5.0 * scale,
                scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 11.0 * scale,
                y + 5.0 * scale,
                scale,
                5.0 * scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 15.0 * scale,
                y + 2.0 * scale,
                3.0 * scale,
                scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 14.0 * scale,
                y + 6.0 * scale,
                4.0 * scale,
                scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 14.0 * scale,
                y + 10.0 * scale,
                3.0 * scale,
                scale,
                ink,
            );
            fill_rect(pixmap, x + 16.0 * scale, y, scale, 13.0 * scale, ink);
        }
        ControlId::Topmost => {
            fill_rect(pixmap, x + 4.0 * scale, y, 10.0 * scale, 2.0 * scale, ink);
            fill_rect(
                pixmap,
                x + 7.5 * scale,
                y + 2.0 * scale,
                3.0 * scale,
                8.0 * scale,
                ink,
            );
            fill_rect(
                pixmap,
                x + 8.4 * scale,
                y + 9.0 * scale,
                1.2 * scale,
                7.0 * scale,
                ink,
            );
        }
        ControlId::Theme => {
            let accent = if visual.system_theme {
                (112, 155, 210, ink.3)
            } else {
                ink
            };
            fill_rect(
                pixmap,
                x + 4.0 * scale,
                y + 3.0 * scale,
                10.0 * scale,
                10.0 * scale,
                accent,
            );
            if visual.dark {
                fill_rect(
                    pixmap,
                    x + 9.0 * scale,
                    y + 1.0 * scale,
                    8.0 * scale,
                    8.0 * scale,
                    (43, 43, 40, 255),
                );
            }
        }
        ControlId::Opacity => {
            fill_rect(
                pixmap,
                x,
                y + 2.0 * scale,
                18.0 * scale,
                13.0 * scale,
                (ink.0, ink.1, ink.2, 60),
            );
            fill_rect(
                pixmap,
                x,
                y + 2.0 * scale,
                18.0 * scale * f32::from(visual.opacity) / 100.0,
                13.0 * scale,
                ink,
            );
        }
        ControlId::Collapse => {
            fill_rect(
                pixmap,
                x + 2.0 * scale,
                y + 7.0 * scale,
                14.0 * scale,
                2.0 * scale,
                ink,
            );
        }
        ControlId::Close => {
            for offset in 0..10 {
                let offset = offset as f32 * scale;
                fill_rect(
                    pixmap,
                    x + 4.0 * scale + offset,
                    y + 3.0 * scale + offset,
                    scale.max(1.0),
                    scale.max(1.0),
                    ink,
                );
                fill_rect(
                    pixmap,
                    x + 13.0 * scale - offset,
                    y + 3.0 * scale + offset,
                    scale.max(1.0),
                    scale.max(1.0),
                    ink,
                );
            }
        }
    }
}

fn paint_split_sync(
    pixmap: &mut Pixmap,
    rect: ControlRect,
    active: bool,
    dark: bool,
    ink: (u8, u8, u8, u8),
) {
    fill_control_rect(
        pixmap,
        rect,
        if active {
            if dark {
                (73, 76, 79, 235)
            } else {
                (210, 215, 218, 235)
            }
        } else if dark {
            (43, 43, 40, 220)
        } else {
            (238, 235, 226, 220)
        },
    );
    let scale = (rect.height as f32 / 22.0).max(0.5);
    let x = rect.x as f32 + 4.0 * scale;
    let y = rect.y as f32 + 6.0 * scale;
    fill_rect(pixmap, x, y, 10.0 * scale, scale, ink);
    fill_rect(pixmap, x, y + 8.0 * scale, 10.0 * scale, scale, ink);
    fill_rect(pixmap, x, y, scale, 4.0 * scale, ink);
    fill_rect(
        pixmap,
        x + 9.0 * scale,
        y + 5.0 * scale,
        scale,
        4.0 * scale,
        ink,
    );
}

fn paint_opacity_popup(pixmap: &mut Pixmap, layout: &ControlLayout, visual: ToolbarVisual) {
    let background = if visual.dark {
        (54, 54, 50, 255)
    } else {
        (249, 247, 240, 255)
    };
    let track = if visual.dark {
        (98, 96, 88, 255)
    } else {
        (190, 186, 175, 255)
    };
    let accent = if visual.dark {
        (129, 179, 236, 255)
    } else {
        (47, 104, 166, 255)
    };
    fill_control_rect(pixmap, layout.opacity_popup, background);
    let slider = layout.opacity_slider;
    fill_rect(
        pixmap,
        slider.x as f32,
        (slider.y + slider.height / 2.0 - 1.5) as f32,
        slider.width as f32,
        3.0,
        track,
    );
    let ratio = f64::from(visual.opacity.saturating_sub(40)) / 60.0;
    let thumb_x = slider.x + ratio * slider.width;
    fill_rect(
        pixmap,
        (thumb_x - 4.0) as f32,
        slider.y as f32,
        8.0,
        slider.height as f32,
        accent,
    );
    fill_control_rect(pixmap, layout.opacity_input, track);
    paint_number(
        pixmap,
        visual.opacity,
        layout.opacity_input.x as f32 + 5.0,
        layout.opacity_input.y as f32 + 5.0,
        if visual.dark {
            (240, 237, 228, 255)
        } else {
            (45, 43, 39, 255)
        },
    );
}

fn paint_number(pixmap: &mut Pixmap, value: u8, mut x: f32, y: f32, color: (u8, u8, u8, u8)) {
    let digits = if value >= 100 {
        [Some(value / 100), Some((value / 10) % 10), Some(value % 10)]
    } else if value >= 10 {
        [None, Some(value / 10), Some(value % 10)]
    } else {
        [None, None, Some(value)]
    };
    for digit in digits.into_iter().flatten() {
        paint_digit(pixmap, digit, x, y, color);
        x += 9.0;
    }
}

fn paint_digit(pixmap: &mut Pixmap, digit: u8, x: f32, y: f32, color: (u8, u8, u8, u8)) {
    const MASKS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    let mask = MASKS.get(digit as usize).copied().unwrap_or(0);
    let segments = [
        (x + 1.0, y, 5.0, 1.5),
        (x + 6.0, y + 1.0, 1.5, 5.0),
        (x + 6.0, y + 7.0, 1.5, 5.0),
        (x + 1.0, y + 12.0, 5.0, 1.5),
        (x, y + 7.0, 1.5, 5.0),
        (x, y + 1.0, 1.5, 5.0),
        (x + 1.0, y + 6.0, 5.0, 1.5),
    ];
    for (index, (sx, sy, width, height)) in segments.into_iter().enumerate() {
        if mask & (1 << index) != 0 {
            fill_rect(pixmap, sx, sy, width, height, color);
        }
    }
}

fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: (u8, u8, u8, u8),
) {
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, color.3);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}
