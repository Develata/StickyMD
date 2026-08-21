//! Managed-image names, conservative reference counts, and pure asset effects.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Range;

/// Longest valid managed filename in UTF-8 bytes.
pub const MAX_MANAGED_ASSET_NAME_BYTES: usize = 9 + 64 + 5;

/// Extension of a StickyMD-managed image. The spelling is canonical and
/// deliberately excludes aliases such as `jpeg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ManagedAssetExtension {
    Png,
    Jpg,
    Webp,
    Gif,
}

impl ManagedAssetExtension {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "png" => Some(Self::Png),
            "jpg" => Some(Self::Jpg),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }
}

/// Strict basename used by conservative reference tracking. Parsing this
/// value proves only syntax; filesystem ownership requires the adapter's
/// separate location, reparse-point, and content-hash checks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ManagedAssetName(String);

impl ManagedAssetName {
    pub fn parse(value: &str) -> Option<Self> {
        let (stem, extension) = value.rsplit_once('.')?;
        ManagedAssetExtension::parse(extension)?;
        let hash = stem.strip_prefix("stickymd-")?;
        if !matches!(hash.len(), 20 | 32 | 64)
            || !hash
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return None;
        }
        Some(Self(value.to_owned()))
    }

    pub fn from_hash_prefix(
        full_hash_hex: &str,
        prefix_len: usize,
        extension: ManagedAssetExtension,
    ) -> Option<Self> {
        if full_hash_hex.len() != 64
            || !matches!(prefix_len, 20 | 32 | 64)
            || !full_hash_hex
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return None;
        }
        Self::parse(&format!(
            "stickymd-{}.{}",
            &full_hash_hex[..prefix_len],
            extension.as_str()
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn hash_prefix(&self) -> &str {
        let without_prefix = &self.0["stickymd-".len()..];
        without_prefix.split_once('.').map_or("", |(hash, _)| hash)
    }

    pub fn extension(&self) -> ManagedAssetExtension {
        let extension = self.0.rsplit_once('.').map_or("", |(_, ext)| ext);
        // Construction is private and parse validates the extension.
        ManagedAssetExtension::parse(extension).unwrap_or(ManagedAssetExtension::Png)
    }
}

impl fmt::Display for ManagedAssetName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ManagedAssetLocation {
    Images,
    Trash,
}

/// Pure filesystem intent coupled to a text history entry. The core never
/// executes it; the asset coordinator proves ownership immediately before I/O.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetEffect {
    pub name: ManagedAssetName,
    pub from: ManagedAssetLocation,
    pub to: ManagedAssetLocation,
}

impl AssetEffect {
    pub fn reversed(&self) -> Self {
        Self {
            name: self.name.clone(),
            from: self.to,
            to: self.from,
        }
    }

    pub(crate) fn approx_bytes(&self) -> usize {
        self.name.0.len().saturating_add(32)
    }
}

/// Conservative literal scanner. It deliberately counts valid managed names
/// in code blocks, raw HTML, and ordinary text so parser ambiguity can retain
/// files but can never cause automatic data loss.
pub fn scan_managed_asset_references(text: &str) -> HashMap<ManagedAssetName, usize> {
    scan_bytes(text.as_bytes())
}

pub(crate) fn reference_changes_after_replace(
    text: &str,
    range: &Range<usize>,
    inserted: &str,
    current: &HashMap<ManagedAssetName, usize>,
) -> (Vec<ReferenceCountChange>, Vec<AssetEffect>) {
    let start = floor_char_boundary(
        text,
        range.start.saturating_sub(MAX_MANAGED_ASSET_NAME_BYTES),
    );
    let end = ceil_char_boundary(
        text,
        range
            .end
            .saturating_add(MAX_MANAGED_ASSET_NAME_BYTES)
            .min(text.len()),
    );
    let old_local = scan_managed_asset_references(&text[start..end]);

    let mut replacement =
        String::with_capacity(range.start - start + inserted.len() + end.saturating_sub(range.end));
    replacement.push_str(&text[start..range.start]);
    replacement.push_str(inserted);
    replacement.push_str(&text[range.end..end]);
    let new_local = scan_managed_asset_references(&replacement);

    let mut touched = HashSet::with_capacity(old_local.len().saturating_add(new_local.len()));
    touched.extend(old_local.keys().cloned());
    touched.extend(new_local.keys().cloned());

    let mut effects = Vec::new();
    let mut changes = Vec::with_capacity(touched.len());
    for name in touched {
        let before = current.get(&name).copied().unwrap_or(0);
        let removed = old_local.get(&name).copied().unwrap_or(0);
        let added = new_local.get(&name).copied().unwrap_or(0);
        let after = before.saturating_sub(removed).saturating_add(added);
        if before != after {
            changes.push(ReferenceCountChange {
                name: name.clone(),
                count: after,
            });
        }
        match (before, after) {
            (0, count) if count > 0 => effects.push(AssetEffect {
                name,
                from: ManagedAssetLocation::Trash,
                to: ManagedAssetLocation::Images,
            }),
            (count, 0) if count > 0 => effects.push(AssetEffect {
                name,
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            }),
            _ => {}
        }
    }
    changes.sort_by(|left, right| left.name.cmp(&right.name));
    effects.sort_by(|left, right| left.name.cmp(&right.name));
    (changes, effects)
}

/// One non-fallible count update prepared from the pre-edit canonical text.
/// Keeping only touched names makes ordinary edits O(local-window + changed
/// names), rather than cloning every distinct asset reference on each keypress.
pub(crate) struct ReferenceCountChange {
    name: ManagedAssetName,
    count: usize,
}

