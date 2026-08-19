//! Pure, platform-free persistence logic for the Phase 1E spike.
//!
//! plan_ref: docs/plan/05_document_persistence.md
//!
//! Everything in this module is deterministic and unit-testable without touching
//! the filesystem or Win32, so the conflict / newline / identity rules can be
//! verified by `cargo test` on any host. The Win32-specific pieces live in
//! `win32.rs`.
//!
//! NOTE: spike code, deletable. Production will re-derive these rules inside the
//! Execution Domain file adapter; nothing here is imported by production crates.

use sha2::{Digest, Sha256};

/// Line-ending style detected from an existing document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewlineStyle {
    Crlf,
    Lf,
}

/// Detect the dominant newline style of `text` (plan 05: majority wins; tie → CRLF).
/// A document with no newlines defaults to CRLF (first-create rule).
pub fn detect_newline_style(text: &str) -> NewlineStyle {
    let mut crlf = 0usize;
    let mut lone_lf = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            crlf += 1;
            i += 2;
        } else if bytes[i] == b'\n' {
            lone_lf += 1;
            i += 1;
        } else {
            i += 1;
        }
    }
    if crlf >= lone_lf {
        NewlineStyle::Crlf
    } else {
        NewlineStyle::Lf
    }
}

/// Internal representation is always `\n`; convert to the target style on save.
#[allow(dead_code)]
pub fn to_internal(text: &str) -> String {
    // Normalize CRLF and stray CR to LF for the in-memory canonical form.
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Convert the internal `\n` form to the on-disk newline style.
pub fn to_disk(text: &str, style: NewlineStyle) -> String {
    match style {
        NewlineStyle::Lf => text.to_string(),
        NewlineStyle::Crlf => text.replace('\n', "\r\n"),
    }
}

/// Strip a UTF-8 BOM if present (plan 05: read tolerates BOM, write omits it).
pub fn strip_bom(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    }
}

/// SHA-256 hex digest of a byte slice (used for disk-hash and identity).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Derive the single-instance identity object name from a canonical directory path.
///
/// `canonical` is expected to already be the resolved NT path (junctions/symlinks
/// expanded, from `GetFinalPathNameByHandleW`). We lowercase because Windows paths
/// are case-insensitive, so `C:\X` and `c:\x` must map to the same identity.
pub fn identity_name(canonical: &str) -> String {
    let normalized = canonical.trim().to_ascii_lowercase();
    let hash = sha256_hex(normalized.as_bytes());
    // Local named-object namespace; keep the full hash to avoid collisions.
    format!("Local\\StickyMD-{hash}")
}

/// The decision taken when an external change to note.md is observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalChangeAction {
    /// The observed hash matches our own last-saved hash → this is our own atomic
    /// replacement echoing back through the watcher. Ignore.
    Ignore,
    /// External change, buffer clean → silently reload.
    Reload,
    /// External change, buffer dirty → conflict; pause autosave, ask the user.
    Conflict,
}

/// Decide how to react to an observed external file change.
///
/// - `observed_hash`: hash of the file the watcher just saw on disk.
/// - `last_saved_hash`: hash of the content WE last wrote (None if never saved).
/// - `buffer_dirty`: whether the in-memory DocumentState has unsaved edits.
pub fn decide_external_change(
    observed_hash: &str,
    last_saved_hash: Option<&str>,
    buffer_dirty: bool,
) -> ExternalChangeAction {
    if let Some(last) = last_saved_hash
        && observed_hash == last {
            return ExternalChangeAction::Ignore;
        }
    if buffer_dirty {
        ExternalChangeAction::Conflict
    } else {
        ExternalChangeAction::Reload
    }
}

/// Startup recovery decision for a leftover `note.md.tmp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// No temp present → nothing to recover.
    None,
    /// Temp present but empty / invalid UTF-8 → discard, use current file.
    DiscardTemp,
    /// Temp present, valid UTF-8, newer than note.md and content differs →
    /// offer recovery to the user (do NOT auto-overwrite).
    OfferRecovery,
    /// Temp present but identical to note.md (or older) → just clean it up.
    CleanStale,
}

