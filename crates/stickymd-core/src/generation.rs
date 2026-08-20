//! Monotonic document generation used to reject stale work.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#generation

use core::fmt;

/// Monotonic version of the canonical document state.
///
/// A generation is an ordering token, not a historical snapshot id. Undo and redo
/// therefore advance it just like ordinary edits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// Generation of a freshly loaded or created document.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Return the next generation, or `None` rather than wrapping or saturating.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Raw value for diagnostics and ordering only.
    pub const fn value(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_next_advances_without_wrapping() {
        assert_eq!(
            Generation::initial().checked_next().map(Generation::value),
            Some(1)
        );
        assert_eq!(Generation::for_test(u64::MAX).checked_next(), None);
    }
}
