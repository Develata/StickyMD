//! Monotonic document version.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#generation-semantics统一规则
//!
//! Every text mutation increments the generation. Background results carry the
//! generation they were computed from; stale results must be dropped without
//! side effects.

/// Monotonic, process-local document version number.
///
/// A `Generation` only expresses document version ordering; it is **not** a
/// timestamp and restarts from [`Generation::initial`] on each process launch
/// (paired with the disk hash to identify state across restarts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// The generation of a freshly loaded/created document.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// The generation that follows `self`. Saturates at `u64::MAX` so a
    /// pathological long session can never wrap and break ordering.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Raw numeric value (for diagnostics and ordering only).
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_monotonic() {
        let g = Generation::initial();
        assert_eq!(g.value(), 0);
        assert_eq!(g.next().value(), 1);
        assert!(g.next() > g);
    }

    #[test]
    fn generation_next_saturates() {
        let max = Generation(u64::MAX);
        assert_eq!(max.next(), max);
    }
}