/// Decide the startup recovery action for a leftover temp file.
///
/// - `temp`: the temp file bytes, or None if absent.
/// - `current`: the current note.md bytes, or None if absent.
pub fn decide_recovery(temp: Option<&[u8]>, current: Option<&[u8]>) -> RecoveryDecision {
    let Some(temp) = temp else {
        return RecoveryDecision::None;
    };
    // Temp must be valid UTF-8 to be recoverable.
    if core::str::from_utf8(strip_bom(temp)).is_err() {
        return RecoveryDecision::DiscardTemp;
    }
    let temp_hash = sha256_hex(temp);
    match current {
        None => RecoveryDecision::OfferRecovery,
        Some(cur) => {
            if sha256_hex(cur) == temp_hash {
                RecoveryDecision::CleanStale
            } else {
                RecoveryDecision::OfferRecovery
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newline_majority_crlf() {
        assert_eq!(detect_newline_style("a\r\nb\r\nc\n"), NewlineStyle::Crlf);
    }

    #[test]
    fn newline_majority_lf() {
        assert_eq!(detect_newline_style("a\nb\nc\r\n"), NewlineStyle::Lf);
    }

    #[test]
    fn newline_tie_prefers_crlf() {
        assert_eq!(detect_newline_style("a\r\nb\n"), NewlineStyle::Crlf);
        assert_eq!(detect_newline_style(""), NewlineStyle::Crlf);
    }

    #[test]
    fn roundtrip_internal_disk() {
        let internal = to_internal("a\r\nb\r\nc");
        assert_eq!(internal, "a\nb\nc");
        assert_eq!(to_disk(&internal, NewlineStyle::Crlf), "a\r\nb\r\nc");
        assert_eq!(to_disk(&internal, NewlineStyle::Lf), "a\nb\nc");
    }

    #[test]
    fn bom_strip() {
        assert_eq!(strip_bom(&[0xEF, 0xBB, 0xBF, b'x']), b"x");
        assert_eq!(strip_bom(b"x"), b"x");
    }

    #[test]
    fn identity_is_case_insensitive_and_stable() {
        let a = identity_name(r"\\?\C:\Users\Me\StickyMD");
        let b = identity_name(r"\\?\c:\users\me\stickymd");
        assert_eq!(a, b);
        let c = identity_name(r"\\?\D:\other");
        assert_ne!(a, c);
        assert!(a.starts_with("Local\\StickyMD-"));
    }

    #[test]
    fn own_write_is_ignored() {
        let h = "abc";
        assert_eq!(
            decide_external_change(h, Some(h), true),
            ExternalChangeAction::Ignore
        );
        assert_eq!(
            decide_external_change(h, Some(h), false),
            ExternalChangeAction::Ignore
        );
    }

    #[test]
    fn external_change_clean_reloads() {
        assert_eq!(
            decide_external_change("new", Some("old"), false),
            ExternalChangeAction::Reload
        );
        assert_eq!(
            decide_external_change("new", None, false),
            ExternalChangeAction::Reload
        );
    }

    #[test]
    fn external_change_dirty_conflicts() {
        assert_eq!(
            decide_external_change("new", Some("old"), true),
            ExternalChangeAction::Conflict
        );
        assert_eq!(
            decide_external_change("new", None, true),
            ExternalChangeAction::Conflict
        );
    }

    #[test]
    fn recovery_none_without_temp() {
        assert_eq!(decide_recovery(None, Some(b"cur")), RecoveryDecision::None);
    }

    #[test]
    fn recovery_discards_invalid_utf8_temp() {
        assert_eq!(
            decide_recovery(Some(&[0xFF, 0xFE, 0xFD]), Some(b"cur")),
            RecoveryDecision::DiscardTemp
        );
    }

    #[test]
    fn recovery_offers_when_temp_differs() {
        assert_eq!(
            decide_recovery(Some(b"newer"), Some(b"cur")),
            RecoveryDecision::OfferRecovery
        );
        assert_eq!(decide_recovery(Some(b"only"), None), RecoveryDecision::OfferRecovery);
    }

    #[test]
    fn recovery_cleans_stale_identical_temp() {
        assert_eq!(
            decide_recovery(Some(b"same"), Some(b"same")),
            RecoveryDecision::CleanStale
        );
    }
}
