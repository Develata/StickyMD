//! Line-ending style recorded for a document on disk.
//!
//! plan_ref: docs/plan/05_document_persistence.md#text-encoding-newlines
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

    /// Normalize CRLF to the internal `\n` form while preserving isolated CR.
    ///
    /// Isolated carriage returns are user content, not line-ending evidence.
    pub fn to_internal(text: &str) -> String {
        text.replace("\r\n", "\n")
    }

    /// Convert internal `\n`-normalized text to this line ending style.
    pub fn apply(self, normalized: &str) -> String {
        let replacement = self.as_str();
        let bytes = normalized.as_bytes();
        let extra = if self == LineEnding::Crlf {
            bytes.iter().filter(|byte| **byte == b'\n').count()
        } else {
            0
        };
        let mut output = String::with_capacity(normalized.len() + extra);
        let mut segment = 0usize;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte != b'\n' {
                continue;
            }
            // Canonical text uses `\n` as its only line-break token. A `\r`
            // immediately before it is still ordinary user content and must
            // not be consumed as if this were an unnormalized durable CRLF.
            output.push_str(&normalized[segment..index]);
            output.push_str(replacement);
            segment = index + 1;
        }
        output.push_str(&normalized[segment..]);
        output
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
        // `apply` receives canonical text. An adjacent carriage return is
        // content, not a second durable newline representation.
        assert_eq!(LineEnding::Lf.apply("a\r\nb\n"), "a\r\nb\n");
    }

    #[test]
    fn to_internal_preserves_isolated_carriage_return() {
        assert_eq!(LineEnding::to_internal("a\r\nb\rc\n"), "a\nb\rc\n");
    }

    #[test]
    fn adjacent_newline_does_not_consume_an_isolated_carriage_return() {
        let runtime = "a\r\nb";
        assert_eq!(LineEnding::Lf.apply(runtime), "a\r\nb");
        assert_eq!(LineEnding::Crlf.apply(runtime), "a\r\r\nb");
    }
}
