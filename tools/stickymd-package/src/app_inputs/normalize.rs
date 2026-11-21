//! Release-version normalization for functional fingerprints.
//!
//! A release bump rewrites the workspace version in `Cargo.toml` and the workspace
//! packages' `version` lines in `Cargo.lock`. Functional modules must not become stale
//! for that label alone; exact-byte gates, Source Freeze and the workspace-tests identity
//! keep raw bytes. Anything outside the narrow grammar below falls back to raw bytes, so
//! an unrecognized manifest can only cause a rerun, never a false reuse.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::borrow::Cow;
use std::fs;
use std::path::Path;

pub(super) const ROOT_MANIFEST: &str = "Cargo.toml";
pub(super) const LOCKFILE: &str = "Cargo.lock";

/// The normalized forms of the root manifest and the lockfile, decided once per
/// planning pass for the whole set: either every manifest and the lockfile are inside
/// the grammar and agree on the workspace version, or nothing is normalized.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct VersionNormalizer {
    root: Normalized,
    lock: Normalized,
}

/// The bytes read during planning and their normalized form.
#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    raw: Vec<u8>,
    normalized: Vec<u8>,
}

impl VersionNormalizer {
    /// `None` unless the root manifest, every workspace member manifest and the tracked
    /// lockfile are all inside the grammar, every member inherits one quoted workspace
    /// version, and the lockfile records exactly that version for each member.
    pub(super) fn read(root: &Path, tracked: &[String]) -> Option<Self> {
        let is_tracked = |relative: &str| tracked.iter().any(|path| path == relative);
        if !is_tracked(ROOT_MANIFEST) || !is_tracked(LOCKFILE) {
            return None;
        }
        let root_raw = fs::read(root.join(ROOT_MANIFEST)).ok()?;
        let manifest = std::str::from_utf8(&root_raw).ok()?;
        let workspace = parse_root(manifest)?;
        let mut members = Vec::with_capacity(workspace.members.len());
        for member in &workspace.members {
            let relative = format!("{member}/Cargo.toml");
            if !is_tracked(&relative) {
                return None;
            }
            let text = fs::read_to_string(root.join(&relative)).ok()?;
            members.push(parse_member(&text)?);
        }
        members.sort();
        if members.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        let lock_raw = fs::read(root.join(LOCKFILE)).ok()?;
        let lock = std::str::from_utf8(&lock_raw).ok()?;
        let lock_normalized = lock_without_member_versions(lock, &workspace.version, &members)?;
        let root_normalized = remove_lines(manifest, &[workspace.version_line]);
        Some(Self {
            root: Normalized {
                raw: root_raw,
                normalized: root_normalized.into_bytes(),
            },
            lock: Normalized {
                raw: lock_raw,
                normalized: lock_normalized.into_bytes(),
            },
        })
    }

    pub(super) fn applies_to(path: &str) -> bool {
        path == ROOT_MANIFEST || path == LOCKFILE
    }

    /// The normalized form of `bytes`, provided they are exactly the bytes judged during
    /// planning; anything else (including a file rewritten since) stays raw.
    pub(super) fn normalize<'a>(&'a self, path: &str, bytes: &'a [u8]) -> Cow<'a, [u8]> {
        let file = match path {
            ROOT_MANIFEST => &self.root,
            LOCKFILE => &self.lock,
            _ => return Cow::Borrowed(bytes),
        };
        if bytes == file.raw.as_slice() {
            Cow::Borrowed(&file.normalized)
        } else {
            Cow::Borrowed(bytes)
        }
    }
}