pub(crate) fn apply_reference_count_changes(
    current: &mut HashMap<ManagedAssetName, usize>,
    changes: Vec<ReferenceCountChange>,
) {
    for change in changes {
        if change.count == 0 {
            current.remove(&change.name);
        } else {
            current.insert(change.name, change.count);
        }
    }
}

fn scan_bytes(bytes: &[u8]) -> HashMap<ManagedAssetName, usize> {
    let marker = b"stickymd-";
    let mut counts = HashMap::new();
    let mut cursor: usize = 0;
    while cursor.saturating_add(marker.len()) <= bytes.len() {
        let Some(relative) = bytes[cursor..]
            .windows(marker.len())
            .position(|window| window == marker)
        else {
            break;
        };
        let start = cursor + relative;
        let mut matched = None;
        for hash_len in [64usize, 32, 20] {
            let hash_end = start + marker.len() + hash_len;
            if hash_end > bytes.len()
                || !bytes[start + marker.len()..hash_end]
                    .iter()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
                || bytes.get(hash_end) != Some(&b'.')
            {
                continue;
            }
            for extension in [b"webp".as_slice(), b"png", b"jpg", b"gif"] {
                let end = hash_end + 1 + extension.len();
                if bytes.get(hash_end + 1..end) == Some(extension) {
                    matched = std::str::from_utf8(&bytes[start..end])
                        .ok()
                        .and_then(ManagedAssetName::parse)
                        .map(|name| (name, end));
                    break;
                }
            }
            if matched.is_some() {
                break;
            }
        }
        if let Some((name, end)) = matched {
            *counts.entry(name).or_insert(0) += 1;
            cursor = end;
        } else {
            cursor = start + marker.len();
        }
    }
    counts
}

fn floor_char_boundary(text: &str, mut byte: usize) -> usize {
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn ceil_char_boundary(text: &str, mut byte: usize) -> usize {
    while byte < text.len() && !text.is_char_boundary(byte) {
        byte += 1;
    }
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH20: &str = "0123456789abcdef0123";

    #[test]
    fn strict_name_parser_rejects_lookalikes() {
        for valid in [
            format!("stickymd-{HASH20}.png"),
            format!("stickymd-{}.jpg", "a".repeat(32)),
            format!("stickymd-{}.webp", "f".repeat(64)),
        ] {
            assert!(ManagedAssetName::parse(&valid).is_some(), "{valid}");
        }
        for invalid in [
            format!("stickymd-{HASH20}.jpeg"),
            format!("stickymd-{}.png", "A".repeat(20)),
            format!("stickymd-{}.png", "a".repeat(21)),
            format!("x/stickymd-{HASH20}.png"),
        ] {
            assert!(ManagedAssetName::parse(&invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn conservative_scanner_counts_literals_in_every_markdown_context() {
        let name = format!("stickymd-{HASH20}.png");
        let source = format!("![](images/{name})\n`{name}`\n<!-- {name} -->");
        assert_eq!(
            scan_managed_asset_references(&source)
                .values()
                .copied()
                .sum::<usize>(),
            3
        );
    }

    #[test]
    fn incremental_window_detects_name_formed_across_edit_boundary() {
        let name = format!("stickymd-{HASH20}.png");
        let before = &name[..name.len() - 1];
        let current = scan_managed_asset_references(before);
        let (changes, effects) =
            reference_changes_after_replace(before, &(before.len()..before.len()), "g", &current);
        let mut next = current;
        apply_reference_count_changes(&mut next, changes);
        assert_eq!(next.values().copied().sum::<usize>(), 1);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].to, ManagedAssetLocation::Images);
    }

    #[test]
    fn incremental_window_handles_cjk_without_invalid_slicing() {
        let name = format!("stickymd-{HASH20}.gif");
        let text = format!("前缀🙂{name}后缀");
        let start = text.find(&name).unwrap();
        let current = scan_managed_asset_references(&text);
        let (changes, effects) =
            reference_changes_after_replace(&text, &(start..start + name.len()), "", &current);
        let mut next = current;
        apply_reference_count_changes(&mut next, changes);
        assert!(next.is_empty());
        assert_eq!(effects[0].to, ManagedAssetLocation::Trash);
    }

    #[test]
    #[ignore = "Release-only Phase 7 managed-reference scan timing baseline"]
    fn phase7_managed_scan_release_baseline() {
        use std::time::Instant;

        let name = format!("stickymd-{HASH20}.png");
        let mut document = String::new();
        while document.len() < 1024 * 1024 {
            document.push_str("普通 Markdown 文本 and code `literal`\n");
            if document.len() % 8192 < 64 {
                document.push_str(&format!("![](images/{name})\n"));
            }
        }
        let current = scan_managed_asset_references(&document);
        let edit_at = document.len();
        let mut full = Vec::new();
        let mut incremental = Vec::new();
        for _ in 0..30 {
            let started = Instant::now();
            let rescanned = scan_managed_asset_references(&document);
            full.push(started.elapsed());
            assert_eq!(rescanned, current);
            let started = Instant::now();
            let (changes, _) =
                reference_changes_after_replace(&document, &(edit_at..edit_at), "x", &current);
            incremental.push(started.elapsed());
            assert!(changes.is_empty());
        }
        full.sort_unstable();
        incremental.sort_unstable();
        println!(
            "phase7 managed scan 1MiB full median_us={} p95_us={} max_us={} incremental median_us={} p95_us={} max_us={}",
            full[15].as_micros(),
            full[28].as_micros(),
            full[29].as_micros(),
            incremental[15].as_micros(),
            incremental[28].as_micros(),
            incremental[29].as_micros(),
        );
    }
}
