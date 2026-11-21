//! The Rust side of the packaging boundary: a lexer that tells comments from literals, and the
//! token-level rules for names of the verification tool, includes, `#[path]`, `cfg_attr` and
//! module declarations.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::{counted_like, resolve};

/// The verification tool's package and crate names: neither may appear in code (comments aside),
/// in a literal or out of one.
const TOOL_NAMES: &[&str] = &["stickymd-smoke", "stickymd_smoke"];

/// A Rust source with its comments removed (`code`: literals kept as written) and, byte for byte
/// aligned with it, the same text with every literal's contents masked by `_` (`tokens`), plus the
/// contents of its string literals as written. A masked run can read as an identifier made of
/// underscores; it never spells a keyword or a macro name.
pub(super) struct Lexed {
    pub(super) code: String,
    pub(super) tokens: String,
    pub(super) strings: Vec<String>,
}

/// Lexes Rust source. Comments (line, nested block) become spaces with newlines kept; string
/// literals (plain, byte, C, raw in each form) and character literals (`'x'`, `b'x'`, escapes)
/// are kept in `code` and masked in `tokens`; a `'` that opens no character literal is read as a
/// lifetime. An unterminated string or block comment is an error.
pub(super) fn lex(text: &str) -> Result<Lexed, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Lexed {
        code: String::with_capacity(text.len()),
        tokens: String::with_capacity(text.len()),
        strings: Vec::new(),
    };
    let keep = |out: &mut Lexed, c: char| {
        out.code.push(c);
        out.tokens.push(c);
    };
    let hide = |out: &mut Lexed, c: char| {
        out.code.push(c);
        if c == '\n' {
            out.tokens.push('\n');
        } else {
            out.tokens.extend(std::iter::repeat_n('_', c.len_utf8()));
        }
    };
    let blank = |out: &mut Lexed, c: char| {
        if c == '\n' {
            out.code.push('\n');
            out.tokens.push('\n');
        } else {
            for _ in 0..c.len_utf8() {
                out.code.push(' ');
                out.tokens.push(' ');
            }
        }
    };
    let identifier = |c: char| c.is_alphanumeric() || c == '_';
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let previous = i.checked_sub(1).map(|p| chars[p]);
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                blank(&mut out, chars[i]);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            loop {
                if i >= chars.len() {
                    return Err("unterminated block comment".to_owned());
                }
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    blank(&mut out, '/');
                    blank(&mut out, '*');
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    blank(&mut out, '*');
                    blank(&mut out, '/');
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut out, chars[i]);
                    i += 1;
                }
            }
        } else if !previous.is_some_and(identifier)
            && let Some((open, hashes)) = raw_string_start(&chars, i)
        {
            let close = (open + 1..chars.len())
                .find(|&k| {
                    chars[k] == '"'
                        && chars.len() > k + hashes
                        && chars[k + 1..=k + hashes].iter().all(|ch| *ch == '#')
                })
                .ok_or("unterminated raw string literal")?;
            for ch in &chars[i..=open] {
                keep(&mut out, *ch);
            }
            out.strings.push(chars[open + 1..close].iter().collect());
            for ch in &chars[open + 1..close] {
                hide(&mut out, *ch);
            }
            for ch in &chars[close..=close + hashes] {
                keep(&mut out, *ch);
            }
            i = close + hashes + 1;
        } else if c == '"' {
            keep(&mut out, c);
            i += 1;
            let mut content = String::new();
            loop {
                let Some(&s) = chars.get(i) else {
                    return Err("unterminated string literal".to_owned());
                };
                i += 1;
                if s == '"' {
                    keep(&mut out, s);
                    break;
                }
                content.push(s);
                hide(&mut out, s);
                if s == '\\'
                    && let Some(&escaped) = chars.get(i)
                {
                    content.push(escaped);
                    hide(&mut out, escaped);
                    i += 1;
                }
            }
            out.strings.push(content);
        } else if c == '\''
            && (!previous.is_some_and(identifier)
                || (previous == Some('b')
                    && !i.checked_sub(2).map(|p| chars[p]).is_some_and(identifier)))
        {
            let end = if next == Some('\\') {
                (i + 3..chars.len().min(i + 12)).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            keep(&mut out, c);
            match end {
                Some(end) => {
                    for ch in &chars[i + 1..end] {
                        hide(&mut out, *ch);
                    }
                    keep(&mut out, '\'');
                    i = end + 1;
                }
                // A lifetime.
                None => i += 1,
            }
        } else {
            keep(&mut out, c);
            i += 1;
        }
    }
    Ok(out)
}

