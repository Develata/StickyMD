//! StickyMD platform-independent core.
//!
//! plan_ref: docs/plan/03_system_architecture.md#object-plane
//!
//! Phase 1 status: package skeleton only. This crate holds the future
//! platform-independent document model, persistence contracts and state
//! model. It must never depend on Windows APIs, windowing, rendering or
//! any UI crate.
#![forbid(unsafe_code)]

pub mod error;

use std::fmt;

/// Monotonic document version.
///
/// plan_ref: docs/plan/04_runtime_state_model.md#generation-semantics统一规则
///
/// Every text mutation increments the generation. Background results carry
/// the generation they were computed from; stale results must be dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    pub const fn initial() -> Self {
        Self(0)
    }

    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Line ending style recorded for a document on disk.
///
/// plan_ref: docs/plan/05_document_persistence.md#文本编码与换行
///
/// Internal text is always UTF-8 + `\n`; conversion happens at save time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// Windows default; used for first creation and for mixed files with
    /// equal CRLF/LF counts.
    #[default]
    Crlf,
    Lf,
}

impl LineEnding {
    pub const fn as_str(self) -> &'static str {
        match self {
            LineEnding::Crlf => "\r\n",
            LineEnding::Lf => "\n",
        }
    }

    /// Detect the dominant line ending of a document body.
    /// Equal counts resolve to CRLF per the persistence contract.
    pub fn detect(text: &str) -> Self {
        let mut crlf = 0usize;
        let mut lf = 0usize;
        let bytes = text.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'\n' {
                if i > 0 && bytes[i - 1] == b'\r' {
                    crlf += 1;
                } else {
                    lf += 1;
                }
            }
            i += 1;
        }
        if lf > crlf {
            LineEnding::Lf
        } else {
            LineEnding::Crlf
        }
    }

    /// Convert internal `\n`-normalized text to this line ending style.
    pub fn apply(self, normalized: &str) -> String {
        match self {
            LineEnding::Lf => normalized.replace("\r\n", "\n"),
            LineEnding::Crlf => {
                let without_cr = normalized.replace("\r\n", "\n");
                without_cr.replace('\n', "\r\n")
            }
        }
    }
}

impl fmt::Display for LineEnding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LineEnding::Crlf => "crlf",
            LineEnding::Lf => "lf",
        })
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
    fn line_ending_detection_majority_wins() {
        assert_eq!(LineEnding::detect("a\r\nb\r\nc\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("a\nb\nc\r\n"), LineEnding::Lf);
        // Equal counts -> CRLF per contract.
        assert_eq!(LineEnding::detect("a\r\nb\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("no newlines"), LineEnding::Crlf);
    }

    #[test]
    fn line_ending_roundtrip() {
        let text = "line1\nline2\n";
        assert_eq!(LineEnding::Crlf.apply(text), "line1\r\nline2\r\n");
        assert_eq!(LineEnding::Lf.apply(text), "line1\nline2\n");
        // Mixed input is normalized first.
        assert_eq!(LineEnding::Lf.apply("a\r\nb\n"), "a\nb\n");
    }
}
