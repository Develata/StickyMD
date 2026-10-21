//! Fixed native top-control layout and transient popup projection.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::keyboard::PhysicalKey;

use crate::instruction::WindowPreferenceIntent;

pub(super) const TOOLBAR_HEIGHT_DIP: f64 = 34.0;
const CONTROL_DIP: f64 = 28.0;
const CONTROL_GAP_DIP: f64 = 4.0;
const EDGE_DIP: f64 = 5.0;
// The compact edge stays outside the 6 DIP native resize hit region. At the
// 220 DIP minimum, removing inter-control gaps leaves nine approximately
// 23.1 DIP hit targets instead of shrinking them below usable size.
const COMPACT_EDGE_DIP: f64 = 6.0;
const COMPACT_GAP_DIP: f64 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlId {
    Source,
    Split,
    Preview,
    ConvertMath,
    Topmost,
    Theme,
    Opacity,
    Collapse,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ControlRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ControlRect {
    pub fn contains(self, point: PhysicalPosition<f64>) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }
}

/// Where a press lands on the open opacity popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PopupHit {
    Slider,
    Field,
    /// The popup's own surface: it covers what lies under it, so the press goes nowhere else.
    Blank,
}

#[derive(Debug, Clone)]
pub(super) struct ControlLayout {
    controls: [(ControlId, ControlRect); 9],
    pub toolbar: ControlRect,
    pub opacity_popup: ControlRect,
    pub opacity_slider: ControlRect,
    pub opacity_input: ControlRect,
}

impl ControlLayout {
    pub fn new(size: PhysicalSize<u32>, scale: f64) -> Self {
        let scale = scale.max(0.5);
        let toolbar_height = TOOLBAR_HEIGHT_DIP * scale;
        let regular_required = 2.0 * EDGE_DIP + 9.0 * CONTROL_DIP + 8.0 * CONTROL_GAP_DIP;
        let compact = size.width as f64 / scale < regular_required;
        let edge = if compact { COMPACT_EDGE_DIP } else { EDGE_DIP } * scale;
        let gap = if compact {
            COMPACT_GAP_DIP
        } else {
            CONTROL_GAP_DIP
        } * scale;
        let available = (size.width as f64 - 2.0 * edge - 8.0 * gap).max(9.0 * scale);
        let control = (available / 9.0).min(CONTROL_DIP * scale);
        let y = (toolbar_height - control) * 0.5;
        let left = [
            ControlId::Source,
            ControlId::Split,
            ControlId::Preview,
            ControlId::ConvertMath,
        ];
        let right = [
            ControlId::Topmost,
            ControlId::Theme,
            ControlId::Opacity,
            ControlId::Collapse,
            ControlId::Close,
        ];
        let mut controls = [(
            ControlId::Source,
            ControlRect {
                x: 0.0,
                y,
                width: control,
                height: control,
            },
        ); 9];
        for (index, id) in left.into_iter().enumerate() {
            controls[index] = (
                id,
                ControlRect {
                    x: edge + index as f64 * (control + gap),
                    y,
                    width: control,
                    height: control,
                },
            );
        }
        let right_width = right.len() as f64 * control + (right.len() - 1) as f64 * gap;
        let right_origin = size.width as f64 - edge - right_width;
        for (offset, id) in right.into_iter().enumerate() {
            controls[4 + offset] = (
                id,
                ControlRect {
                    x: right_origin + offset as f64 * (control + gap),
                    y,
                    width: control,
                    height: control,
                },
            );
        }
        let popup_width = (230.0 * scale).min(size.width as f64);
        let popup_height = 58.0 * scale;
        let opacity = controls
            .iter()
            .find_map(|(id, rect)| (*id == ControlId::Opacity).then_some(*rect))
            .unwrap_or(controls[0].1);
        let popup_x = (opacity.x + opacity.width - popup_width)
            .clamp(0.0, (size.width as f64 - popup_width).max(0.0));
        let opacity_popup = ControlRect {
            x: popup_x,
            y: toolbar_height,
            width: popup_width,
            height: popup_height,
        };
        let opacity_slider = ControlRect {
            x: popup_x + 12.0 * scale,
            y: toolbar_height + 19.0 * scale,
            width: 150.0 * scale,
            height: 20.0 * scale,
        };
        let opacity_input = ControlRect {
            x: popup_x + 172.0 * scale,
            y: toolbar_height + 13.0 * scale,
            width: 45.0 * scale,
            height: 30.0 * scale,
        };
        Self {
            controls,
            toolbar: ControlRect {
                x: 0.0,
                y: 0.0,
                width: size.width as f64,
                height: toolbar_height,
            },
            opacity_popup,
            opacity_slider,
            opacity_input,
        }
    }

