//! Minimal storage boundary and the v1 `String` implementation.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate

use std::ops::Range;

use crate::DocumentError;

pub(crate) trait TextStore {
    fn as_str(&self) -> &str;
    fn len_bytes(&self) -> usize;
    fn replace(&mut self, range: Range<usize>, replacement: &str) -> Result<(), DocumentError>;
    fn replace_all(&mut self, text: String);
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StringTextStore {
    text: String,
}

impl StringTextStore {
    pub(crate) fn new(text: String) -> Self {
        Self { text }
    }
}

impl TextStore for StringTextStore {
    fn as_str(&self) -> &str {
        &self.text
    }

    fn len_bytes(&self) -> usize {
        self.text.len()
    }

    fn replace(&mut self, range: Range<usize>, replacement: &str) -> Result<(), DocumentError> {
        validate_range(&self.text, &range)?;
        self.text.replace_range(range, replacement);
        Ok(())
    }

    fn replace_all(&mut self, text: String) {
        self.text = text;
    }
}

pub(crate) fn validate_range(text: &str, range: &Range<usize>) -> Result<(), DocumentError> {
    if range.start > range.end {
        return Err(DocumentError::InvalidRange);
    }
    if range.end > text.len() {
        return Err(DocumentError::RangeOutOfBounds);
    }
    if !text.is_char_boundary(range.start) || !text.is_char_boundary(range.end) {
        return Err(DocumentError::InvalidCharBoundary);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_is_failure_atomic_for_invalid_utf8_boundaries() {
        let mut store = StringTextStore::new("a中🙂".to_owned());
        let before = store.clone();
        assert_eq!(
            store.replace(2..3, "x"),
            Err(DocumentError::InvalidCharBoundary)
        );
        assert_eq!(store, before);
    }
}