/// The lockfile without the `version` lines of workspace members, which must each
/// appear exactly once with `version`. Keys are accepted once each; TOML forbids
/// redefinition, so a repeated key is outside the grammar rather than "last one wins".
fn lock_without_member_versions(text: &str, version: &str, members: &[String]) -> Option<String> {
    if has_multiline_string(text) {
        return None;
    }
    let mut remove = Vec::new();
    let mut seen = vec![false; members.len()];
    let mut block: Option<LockBlock> = None;
    let mut format_version = false;
    let mut in_array = false;
    for (index, line) in lines(text).enumerate() {
        let trimmed = line.trim();
        if in_array {
            // Dependency entries name a member without a version unless Cargo had
            // to disambiguate; a versioned member reference is outside the grammar.
            if members
                .iter()
                .any(|member| trimmed.starts_with(&format!("\"{member} ")))
            {
                return None;
            }
            if trimmed.starts_with(']') {
                in_array = false;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            if let Some(finished) = block.take() {
                accept_block(finished, version, members, &mut seen, &mut remove)?;
            }
            if trimmed != "[[package]]" {
                return None;
            }
            block = Some(LockBlock::default());
            continue;
        }
        let (key, value) = trimmed.split_once('=')?;
        let (key, value) = (key.trim(), value.trim());
        let Some(current) = block.as_mut() else {
            // The only top-level key is the lockfile format version, defined once.
            if key != "version"
                || format_version
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            format_version = true;
            continue;
        };
        match key {
            "name" => current.name = Some(set_once(current.name.take(), quoted(value)?)?),
            "version" => {
                current.version = Some(set_once(current.version.take(), quoted(value)?)?);
                current.version_line = Some(index);
            }
            "source" => current.source = Some(set_once(current.source.take(), ())?),
            "checksum" => current.checksum = Some(set_once(current.checksum.take(), ())?),
            "dependencies" => {
                current.dependencies = Some(set_once(current.dependencies.take(), ())?);
                if value == "[" {
                    in_array = true;
                } else if value != "[]" {
                    return None;
                }
            }
            _ => return None,
        }
    }
    if in_array || !format_version {
        return None;
    }
    if let Some(finished) = block.take() {
        accept_block(finished, version, members, &mut seen, &mut remove)?;
    }
    if seen.iter().any(|found| !found) {
        return None;
    }
    Some(remove_lines(text, &remove))
}

/// Record one finished `[[package]]` block; `members` is sorted.
fn accept_block(
    block: LockBlock,
    version: &str,
    members: &[String],
    seen: &mut [bool],
    remove: &mut Vec<usize>,
) -> Option<()> {
    let name = block.name?;
    block.version.as_ref()?;
    let Ok(position) = members.binary_search(&name) else {
        return Some(());
    };
    if block.source.is_some() || seen[position] || block.version.as_deref() != Some(version) {
        return None;
    }
    seen[position] = true;
    remove.push(block.version_line?);
    Some(())
}

#[derive(Default)]
struct LockBlock {
    name: Option<String>,
    version: Option<String>,
    version_line: Option<usize>,
    source: Option<()>,
    checksum: Option<()>,
    dependencies: Option<()>,
}

struct RootManifest {
    version: String,
    version_line: usize,
    members: Vec<String>,
}

fn parse_root(text: &str) -> Option<RootManifest> {
    let mut version = None;
    let mut members = None;
    for entry in entries(text)? {
        // Dotted or inline spellings of the same facts are outside the grammar.
        if entry.key.contains("package")
            || (entry.table.is_empty() && entry.key.starts_with("workspace"))
        {
            return None;
        }
        match (entry.table.as_str(), entry.key.as_str()) {
            ("workspace", "members") => {
                let inner = entry.value.strip_prefix('[')?.strip_suffix(']')?;
                let mut items = Vec::new();
                push_array_items(inner, &mut items)?;
                members = Some(set_once(members.take(), items)?);
            }
            ("workspace.package", "version") if entry.lines == 1 => {
                version = Some(set_once(
                    version.take(),
                    (quoted(&entry.value)?, entry.line),
                )?);
            }
            ("workspace.package", other) if other.starts_with("version") => return None,
            _ => {}
        }
    }
    let (version, version_line) = version?;
    let members: Vec<String> = members?;
    (!members.is_empty()).then_some(RootManifest {
        version,
        version_line,
        members,
    })
}

/// A member's package name, provided it inherits the workspace version exactly once.
fn parse_member(text: &str) -> Option<String> {
    let mut name = None;
    let mut inherits = false;
    for entry in entries(text)?
        .into_iter()
        .filter(|entry| entry.table == "package")
    {
        match entry.key.as_str() {
            "name" => name = Some(set_once(name.take(), quoted(&entry.value)?)?),
            "version.workspace" if entry.value == "true" && !inherits => inherits = true,
            other if other.starts_with("version") => return None,
            _ => {}
        }
    }
    inherits.then_some(name?)
}

/// One `key = value` assignment; a value may continue over lines inside brackets.
struct Entry {
    table: String,
    key: String,
    value: String,
    line: usize,
    lines: usize,
}

/// Split a manifest into table-qualified assignments. Multi-line strings, unbalanced
/// brackets and lines that are neither headers, assignments nor continuations are
/// outside the grammar.
fn entries(text: &str) -> Option<Vec<Entry>> {
    if has_multiline_string(text) {
        return None;
    }
    let mut table = String::new();
    let mut output: Vec<Entry> = Vec::new();
    let mut depth = 0_i32;
    for (index, line) in lines(text).enumerate() {
        let content = strip_comment(line).trim();
        if depth > 0 {
            let entry = output.last_mut()?;
            entry.value.push(' ');
            entry.value.push_str(content);
            entry.lines += 1;
            depth += bracket_delta(content);
            if depth < 0 {
                return None;
            }
            continue;
        }
        if content.is_empty() {
            continue;
        }
        if content.starts_with('[') {
            table = table_name(content)?;
            continue;
        }
        let (key, value) = content.split_once('=')?;
        let value = value.trim();
        depth = bracket_delta(value);
        if depth < 0 {
            return None;
        }
        output.push(Entry {
            table: table.clone(),
            key: key.trim().to_owned(),
            value: value.to_owned(),
            line: index,
            lines: 1,
        });
    }
    (depth == 0).then_some(output)
}

/// Net opening brackets outside basic and literal strings.
fn bracket_delta(content: &str) -> i32 {
    let mut delta = 0;
    let mut quote = None;
    let mut escaped = false;
    for byte in content.bytes() {
        match quote {
            Some(b'"') if escaped => escaped = false,
            Some(b'"') if byte == b'\\' => escaped = true,
            Some(open) if byte == open => quote = None,
            Some(_) => {}
            None => match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'[' | b'{' => delta += 1,
                b']' | b'}' => delta -= 1,
                _ => {}
            },
        }
    }
    delta
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// Remove whole lines (with their terminators) by index.
fn remove_lines(text: &str, indexes: &[usize]) -> String {
    let mut output = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if !indexes.contains(&index) {
            output.push_str(line);
        }
    }
    output
}

