//! Line-ending style recorded for a document on disk.
//!
//! plan_ref: docs/plan/05_document_persistence.md#文本编码与换行
//!
//! Internal text is always UTF-8 + `\n`; conversion to the recorded style
//! happens only at save time. Detection follows the persistence contract:
//! majority wins, and a tie resolves to CRLF.

use std::fmt;

/// Line ending style recorded for a document on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineEnding {
    /// Windows default; used for first creation and for mixed files with
    /// equal CRLF/LF counts.
    #[default]
    Crlf,
    Lf,
}

impl LineEnding {
    /// The on-disk byte sequence for a single line break.
    pub const fn as_str(self) -> &'static str {
        match self {
            LineEnding::Crlf => "\r\n",
            LineEnding::Lf => "\n",
        }
    }

    /// Detect the dominant line ending of a document body. Equal counts resolve
    /// to CRLF per the persistence contract.
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

    /// Normalize `text` to the internal `\n` form (CRLF and stray CR → LF).
    pub fn to_internal(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
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

    #[test]
    fn to_internal_normalizes_all_cr_forms() {
        assert_eq!(LineEnding::to_internal("a\r\nb\rc\n"), "a\nb\nc\n");
    }
}
