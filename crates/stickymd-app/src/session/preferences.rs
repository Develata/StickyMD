//! Window preferences as the session applies them: the configuration takes the change, and the
//! session decides when it is written. The shell shows it (`ShellEffect::Preference`).
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use crate::flow::window::state::WindowIntent;
use crate::flow::{WindowPreferenceEffect, coordinate_window_preference};
use crate::instruction::WindowPreferenceIntent;

use super::{AppSession, ShellEffect, ShellEffects};

/// How long the content zoom must stay still before its configuration is written.
const ZOOM_CONFIG_SETTLE_MS: u64 = 250;

/// What changed the content zoom, which decides when its configuration is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomInput {
    /// A shortcut: one deliberate step, written at once.
    Key,
    /// The wheel: a stream of steps, written once it settles.
    Wheel,
}

impl AppSession {
    /// A changed preference is written at once, except an opacity that is only previewed, the
    /// content zoom, whose write waits until the zoom settles (`defer_zoom_config`), and
    /// always-on-top, whose window has to confirm it first. With no window flow there is nothing
    /// to confirm and the startup applies whatever the configuration holds.
    pub fn dispatch_window_preference(
        &mut self,
        intent: WindowPreferenceIntent,
        out: &mut ShellEffects,
    ) {
        if let WindowPreferenceIntent::SetAlwaysOnTop(on) = intent
            && self.window_flow.is_some()
        {
            let now_ms = self.now_ms();
            self.dispatch_window_intent(WindowIntent::UserTopmostRequested { on, now_ms }, out);
            return;
        }
        self.apply_window_preference(intent, out);
    }

    /// The window confirmed the user's value: the configuration, its write and the tray take it.
    pub(super) fn commit_user_topmost(&mut self, on: bool, out: &mut ShellEffects) {
        self.apply_window_preference(WindowPreferenceIntent::SetAlwaysOnTop(on), out);
    }

    fn apply_window_preference(&mut self, intent: WindowPreferenceIntent, out: &mut ShellEffects) {
        let effect = match coordinate_window_preference(&mut self.config, intent) {
            Ok(effect) => effect,
            Err(error) => {
                out.push(ShellEffect::Diagnostic(error.to_string()));
                return;
            }
        };
        let persist = match effect {
            WindowPreferenceEffect::NoOp => return,
            WindowPreferenceEffect::ApplyOpacity { persist, .. } => persist,
            WindowPreferenceEffect::ApplyContentZoom(_) => false,
            WindowPreferenceEffect::ApplyTheme(_)
            | WindowPreferenceEffect::ApplyAlwaysOnTop(_)
            | WindowPreferenceEffect::ApplySplitScrollSync(_) => true,
        };
        out.push(ShellEffect::Preference(effect));
        if persist {
            self.submit_config_if_needed();
        }
        out.push(ShellEffect::RedrawNeeded);
    }

    /// The shell has shown a new content zoom: its write waits until the zoom has been still for
    /// `ZOOM_CONFIG_SETTLE_MS`, counted from now.
    pub fn defer_zoom_config(&mut self) {
        self.zoom_config_deadline = Some(self.now_ms().saturating_add(ZOOM_CONFIG_SETTLE_MS));
    }

    /// The shell has applied a zoom from `input`, its relayout included. A key's is written now
    /// and drops the settle deadline the shown zoom took; a wheel's keeps that deadline.
    pub fn finish_zoom_input(&mut self, input: ZoomInput) {
        match input {
            ZoomInput::Key => {
                self.zoom_config_deadline = None;
                self.submit_config_if_needed();
            }
            ZoomInput::Wheel => {}
        }
    }
}