fn has_multiline_string(text: &str) -> bool {
    text.contains("\"\"\"") || text.contains("'''")
}

/// Quoted scalars here never contain `#`, so a comment starts at the first `#`
/// outside a basic string.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b'"' => in_string = !in_string,
            b'#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

/// A table header's dotted key, e.g. `package`, `[bench]` arrays, or
/// `target.'cfg(windows)'.dependencies`. Segments are bare keys or quoted keys without
/// escapes; the only tables this module interprets are bare (`workspace`,
/// `workspace.package`, `package`), so quoted segments only need to be well formed.
fn table_name(content: &str) -> Option<String> {
    let inner = content.strip_prefix('[')?.strip_suffix(']')?;
    let inner = inner
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(inner);
    let name = inner.trim();
    let mut rest = name;
    loop {
        rest = rest.trim_start();
        let after_segment = match rest.as_bytes().first()? {
            quote @ (b'"' | b'\'') => {
                let body = &rest[1..];
                let end = body.find(*quote as char)?;
                if end == 0 || body[..end].contains('\\') {
                    return None;
                }
                &body[end + 1..]
            }
            _ => {
                let end = rest
                    .find(|character: char| {
                        !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
                    })
                    .unwrap_or(rest.len());
                if end == 0 {
                    return None;
                }
                &rest[end..]
            }
        };
        let after_segment = after_segment.trim_start();
        if after_segment.is_empty() {
            return Some(name.to_owned());
        }
        rest = after_segment.strip_prefix('.')?;
    }
}

/// A basic quoted scalar with no escapes, followed by nothing.
fn quoted(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.is_empty() && !inner.contains(['"', '\\'])).then(|| inner.to_owned())
}

fn push_array_items(body: &str, items: &mut Vec<String>) -> Option<()> {
    for item in body.split(',') {
        let item = item.trim();
        if !item.is_empty() {
            items.push(quoted(item)?);
        }
    }
    Some(())
}

fn set_once<T>(previous: Option<T>, value: T) -> Option<T> {
    previous.is_none().then_some(value)
}

#[cfg(test)]
mod tests;