    pub fn control_at(&self, point: PhysicalPosition<f64>) -> Option<ControlId> {
        self.controls
            .iter()
            .find_map(|(id, rect)| rect.contains(point).then_some(*id))
    }

    pub fn rect(&self, id: ControlId) -> ControlRect {
        self.controls
            .iter()
            .find_map(|(candidate, rect)| (*candidate == id).then_some(*rect))
            .unwrap_or(self.toolbar)
    }

    /// Where a press lands on the open opacity popup; `None` outside it.
    pub fn popup_hit(&self, point: PhysicalPosition<f64>) -> Option<PopupHit> {
        if self.opacity_slider.contains(point) {
            Some(PopupHit::Slider)
        } else if self.opacity_input.contains(point) {
            Some(PopupHit::Field)
        } else if self.opacity_popup.contains(point) {
            Some(PopupHit::Blank)
        } else {
            None
        }
    }

    pub fn is_drag_region(&self, point: PhysicalPosition<f64>) -> bool {
        self.toolbar.contains(point) && self.control_at(point).is_none()
    }

    pub fn opacity_at(&self, x: f64) -> u8 {
        let ratio = ((x - self.opacity_slider.x) / self.opacity_slider.width).clamp(0.0, 1.0);
        (40.0 + ratio * 60.0).round() as u8
    }
}

/// Split-only synchronization control centered over the fixed divider.
///
/// Keeping this control out of the nine-item toolbar preserves the 23 DIP
/// minimum hit target at the 220 DIP compact-window boundary.
pub(super) fn split_sync_rect(divider_x: u32, toolbar_height: u32, scale: f64) -> ControlRect {
    let size = 22.0 * scale.max(0.5);
    ControlRect {
        x: f64::from(divider_x) - size * 0.5,
        y: f64::from(toolbar_height) + 4.0 * scale.max(0.5),
        width: size,
        height: size,
    }
}

/// The tail of the focused opacity field's text, as painted: at most `TAIL` characters, each an
/// ASCII digit, `-` or `.` as itself and any other character as `?`, copied so the toolbar
/// projection stays `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OpacityField {
    bytes: [u8; OpacityField::TAIL],
    len: u8,
}

impl OpacityField {
    /// More characters than the field shows at any scale up to 600% (it fits about five per
    /// 100%).
    pub const TAIL: usize = 32;

    /// The last `TAIL` characters of `text`, found by walking back from its end one character at
    /// a time (a byte cut could split one), so the cost does not grow with the entry.
    fn tail_of(text: &str) -> Self {
        let start = text
            .char_indices()
            .rev()
            .nth(Self::TAIL - 1)
            .map_or(0, |(index, _)| index);
        let mut bytes = [0; Self::TAIL];
        let mut len = 0;
        for character in text[start..].chars() {
            bytes[len] = match character {
                '0'..='9' | '-' | '.' => character as u8,
                _ => b'?',
            };
            len += 1;
        }
        Self {
            bytes,
            len: len as u8,
        }
    }

    pub fn text(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

/// The value a field commits (plan 09: an integer field; out-of-range values clamp to 40–100;
/// a non-integer commits nothing). Any length is read without overflow: a negative integer is
/// below the range, and more than three significant digits are above it.
fn committed_opacity(input: &str) -> Option<u8> {
    let (negative, digits) = match input.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, input),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if negative {
        return Some(40);
    }
    let significant = digits.trim_start_matches('0');
    let value = if significant.len() > 3 {
        u16::MAX
    } else {
        significant.parse::<u16>().unwrap_or(0)
    };
    Some(value.clamp(40, 100) as u8)
}

#[derive(Debug, Clone)]
pub(super) struct ControlState {
    pub opacity_popup_open: bool,
    pub opacity_dragging: bool,
    pub opacity_preview: u8,
    pub opacity_input: String,
    pub opacity_input_focused: bool,
    /// The field shows a value it did not type (the committed or previewed one), as if it were
    /// selected: the next typed character or Backspace replaces it.
    opacity_input_selected: bool,
    /// The key whose press ended the field edit (Enter): while it is held its auto-repeats, and
    /// then its release, belong to the field, so a held Enter does not type into the note or
    /// Search the keyboard returns to.
    commit_key: Option<PhysicalKey>,
}

impl ControlState {
    pub fn new(opacity: u8) -> Self {
        let opacity = opacity.clamp(40, 100);
        Self {
            opacity_popup_open: false,
            opacity_dragging: false,
            opacity_preview: opacity,
            opacity_input: opacity.to_string(),
            opacity_input_focused: false,
            opacity_input_selected: true,
            commit_key: None,
        }
    }

