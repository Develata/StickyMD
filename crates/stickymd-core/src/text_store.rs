//! Text storage abstraction and the v1 `String` implementation.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
//!
//! The internal representation is UTF-8 + `\n`. `DocumentState` owns a `TextStore`;
//! v1 uses `String`. If the 1 MiB performance gate fails, the store can be swapped
//! for a rope **without** changing this trait or the callers above it. (Rope is
//! intentionally not introduced before the benchmark justifies it.)

use crate::error::EditError;
use crate::text_delta::TextDelta;

/// Storage for the canonical document text.
pub trait TextStore {
    /// The full document text as a UTF-8 `str`.
    fn as_str(&self) -> &str;

    /// Apply `delta`, validating char boundaries and bounds first.
    fn apply(&mut self, delta: &TextDelta) -> Result<(), EditError>;

    /// Length in bytes.
    fn len_bytes(&self) -> usize;

    /// True when the store holds no text.
    fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }
}

/// v1 text store backed by a plain `String`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StringTextStore {
    text: String,
}

impl StringTextStore {
    /// Create a store from already-normalized (UTF-8 + `\n`) text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Replace the whole buffer (used by reload/recovery reconciliation).
    pub fn replace_all(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl TextStore for StringTextStore {
    fn as_str(&self) -> &str {
        &self.text
    }

    fn apply(&mut self, delta: &TextDelta) -> Result<(), EditError> {
        // Validate first; on any error the store is left untouched.
        delta.validate(&self.text)?;
        self.text
            .replace_range(delta.range.clone(), &delta.replacement);
        Ok(())
    }

    fn len_bytes(&self) -> usize {
        self.text.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_insert_and_replace() {
        let mut store = StringTextStore::new("hello");
        store.apply(&TextDelta::insert(5, " world")).unwrap();
        assert_eq!(store.as_str(), "hello world");
        store.apply(&TextDelta::new(0..5, "HEY")).unwrap();
        assert_eq!(store.as_str(), "HEY world");
    }

    #[test]
    fn apply_failure_leaves_store_untouched() {
        let mut store = StringTextStore::new("héllo");
        let before = store.as_str().to_string();
        let bad = TextDelta::new(1..2, "x"); // mid 'é'
        assert!(store.apply(&bad).is_err());
        assert_eq!(store.as_str(), before);
    }

    #[test]
    fn len_and_empty() {
        let mut store = StringTextStore::new("");
        assert!(store.is_empty());
        store.apply(&TextDelta::insert(0, "abc")).unwrap();
        assert_eq!(store.len_bytes(), 3);
        assert!(!store.is_empty());
    }

    #[test]
    fn replace_all_overwrites() {
        let mut store = StringTextStore::new("old");
        store.replace_all("brand new");
        assert_eq!(store.as_str(), "brand new");
    }
}
