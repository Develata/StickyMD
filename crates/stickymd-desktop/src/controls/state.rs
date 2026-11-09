//! The opacity popup's transient state: the slider drag and the numeric field.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use stickymd_app::instruction::WindowPreferenceIntent;
use winit::keyboard::PhysicalKey;

/// The tail of the focused opacity field's text, as painted: at most `TAIL` characters, each an
/// ASCII digit, `-` or `.` as itself and any other character as `?`, copied so the toolbar
/// projection stays `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpacityField {
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
pub struct ControlState {
    pub opacity_popup_open: bool,
    pub opacity_dragging: bool,
    pub opacity_preview: u8,
    pub opacity_input: String,
    pub opacity_input_focused: bool,
    /// The field shows a value it did not type (the committed or previewed one), as if it were
    /// selected: the next typed character or Backspace replaces it.
    pub(super) opacity_input_selected: bool,
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
