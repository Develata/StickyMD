//! What a session step asks of the shell, in the order it must happen.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use crate::config::ViewMode;
use crate::flow::window::state::WindowEffect;
use crate::flow::{PreviewAction, WindowPreferenceEffect};
use stickymd_core::Generation;

use super::document::ProjectionChange;
use super::turns::SessionTurn;

/// One request to the shell. The shell applies them in order and decides nothing about them.
#[derive(Debug)]
pub enum ShellEffect {
    /// A window effect for the shell to carry out.
    Window(WindowEffect),
    /// The session's next step, which comes after what the shell did so far: hand it back to
    /// `AppSession::resume`.
    Resume(SessionTurn),
    Projection(ProjectionChange),
    /// The document moved to this generation: the Preview's schedule and offsets follow it.
    PreviewDocumentChanged(Generation),
    /// A Preview build or relayout fell due: the shell supplies the viewport.
    SubmitPreview(PreviewAction),
    /// The editor state changed: the caret, the IME area and the frame follow it.
    PresentationChanged,
    /// The view is now this one: the shell lays it out and gives the keyboard to its pane.
    ViewModeChanged(ViewMode),
    /// Whoever takes the keyboard may have changed: the IME follows it.
    KeyboardOwnerChanged,
    /// The note's pane takes the keyboard from Preview.
    SourceFocused,
    /// Search moved, opened or closed: the IME area follows its field or the caret.
    ImeAreaChanged,
    /// The configuration took a window preference: the shell shows it.
    Preference(WindowPreferenceEffect),
    TitleDirty,
    RedrawNeeded,
    /// Replaces the status line; the last one in a list is the one that stays.
    Diagnostic(String),
    /// Recovery no longer holds the directory watcher back.
    StartWatcher,
    /// The window lost focus: end the drags, the hover and the edit the shell owns.
    FocusLost,
    /// The quit is over: destroy the tray, then end the event loop when the callback ends.
    Exit,
}

/// Where one step's effects start in the buffer.
#[derive(Debug, Clone, Copy)]
pub struct Frame(usize);

/// The ordered output of session steps, owned by the shell and reused from event to event, so a
/// step allocates nothing once the buffer has grown.
///
/// Steps nest: applying an effect can run another step, and that step's effects come before what
/// is left of the outer one, as a call's do. So the buffer is a stack of frames. A step pushes its
/// effects into a new frame (`open`), `seal` reverses that frame, and `next` pops it, which
/// yields the effects in the order they were pushed. Invariant: while `next` is being called on
/// a frame, every frame above it has been emptied, so popping only ever reaches this frame.
#[derive(Debug, Default)]
pub struct ShellEffects {
    stack: Vec<ShellEffect>,
}

impl ShellEffects {
    pub fn open(&self) -> Frame {
        Frame(self.stack.len())
    }

    pub fn push(&mut self, effect: ShellEffect) {
        self.stack.push(effect);
    }

    pub fn seal(&mut self, frame: Frame) {
        self.stack[frame.0..].reverse();
    }

    pub fn next(&mut self, frame: Frame) -> Option<ShellEffect> {
        if self.stack.len() > frame.0 {
            self.stack.pop()
        } else {
            None
        }
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.stack.capacity()
    }
}