/// Where a raw string literal starting at `i` opens (`r`, `br` or `cr`, then `#`s, then `"`):
/// the index of its `"` and its number of `#`s.
fn raw_string_start(chars: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i;
    if matches!(chars.get(j), Some('b' | 'c')) {
        j += 1;
    }
    if chars.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let mut hashes = 0;
    while chars.get(j) == Some(&'#') {
        hashes += 1;
        j += 1;
    }
    (chars.get(j) == Some(&'"')).then_some((j, hashes))
}

/// Refuses code that names the verification tool, in a literal or not.
pub(super) fn check_names(code: &str) -> Result<(), String> {
    for (index, line) in code.lines().enumerate() {
        for name in TOOL_NAMES {
            if line.contains(name) {
                return Err(format!(
                    "line {} names the verification tool (`{name}`); release bytes may not depend on it",
                    index + 1
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Source {
    /// A file of the packaging crate.
    Packaging,
    /// A member's build script.
    BuildScript,
}

/// Identifier tokens of `tokens`: (start, end, name), a raw identifier (`r#name`) named without
/// its prefix and flagged. Literals are masked, so none is read from inside one.
fn identifiers(tokens: &str) -> Vec<(usize, usize, &str, bool)> {
    let bytes = tokens.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') && (i == 0 || !ident(bytes[i - 1]))
        {
            let start = i;
            let raw = bytes[i] == b'r' && bytes.get(i + 1) == Some(&b'#');
            if raw {
                i += 2;
            }
            let name_start = i;
            while i < bytes.len() && ident(bytes[i]) {
                i += 1;
            }
            found.push((start, i, &tokens[name_start..i], raw));
        } else {
            i += 1;
        }
    }
    found
}

fn skip_space(tokens: &str, at: usize) -> usize {
    at + tokens[at..].len() - tokens[at..].trim_start().len()
}

/// Whether the identifier at `start` is the name of an attribute: `#[name` or `#![name`, with any
/// whitespace (comments are already blank) between the marks.
fn is_attribute(tokens: &str, start: usize) -> bool {
    let Some(rest) = tokens[..start].trim_end().strip_suffix('[') else {
        return false;
    };
    let rest = rest.trim_end();
    let rest = rest.strip_suffix('!').map_or(rest, str::trim_end);
    rest.ends_with('#')
}

/// A file's include and module rules:
/// - no `include!` and no `#[path]`: the packaging crate's Rust is its own `.rs` files, which the
///   check enumerates, and a build script keeps its code in itself;
/// - `include_str!` and `include_bytes!` take a plain string literal naming a path in the Git
///   index exactly as spelled that every platform counting `file` counts (both, for the
///   packaging crate);
/// - `cfg_attr` carries no `path`;
/// - a build script declares no out-of-line module and names no path with a `..` component.
///
/// Macro and attribute names are read as identifiers, a raw identifier (`r#include_str`) by its
/// name, under any path prefix (`core::include_str!`).
pub(super) fn check_includes(
    file: &str,
    lexed: &Lexed,
    source: Source,
    is_tracked: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    let tokens = &lexed.tokens;
    let line_of = |at: usize| tokens[..at].matches('\n').count() + 1;
    let idents = identifiers(tokens);
    for (index, &(start, end, name, raw)) in idents.iter().enumerate() {
        let macro_call = tokens[skip_space(tokens, end)..].starts_with('!');
        match name {
            "include" if macro_call => {
                return Err(format!(
                    "line {}: `include!` brings in Rust by path; keep the code in the crate's own .rs files",
                    line_of(start)
                ));
            }
            "path" if is_attribute(tokens, start) => {
                return Err(format!(
                    "line {}: `#[path]` brings in Rust by path; keep the code in the crate's own .rs files",
                    line_of(start)
                ));
            }
            "cfg_attr" => {
                if let Some(at) = cfg_attr_path(tokens, end) {
                    return Err(format!(
                        "line {}: `cfg_attr` carries a `path`; name modules plainly",
                        line_of(at)
                    ));
                }
            }
            "mod" if !raw && source == Source::BuildScript => {
                let declared = idents
                    .get(index + 1)
                    .is_some_and(|&(next, next_end, _, _)| {
                        skip_space(tokens, end) == next
                            && tokens[skip_space(tokens, next_end)..].starts_with(';')
                    });
                if declared {
                    return Err(format!(
                        "line {}: declares an out-of-line module; keep the code in build.rs",
                        line_of(start)
                    ));
                }
            }
            "include_str" | "include_bytes" if macro_call => {
                let marker = format!("{name}!");
                let bang = skip_space(tokens, end);
                let open = skip_space(tokens, bang + 1);
                let argument = tokens[open..]
                    .starts_with(['(', '[', '{'])
                    .then(|| skip_space(tokens, open + 1))
                    .filter(|&at| lexed.code[at..].starts_with('"'))
                    .and_then(|at| lexed.code[at + 1..].split_once('"'))
                    .map(|(value, _)| value)
                    .ok_or_else(|| {
                        format!(
                            "line {}: `{marker}` takes a plain string literal here",
                            line_of(start)
                        )
                    })?;
                let resolved = resolve(directory, argument).ok_or_else(|| {
                    format!(
                        "line {}: `{marker}` reaches `{argument}` outside the repository",
                        line_of(start)
                    )
                })?;
                if !(is_tracked(&resolved) && counted_like(file, &resolved)) {
                    return Err(format!(
                        "line {}: `{marker}` reaches `{argument}`, which is not a tracked build input of every platform that builds this file, spelled as Git records it",
                        line_of(start)
                    ));
                }
            }
            _ => {}
        }
    }
    if source == Source::BuildScript
        && let Some(literal) = lexed
            .strings
            .iter()
            .find(|literal| literal.split(['/', '\\']).any(|part| part == ".."))
    {
        return Err(format!(
            "names the path {literal:?}, which leaves the member's directory; a build script reads only its own member's files"
        ));
    }
    Ok(())
}

/// Where a `cfg_attr(predicate, attribute, …)` starting after `after` carries a `path = …`
/// attribute, nested `cfg_attr` included; the predicate itself may mention `path`. Unbalanced
/// delimiters end the scan (rustc refuses such code).
fn cfg_attr_path(tokens: &str, after: usize) -> Option<usize> {
    let open = skip_space(tokens, after);
    if !tokens[open..].starts_with('(') {
        return None;
    }
    let mut depth = 0;
    let mut items = Vec::new();
    let mut item_start = open + 1;
    for (offset, byte) in tokens[open..].bytes().enumerate() {
        let at = open + offset;
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    items.push((item_start, at));
                    break;
                }
            }
            b',' if depth == 1 => {
                items.push((item_start, at));
                item_start = at + 1;
            }
            _ => {}
        }
    }
    for &(start, end) in items.iter().skip(1) {
        let item = &tokens[start..end];
        let trimmed = item.trim_start();
        let at = start + item.len() - trimmed.len();
        let name = trimmed.strip_prefix("r#").unwrap_or(trimmed);
        if let Some(rest) = name.strip_prefix("path")
            && rest.trim_start().starts_with('=')
        {
            return Some(at);
        }
        if let Some(rest) = name.strip_prefix("cfg_attr")
            && let Some(nested) = cfg_attr_path(tokens, end - rest.len())
        {
            return Some(nested);
        }
    }
    None
}

/// The PowerShell scripts the packaging crate launches: string literals that name a `*.ps1`
/// (any letter case), which must be a bare file name (its Windows adapter resolves it in
/// `tools/release`).
pub(super) fn scripts_launched(lexed: &Lexed) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for literal in lexed
        .strings
        .iter()
        .filter(|literal| literal.to_ascii_lowercase().contains(".ps1"))
    {
        if !super::is_script_name(literal) {
            return Err(format!(
                "names the script {literal:?} other than by its file name; the packaging crate launches scripts of tools/release by file name"
            ));
        }
        names.push(literal.clone());
    }
    Ok(names)
}