    pub fn preview(&mut self, opacity: u8) -> u8 {
        self.opacity_preview = opacity.clamp(40, 100);
        self.opacity_input = self.opacity_preview.to_string();
        self.opacity_input_selected = true;
        self.opacity_preview
    }

    /// Focuses the field with its value selected.
    pub fn focus_input(&mut self) {
        self.opacity_input_focused = true;
        self.opacity_input_selected = true;
    }

    /// Whether the field is being edited: then it takes every key and IME commit, so none of
    /// them reaches the note or Search behind the popup.
    pub fn takes_keys(&self) -> bool {
        self.opacity_popup_open && self.opacity_input_focused
    }

    /// Ends the field edit, on Enter or when the field loses focus (another control, a click
    /// outside, the window losing focus): commits an integer, else shows the value again.
    pub fn finish_input(&mut self) -> Option<u8> {
        self.opacity_input_focused = false;
        let committed = self.commit_input();
        if committed.is_none() {
            self.preview(self.opacity_preview);
        }
        committed
    }

    /// Records the key whose press ends the edit (`commit_key`).
    pub fn claim_commit_key(&mut self, key: PhysicalKey) {
        self.commit_key = Some(key);
    }

    /// Whether a key event belongs to the key that ended the edit: its auto-repeats are taken,
    /// its release is taken and ends the claim, and a fresh press of it (its release was missed)
    /// ends the claim and goes on; any other key goes on.
    pub fn takes_commit_key_event(
        &mut self,
        key: PhysicalKey,
        pressed: bool,
        repeat: bool,
    ) -> bool {
        if self.commit_key != Some(key) {
            return false;
        }
        if !pressed || !repeat {
            self.commit_key = None;
        }
        !pressed || repeat
    }

    /// Types `text` into the field, every character of it: the field takes any input and only a
    /// commit reads it as an integer (plan 09: a non-integer such as 96.5 commits nothing; USER
    /// decision D5). Keys whose text is a control character (Tab, Delete) are not typing keys
    /// (`opacity_key`). The first character replaces a selected value.
    pub fn type_input(&mut self, text: &str) {
        for character in text.chars() {
            if std::mem::take(&mut self.opacity_input_selected) {
                self.opacity_input.clear();
            }
            self.opacity_input.push(character);
        }
    }

    /// Backspace: clears a selected value, else removes the last character.
    pub fn backspace_input(&mut self) {
        if std::mem::take(&mut self.opacity_input_selected) {
            self.opacity_input.clear();
        } else {
            self.opacity_input.pop();
        }
    }

    /// The focused field's text, for painting.
    pub fn focused_field(&self) -> Option<OpacityField> {
        self.opacity_input_focused
            .then(|| OpacityField::tail_of(&self.opacity_input))
    }

    /// Closes the popup without committing: ends any slider drag or numeric edit and shows the
    /// committed value again, so a later pointer release commits nothing.
    pub fn cancel(&mut self, committed: u8) {
        self.opacity_popup_open = false;
        self.opacity_dragging = false;
        self.opacity_input_focused = false;
        self.preview(committed);
    }

    /// A press on the slider at `value` starts a drag that previews live.
    pub fn press_slider(&mut self, value: u8) -> WindowPreferenceIntent {
        self.opacity_dragging = true;
        WindowPreferenceIntent::PreviewOpacity(value)
    }

