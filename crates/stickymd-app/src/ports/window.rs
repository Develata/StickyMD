//! What the shell needs a native answer for about its window: whether it is shown, its z-order,
//! opacity and frame, and the identity that keeps it reachable. What every window system does the
//! same way stays a direct call of the shell (plan 03 rule 3). These run on the main thread, so
//! none of them is `Send` (rule 6).
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use crate::flow::window::geometry::PhysicalRect;

use super::{FailureKind, HostFailure};

/// `None` is a fact this host cannot observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visibility {
    pub visible: Option<bool>,
    pub minimized: Option<bool>,
}

impl Visibility {
    /// The Execution Receipt of a show: visible and not minimized (plan 04). A value known to be
    /// wrong is a failed show even when the other cannot be read; only then is a missing value
    /// `Unsupported`.
    pub fn show_outcome(self) -> Result<(), FailureKind> {
        match (self.visible, self.minimized) {
            (Some(true), Some(false)) => Ok(()),
            (Some(false), _) | (_, Some(true)) => Err(FailureKind::Failed),
            _ => Err(FailureKind::Unsupported),
        }
    }
}

/// `None` is a fact this host cannot observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopmostFact {
    pub topmost: Option<bool>,
}

impl TopmostFact {
    /// The Execution Receipt of a topmost request for `wanted`. It proves the bit, nothing about
    /// the z-order against other windows or focus.
    pub fn outcome(self, wanted: bool) -> Result<(), FailureKind> {
        match self.topmost {
            Some(topmost) if topmost == wanted => Ok(()),
            Some(_) => Err(FailureKind::Failed),
            None => Err(FailureKind::Unsupported),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameFact {
    pub outer: PhysicalRect,
}

/// Today's hosts answer `show`, `hide`, `focus` and `set_topmost` within the same dispatch; the
/// coordinator's receipt deadlines exist so a host that does not cannot hold a wait open forever.
/// A host that answers later must guarantee cancellation or ordering of what it was asked. An
/// action expires when a tick processes its deadline: a receipt that arrives before that tick is
/// accepted, whatever its `now_ms`, because a synchronous call may simply have taken that long; one
/// that arrives after it is ignored even if it reports success, and the window is brought back in
/// line by a new request under a new id.
pub trait WindowHost {
    /// Shows the window and takes it out of the minimized state, then reads back what it can.
    fn show(&mut self) -> Result<Visibility, FailureKind>;

    /// Hides the window and asks it afterwards: a window still visible is `Failed`, one that
    /// cannot say is `Unsupported`, and no window is `Unavailable`.
    fn hide(&mut self) -> Result<(), FailureKind>;

    /// Takes the window out of the minimized state and asks for focus. Only a real focus event
    /// proves the focus, so the read-back says nothing about it.
    fn focus(&mut self) -> Result<Visibility, FailureKind>;

    fn visibility(&self) -> Result<Visibility, FailureKind>;

    /// Sets the z-order without activating the window and reads back the bit; a request before
    /// the window exists does nothing, because the startup applies the current value.
    fn set_topmost(&mut self, on: bool) -> Result<TopmostFact, HostFailure>;

    /// As `set_topmost` for a host that has opacity. A host without it keeps the default.
    fn set_opacity(&mut self, _percent: u8) -> Result<(), HostFailure> {
        Err(HostFailure::new(
            FailureKind::Unsupported,
            "this desktop has no window opacity",
        ))
    }

    /// Starts a move and resize; it proves nothing about where the window ends up.
    fn apply_frame(&mut self, target: PhysicalRect) -> Result<(), FailureKind>;

    fn observe_frame(&self) -> Result<FrameFact, FailureKind>;

    /// Puts back the identity that goes with a window reached from the tray, which window changes
    /// may take away. A host without such an identity keeps the default.
    fn reassert_identity(&mut self) -> Result<(), HostFailure> {
        Ok(())
    }

    /// Gives the window an ordinary entry among the desktop's open windows: the way back when the
    /// tray is not one.
    fn restore_ordinary_entry(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(visible: Option<bool>, minimized: Option<bool>) -> Result<(), FailureKind> {
        Visibility { visible, minimized }.show_outcome()
    }

    #[test]
    fn a_topmost_receipt_is_the_read_back_of_the_wanted_bit() {
        for (fact, wanted, expected) in [
            (Some(true), true, Ok(())),
            (Some(false), false, Ok(())),
            (Some(false), true, Err(FailureKind::Failed)),
            (Some(true), false, Err(FailureKind::Failed)),
            (None, true, Err(FailureKind::Unsupported)),
        ] {
            assert_eq!(TopmostFact { topmost: fact }.outcome(wanted), expected);
        }
    }

    /// Every combination of the two read-backs: a known wrong value beats a missing one.
    #[test]
    fn a_show_succeeds_only_when_both_read_backs_are_right() {
        use FailureKind::{Failed, Unsupported};
        for (visible, minimized, expected) in [
            (Some(true), Some(false), Ok(())),
            (Some(true), Some(true), Err(Failed)),
            (Some(false), Some(false), Err(Failed)),
            (Some(false), Some(true), Err(Failed)),
            (Some(false), None, Err(Failed)),
            (None, Some(true), Err(Failed)),
            (Some(true), None, Err(Unsupported)),
            (None, Some(false), Err(Unsupported)),
            (None, None, Err(Unsupported)),
        ] {
            assert_eq!(
                outcome(visible, minimized),
                expected,
                "{visible:?} {minimized:?}"
            );
        }
    }
}
