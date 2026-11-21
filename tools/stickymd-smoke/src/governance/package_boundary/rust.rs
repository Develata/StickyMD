//! The Rust side of the packaging boundary: a lexer that tells comments from literals, and the
//! token-level rules for names of the verification tool, includes, `#[path]`, `cfg_attr` and
//! module declarations.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::{CRATE, counted_by_both, resolve};

/// How Rust code names the verification tool: its package, its crate and its directory.
const TOOL_NAMES: &[&str] = &[
    "\"stickymd-smoke\"",
    "-p stickymd-smoke",
    "stickymd_smoke",
    "stickymd-smoke/",
];

/// A Rust source with its comments removed (`code`: literals kept as written) and, byte for byte
/// aligned with it, the same text with every literal's contents masked (`tokens`), plus the
/// contents of its string literals as written.
pub(super) struct Lexed {
    pub(super) code: String,
    pub(super) tokens: String,
    pub(super) strings: Vec<String>,
}

/// Lexes Rust source. Comments (line, nested block) become spaces with newlines kept; string
/// literals (plain, byte, C, raw in each form) and character literals (`'x'`, `b'x'`, escapes)
/// are kept in `code` and masked in `tokens`; a lifetime stays code. An unterminated literal or
/// comment is an error.
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

/// Refuses code that names the verification tool.
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
    /// A `.rs` file of the packaging crate: its Rust includes stay in the crate.
    Packaging,
    /// A packaging-crate file reached only by `include!` or `#[path]` (any extension): as above,
    /// and no out-of-line module, whose file would not be enumerated.
    Fragment,
    /// A member's build script: self-contained, data includes only.
    BuildScript,
}

/// Identifier tokens of `tokens`: (start, end, text), raw identifiers (`r#name`) with their
/// prefix. Literals are masked, so no identifier is read from inside one.
fn identifiers(tokens: &str) -> Vec<(usize, usize, &str)> {
    let bytes = tokens.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') && (i == 0 || !ident(bytes[i - 1]))
        {
            let start = i;
            if bytes[i] == b'r' && bytes.get(i + 1) == Some(&b'#') {
                i += 2;
            }
            while i < bytes.len() && ident(bytes[i]) {
                i += 1;
            }
            found.push((start, i, &tokens[start..i]));
        } else {
            i += 1;
        }
    }
    found
}

fn skip_space(tokens: &str, at: usize) -> usize {
    at + tokens[at..].len() - tokens[at..].trim_start().len()
}

/// The include and `#[path]` targets of a file, each checked, and its other module rules:
/// - `include!` and `#[path]` stay inside the packaging crate (none in a build script);
/// - `include_str!` and `include_bytes!` name a path in the Git index exactly as spelled that
///   both platforms count;
/// - each argument is a plain string literal;
/// - `cfg_attr` carries no `path`; a build script or fragment declares no out-of-line module.
///
/// Returns the Rust files (`include!`, `#[path]`) the file reaches, to be checked in turn.
pub(super) fn check_includes(
    file: &str,
    lexed: &Lexed,
    source: Source,
    is_tracked: &dyn Fn(&str) -> bool,
) -> Result<Vec<String>, String> {
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    let tokens = &lexed.tokens;
    let line_of = |at: usize| tokens[..at].matches('\n').count() + 1;
    let mut reached = Vec::new();
    let idents = identifiers(tokens);
    for (index, &(start, end, name)) in idents.iter().enumerate() {
        let marker = match name {
            "include" | "include_str" | "include_bytes" => {
                let bang = skip_space(tokens, end);
                if tokens[bang..].starts_with('!') {
                    let open = skip_space(tokens, bang + 1);
                    Some((format!("{name}!"), open, true))
                } else {
                    None
                }
            }
            "path" => {
                // `#[path = "…"]`: the identifier right after `#` and `[`.
                let before = tokens[..start].trim_end();
                let attribute = before.strip_suffix('[').is_some_and(|rest| {
                    rest.trim_end().ends_with('#') || rest.trim_end().ends_with("#!")
                });
                attribute.then(|| ("#[path".to_owned(), skip_space(tokens, end), false))
            }
            "cfg_attr" => {
                if let Some(at) = cfg_attr_path(tokens, end) {
                    return Err(format!(
                        "line {}: `cfg_attr` carries a `path`; name modules plainly",
                        line_of(at)
                    ));
                }
                None
            }
            "mod" if source != Source::Packaging => {
                let declared = idents.get(index + 1).is_some_and(|&(next, next_end, _)| {
                    skip_space(tokens, end) == next
                        && tokens[skip_space(tokens, next_end)..].starts_with(';')
                });
                if declared {
                    return Err(format!(
                        "line {}: declares an out-of-line module; keep the code in this file",
                        line_of(start)
                    ));
                }
                None
            }
            _ => None,
        };
        let Some((marker, open, call)) = marker else {
            continue;
        };
        // A call opens with a delimiter; an attribute with `=`.
        let literal = if call {
            tokens[open..]
                .starts_with(['(', '[', '{'])
                .then(|| skip_space(tokens, open + 1))
        } else {
            tokens[open..]
                .starts_with('=')
                .then(|| skip_space(tokens, open + 1))
        };
        let argument = literal
            .filter(|&at| lexed.code[at..].starts_with('"'))
            .and_then(|at| lexed.code[at + 1..].split_once('"'))
            .map(|(value, _)| value)
            .ok_or_else(|| {
                format!(
                    "line {}: `{marker}` takes a plain string literal here",
                    line_of(start)
                )
            })?;
        let rust = matches!(marker.as_str(), "include!" | "#[path");
        if rust && source == Source::BuildScript {
            return Err(format!(
                "line {}: a build script uses `{marker}`; keep its code in build.rs",
                line_of(start)
            ));
        }
        let resolved = resolve(directory, argument).ok_or_else(|| {
            format!(
                "line {}: `{marker}` reaches `{argument}` outside the repository",
                line_of(start)
            )
        })?;
        let allowed = if rust {
            resolved.starts_with(&format!("{CRATE}/")) && is_tracked(&resolved)
        } else {
            is_tracked(&resolved) && counted_by_both(&resolved)
        };
        if !allowed {
            return Err(format!(
                "line {}: `{marker}` reaches `{argument}`, which is not {}",
                line_of(start),
                if rust {
                    "a tracked file of the packaging crate"
                } else {
                    "a tracked build input of both platforms, spelled as Git records it"
                }
            ));
        }
        if rust {
            reached.push(resolved);
        }
    }
    Ok(reached)
}

/// Where a `cfg_attr(predicate, attribute, …)` starting after `after` carries a `path = …`
/// attribute, nested `cfg_attr` included; the predicate itself may mention `path`.
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
        if let Some(rest) = trimmed.strip_prefix("path")
            && rest.trim_start().starts_with('=')
        {
            return Some(at);
        }
        if trimmed.starts_with("cfg_attr")
            && let Some(nested) = cfg_attr_path(tokens, at + "cfg_attr".len())
        {
            return Some(nested);
        }
    }
    None
}

/// The PowerShell scripts the packaging crate launches: string literals that name a `*.ps1`,
/// which must be a bare file name (its Windows adapter resolves it in `tools/release`).
pub(super) fn scripts_launched(lexed: &Lexed) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for literal in lexed
        .strings
        .iter()
        .filter(|literal| literal.contains(".ps1"))
    {
        if !super::is_script_name(literal) {
            return Err(format!(
                "names the script {literal:?} by a path; the packaging crate launches scripts of tools/release by file name"
            ));
        }
        names.push(literal.clone());
    }
    Ok(names)
}