    /// The pointer moved: a drag in progress previews the slider value `value` yields (read
    /// only then), never persisting.
    pub fn drag_slider(&self, value: impl FnOnce() -> u8) -> Option<WindowPreferenceIntent> {
        (self.opacity_popup_open && self.opacity_dragging)
            .then(|| WindowPreferenceIntent::PreviewOpacity(value()))
    }

    /// A pointer release yields at most one `CommitOpacity`, and only if a drag was in progress
    /// (plan 09: write the config on slider release, not per step).
    pub fn release_slider(&mut self) -> Option<WindowPreferenceIntent> {
        std::mem::take(&mut self.opacity_dragging)
            .then_some(WindowPreferenceIntent::CommitOpacity(self.opacity_preview))
    }

    pub fn commit_input(&mut self) -> Option<u8> {
        let opacity = committed_opacity(&self.opacity_input)?;
        self.preview(opacity);
        Some(opacity)
    }
}

#[cfg(test)]
mod phase8_control_tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn cancelling_the_opacity_popup_ends_every_edit_and_restores_the_committed_value() {
        for (focused, dragging) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut state = ControlState::new(80);
            state.opacity_popup_open = true;
            state.opacity_input_focused = focused;
            state.opacity_dragging = dragging;
            state.preview(55);
            state.type_input("4");
            state.cancel(80);
            assert!(!state.opacity_popup_open && !state.opacity_input_focused);
            assert!(!state.opacity_dragging, "a later release must not commit");
            assert_eq!(
                (state.opacity_preview, state.opacity_input.as_str()),
                (80, "80")
            );
        }
    }

    /// The open popup takes every press inside it: on the slider, on the field, or on its own
    /// surface, which covers the note, Search and scrollbars under it; a press outside is not its.
    #[test]
    fn the_open_popup_takes_every_press_inside_it() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for (width, height) in [(520, 680), (330, 360), (1200, 900), (220, 120)] {
                let size = PhysicalSize::new(
                    (f64::from(width) * scale).round() as u32,
                    (f64::from(height) * scale).round() as u32,
                );
                let layout = ControlLayout::new(size, scale);
                let center = |rect: ControlRect| {
                    PhysicalPosition::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
                };
                let popup = layout.opacity_popup;
                let label = format!("{size:?} at {scale}");
                assert_eq!(
                    layout.popup_hit(center(layout.opacity_slider)),
                    Some(PopupHit::Slider),
                    "{label}"
                );
                assert_eq!(
                    layout.popup_hit(center(layout.opacity_input)),
                    Some(PopupHit::Field),
                    "{label}"
                );
                let gap = PhysicalPosition::new(
                    (layout.opacity_slider.x
                        + layout.opacity_slider.width
                        + layout.opacity_input.x)
                        / 2.0,
                    center(layout.opacity_input).y,
                );
                for point in [
                    PhysicalPosition::new(popup.x, popup.y),
                    PhysicalPosition::new(
                        popup.x + popup.width - 1.0,
                        popup.y + popup.height - 1.0,
                    ),
                    gap,
                ] {
                    assert_eq!(
                        layout.popup_hit(point),
                        Some(PopupHit::Blank),
                        "{label} {point:?}"
                    );
                }
                for point in [
                    PhysicalPosition::new(popup.x, popup.y - 1.0),
                    PhysicalPosition::new(popup.x, popup.y + popup.height),
                    PhysicalPosition::new(popup.x - 1.0, popup.y),
                ] {
                    assert_eq!(layout.popup_hit(point), None, "{label} {point:?}");
                }
            }
        }
        // The round 2 review press: inside the popup, on neither control.
        let layout = ControlLayout::new(PhysicalSize::new(520, 680), 1.0);
        assert_eq!(
            layout.popup_hit(PhysicalPosition::new(300.0, 40.0)),
            Some(PopupHit::Blank)
        );
        // The round 3 review press: the minimum window's popup covers its left resize border.
        let layout = ControlLayout::new(PhysicalSize::new(220, 120), 1.0);
        assert_eq!(
            layout.popup_hit(PhysicalPosition::new(1.0, 40.0)),
            Some(PopupHit::Blank)
        );
    }

    #[test]
    fn phase8_controls_share_layout_between_hit_testing_and_painting() {
        let layout = ControlLayout::new(PhysicalSize::new(520, 680), 1.0);
        for id in [
            ControlId::Source,
            ControlId::Split,
            ControlId::Preview,
            ControlId::ConvertMath,
            ControlId::Topmost,
            ControlId::Theme,
            ControlId::Opacity,
            ControlId::Collapse,
            ControlId::Close,
        ] {
            let rect = layout.rect(id);
            let center =
                PhysicalPosition::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
            assert_eq!(layout.control_at(center), Some(id));
        }
    }

    /// The pointer decisions through the real preference reducer: every drag step is a
    /// `PreviewOpacity` the reducer does not persist; a valid release yields one `CommitOpacity`,
    /// which the reducer persists when the value changed; after Esc a release yields nothing.
    /// The runtime's config submission and the bytes on disk are not exercised here.
    #[test]
    fn phase8_a_slider_drag_commits_at_most_once_on_release() {
        use crate::config::{ConfigCoordinator, RuntimeConfig};
        use crate::flow::{WindowPreferenceEffect, coordinate_window_preference};

        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        let mut controls = ControlState::new(96);
        controls.opacity_popup_open = true;
        // Applies one intent as the runtime does; returns the value the reducer asks to persist.
        let apply = |config: &mut ConfigCoordinator, controls: &mut ControlState, intent| {
            let effect = coordinate_window_preference(config, intent).unwrap();
            let WindowPreferenceEffect::ApplyOpacity { opacity, persist } = effect else {
                panic!("{effect:?}");
            };
            controls.preview(opacity);
            persist.then_some(opacity)
        };
        // The runtime writes the config whenever it is dirty, from whichever event comes next
        // (`submit_config_if_needed`); a drag must therefore leave it clean at every step.
        let assert_clean = |config: &mut ConfigCoordinator, step: &str| {
            assert!(!config.is_dirty(), "{step} dirtied the config");
            assert_eq!(config.current().opacity, 96, "{step}");
            assert!(config.begin_persist().is_none(), "{step} would be written");
        };
        let mut persisted = Vec::new();
        let press = controls.press_slider(80);
        persisted.extend(apply(&mut config, &mut controls, press));
        assert_clean(&mut config, "the press");
        for value in [70, 60, 55] {
            let drag = controls.drag_slider(|| value).expect("dragging previews");
            persisted.extend(apply(&mut config, &mut controls, drag));
            assert_clean(&mut config, "a drag step");
        }
        assert!(persisted.is_empty(), "{persisted:?}");
        let release = controls.release_slider().expect("the release commits");
        persisted.extend(apply(&mut config, &mut controls, release));
        assert_eq!(persisted, [55]);
        assert!(config.is_dirty(), "the release is written once");
        assert_eq!(config.current().opacity, 55);
        // No drag in progress: moves preview nothing and a release commits nothing.
        assert!(controls.drag_slider(|| 60).is_none());
        assert!(controls.release_slider().is_none());
        // Esc mid-drag cancels it; the later release commits nothing.
        controls.opacity_popup_open = true;
        controls.press_slider(70);
        controls.cancel(55);
        assert!(controls.drag_slider(|| 65).is_none());
        assert!(controls.release_slider().is_none());
    }

    /// Plan 09 opacity range and input: the slider maps every position to an integer from 40
    /// to 100 in steps of one and reaches each, at every DPI; the field keeps whatever is typed
    /// key by key, a commit clamps an integer into the range and a non-integer commits nothing
    /// (plan 09, AC-024; USER decision D5, 2026-10-20); the configured default is 96.
    #[test]
    fn the_opacity_range_is_40_to_100_in_integer_steps() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let layout = ControlLayout::new(PhysicalSize::new(520, 680), scale);
            let slider = layout.opacity_slider;
            let mut previous = layout.opacity_at(slider.x - 40.0);
            assert_eq!(previous, 40, "left of the slider at {scale}");
            let mut reached = std::collections::BTreeSet::from([previous]);
            let mut x = slider.x - 40.0;
            while x <= slider.x + slider.width + 40.0 {
                let value = layout.opacity_at(x);
                assert!(
                    value >= previous && value - previous <= 1,
                    "{previous} -> {value} at x={x}, scale {scale}"
                );
                reached.insert(value);
                previous = value;
                x += 0.25;
            }
            assert_eq!(previous, 100, "right of the slider at {scale}");
            assert_eq!(reached, (40..=100).collect(), "every value at {scale}");
        }

        // Each entry is typed one key at a time into a freshly focused field.
        let typed = |keys: &str| {
            let mut state = ControlState::new(70);
            state.opacity_popup_open = true;
            state.focus_input();
            for key in keys.chars() {
                state.type_input(&key.to_string());
            }
            state
        };
        for (keys, field, committed) in [
            ("0", "0", Some(40)),
            ("39", "39", Some(40)),
            ("40", "40", Some(40)),
            ("96", "96", Some(96)),
            ("100", "100", Some(100)),
            ("101", "101", Some(100)),
            ("999", "999", Some(100)),
            ("1000", "1000", Some(100)),
            ("007", "007", Some(40)),
            ("096", "096", Some(96)),
            ("0096", "0096", Some(96)),
            ("0000", "0000", Some(40)),
            ("-1", "-1", Some(40)),
            ("-0", "-0", Some(40)),
            ("-250", "-250", Some(40)),
            // More digits than any integer type holds still clamp.
            (
                "123456789012345678901234567890",
                "123456789012345678901234567890",
                Some(100),
            ),
            (
                "-123456789012345678901234567890",
                "-123456789012345678901234567890",
                Some(40),
            ),
            // AC-024: out of range clamps; a non-integer commits nothing.
            ("35", "35", Some(40)),
            ("105", "105", Some(100)),
            ("96.5", "96.5", None),
            // Every other character is kept and makes the entry a non-integer.
            ("4.5", "4.5", None),
            ("9%", "9%", None),
            (" 9", " 9", None),
            ("9x", "9x", None),
            ("+5", "+5", None),
            ("5-5", "5-5", None),
            ("--5", "--5", None),
            ("-", "-", None),
            ("x", "x", None),
            // An integer is ASCII digits.
            ("\u{663}\u{665}", "\u{663}\u{665}", None),
            ("\u{ff15}\u{ff10}", "\u{ff15}\u{ff10}", None),
            // So does a control character an IME commit carries.
            ("9\t6", "9\t6", None),
        ] {
            let mut state = typed(keys);
            assert_eq!(state.opacity_input, field, "{keys:?}");
            let before = state.opacity_preview;
            assert_eq!(state.commit_input(), committed, "{keys:?}");
            match committed {
                Some(value) => {
                    assert_eq!(state.opacity_preview, value, "{keys:?}");
                    assert_eq!(state.opacity_input, value.to_string(), "{keys:?}");
                }
                None => assert_eq!(state.opacity_preview, before, "{keys:?} changed the value"),
            }
        }
        // A whole entry delivered as one text event types the same.
        let mut state = typed("");
        state.type_input("-1000");
        assert_eq!(state.opacity_input, "-1000");
        // An empty field commits nothing.
        let mut state = typed("");
        state.backspace_input();
        assert_eq!(state.opacity_input, "");
        assert_eq!(state.commit_input(), None, "an empty field commits nothing");
        assert_eq!(state.opacity_preview, 70);

        let default = crate::config::RuntimeConfig::default().opacity;
        assert_eq!(default, 96);
        assert_eq!(ControlState::new(default).opacity_preview, 96);

        // The preference reducer clamps both previews and commits into the range again.
        use crate::config::{ConfigCoordinator, RuntimeConfig};
        use crate::flow::{WindowPreferenceEffect, coordinate_window_preference};
        use crate::instruction::WindowPreferenceIntent;
        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        for (requested, applied) in [
            (0, 40),
            (39, 40),
            (40, 40),
            (100, 100),
            (101, 100),
            (255, 100),
        ] {
            for intent in [
                WindowPreferenceIntent::PreviewOpacity(requested),
                WindowPreferenceIntent::CommitOpacity(requested),
            ] {
                let effect = coordinate_window_preference(&mut config, intent).unwrap();
                assert!(
                    matches!(effect, WindowPreferenceEffect::ApplyOpacity { opacity, .. } if opacity == applied),
                    "{intent:?} -> {effect:?}"
                );
            }
            assert_eq!(config.current().opacity, applied, "{requested} committed");
        }
    }

    #[test]
    fn phase8_opacity_slider_and_input_clamp_only_on_commit() {
        let layout = ControlLayout::new(PhysicalSize::new(520, 680), 1.0);
        assert_eq!(layout.opacity_at(layout.opacity_slider.x - 100.0), 40);
        assert_eq!(
            layout.opacity_at(layout.opacity_slider.x + layout.opacity_slider.width + 100.0),
            100
        );
        let mut state = ControlState::new(96);
        state.focus_input();
        state.type_input("7");
        assert_eq!(state.opacity_input, "7");
        assert_eq!(state.opacity_preview, 96);
        assert_eq!(state.commit_input(), Some(40));
        state.type_input("x");
        assert_eq!(state.opacity_input, "x");
        assert_eq!(state.commit_input(), None, "a non-integer commits nothing");
        assert_eq!(state.opacity_preview, 40);
    }

    /// Editing the field: focusing selects its value, so the first typed key or Backspace
    /// replaces it and later keys append or remove one character; a value the slider or a
    /// commit writes is selected again. The focused field paints its typed text.
    #[test]
    fn the_opacity_field_edits_like_a_selected_text_field() {
        let mut state = ControlState::new(96);
        assert_eq!(
            state.focused_field(),
            None,
            "an unfocused field paints its value"
        );
        state.focus_input();
        assert_eq!(state.focused_field().unwrap().text(), b"96");
        state.type_input("5");
        assert_eq!(
            state.opacity_input, "5",
            "the first key replaces the selected value"
        );
        state.type_input("0");
        assert_eq!(state.opacity_input, "50", "later keys append");
        state.type_input(".");
        assert_eq!(state.opacity_input, "50.", "every character is typed");
        state.backspace_input();
        state.backspace_input();
        assert_eq!(state.opacity_input, "5");
        assert_eq!(state.focused_field().unwrap().text(), b"5");
        // Any character replaces a selected value, a minus sign as any other.
        state.focus_input();
        state.type_input("x");
        assert_eq!(state.opacity_input, "x");
        state.focus_input();
        state.type_input("-");
        assert_eq!(
            state.opacity_input, "-",
            "a minus sign replaces the selected value"
        );
        // Backspace on a selected value clears all of it.
        state.type_input("23");
        assert_eq!(state.opacity_input, "-23");
        state.focus_input();
        state.backspace_input();
        assert_eq!(state.opacity_input, "");
        // The slider writes a value, selected again.
        state.type_input("4");
        state.preview(60);
        state.type_input("7");
        assert_eq!(state.opacity_input, "7");
        // So does a commit.
        state.type_input("5");
        assert_eq!(state.commit_input(), Some(75));
        state.type_input("8");
        assert_eq!(state.opacity_input, "8");
        // A long entry keeps its last characters for painting.
        state.focus_input();
        state.type_input("-12345678901234567890123456789012345");
        let field = state.focused_field().unwrap();
        assert_eq!(field.text().len(), OpacityField::TAIL);
        assert_eq!(field.text(), b"45678901234567890123456789012345");
        // Digits, `-` and `.` paint as themselves, any other character as `?`, a character at a
        // time however many bytes it takes.
        state.focus_input();
        state.type_input("-9.6x\u{663}");
        assert_eq!(state.focused_field().unwrap().text(), b"-9.6??");
        state.focus_input();
        state.type_input(&format!("{}5", "\u{663}".repeat(40)));
        let field = state.focused_field().unwrap();
        assert_eq!(field.text().len(), OpacityField::TAIL);
        assert_eq!(field.text(), format!("{}5", "?".repeat(31)).as_bytes());
    }

    /// The Enter that ends an edit keeps its auto-repeats and its release, so holding it does not
    /// type newlines into the note behind the field; a fresh press of it, or its release, ends
    /// the claim, and other keys are not touched.
    #[test]
    fn the_key_that_ends_an_edit_keeps_its_repeats_and_release() {
        use winit::keyboard::KeyCode;
        let enter = PhysicalKey::Code(KeyCode::Enter);
        let other = PhysicalKey::Code(KeyCode::KeyA);
        let mut state = ControlState::new(96);
        assert!(
            !state.takes_commit_key_event(enter, true, true),
            "nothing is claimed"
        );
        state.claim_commit_key(enter);
        assert!(
            !state.takes_commit_key_event(other, true, false),
            "another key"
        );
        assert!(
            !state.takes_commit_key_event(other, true, true),
            "another repeat"
        );
        assert!(state.takes_commit_key_event(enter, true, true), "a repeat");
        assert!(
            state.takes_commit_key_event(enter, true, true),
            "every repeat"
        );
        assert!(
            state.takes_commit_key_event(enter, false, false),
            "the release"
        );
        assert!(
            !state.takes_commit_key_event(enter, true, true),
            "the release ended the claim"
        );
        // A fresh press (its release was missed) ends the claim and goes on.
        state.claim_commit_key(enter);
        assert!(
            !state.takes_commit_key_event(enter, true, false),
            "a fresh press"
        );
        assert!(
            !state.takes_commit_key_event(enter, true, true),
            "the fresh press ended the claim"
        );
    }

    /// While edited the field takes every key and IME commit; ending the edit (Enter, or the
    /// field losing focus) commits an integer, or shows the value again with nothing committed.
    #[test]
    fn the_opacity_field_takes_keys_while_edited_and_commits_when_it_ends() {
        for (open, focused) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut state = ControlState::new(96);
            state.opacity_popup_open = open;
            if focused {
                state.focus_input();
            }
            assert_eq!(
                state.takes_keys(),
                open && focused,
                "open {open} focused {focused}"
            );
        }
        let mut state = ControlState::new(96);
        state.opacity_popup_open = true;
        state.focus_input();
        state.type_input("50");
        assert_eq!(state.finish_input(), Some(50));
        assert!(!state.opacity_input_focused && !state.takes_keys());
        assert_eq!(
            (state.opacity_preview, state.opacity_input.as_str()),
            (50, "50")
        );
        assert!(
            state.opacity_input_selected,
            "the committed value is selected"
        );
        // A field without an integer commits nothing and shows the value again, selected.
        state.focus_input();
        state.type_input("-");
        assert_eq!(state.finish_input(), None);
        assert!(!state.opacity_input_focused);
        assert_eq!(
            (state.opacity_preview, state.opacity_input.as_str()),
            (50, "50")
        );
        assert!(state.opacity_input_selected, "the shown value is selected");
        state.focus_input();
        state.type_input("7");
        assert_eq!(
            state.opacity_input, "7",
            "the shown value is selected again"
        );
    }

    #[test]
    fn phase10_compact_layout_keeps_every_control_inside_without_overlap() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let width = (220.0 * scale) as u32;
            let layout =
                ControlLayout::new(PhysicalSize::new(width, (120.0 * scale) as u32), scale);
            let mut previous_right = 0.0;
            for id in [
                ControlId::Source,
                ControlId::Split,
                ControlId::Preview,
                ControlId::ConvertMath,
                ControlId::Topmost,
                ControlId::Theme,
                ControlId::Opacity,
                ControlId::Collapse,
                ControlId::Close,
            ] {
                let rect = layout.rect(id);
                assert!(rect.width / scale >= 23.0);
                assert!(rect.x + f64::EPSILON * 256.0 >= previous_right);
                assert!(rect.x + rect.width <= f64::from(width) + f64::EPSILON);
                previous_right = rect.x + rect.width;
            }
        }
    }

    #[test]
    fn phase14_split_sync_hit_target_is_centered_on_divider() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let rect = split_sync_rect(260, 34, scale);
            assert!(((rect.x + rect.width / 2.0) - 260.0).abs() < f64::EPSILON * 32.0);
            assert!(rect.width / scale >= 22.0);
            assert!(rect.y >= 34.0);
        }
    }

    #[test]
    #[ignore = "Release-only Phase 8 control-layout performance receipt"]
    fn phase8_performance_control_layout_and_hit_test() {
        let mut samples = Vec::with_capacity(25);
        for _ in 0..25 {
            let started = Instant::now();
            let mut hits = 0usize;
            for index in 0..100_000 {
                let layout = ControlLayout::new(
                    PhysicalSize::new(520 + (index % 400), 680),
                    [1.0, 1.25, 1.5, 2.0][index as usize % 4],
                );
                hits += layout
                    .control_at(PhysicalPosition::new((index % 520) as f64, 17.0))
                    .is_some() as usize;
            }
            std::hint::black_box(hits);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        eprintln!(
            "phase8 control_layout_100k median={:?} p95={:?} max={:?}",
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples[samples.len() - 1]
        );
    }
}
