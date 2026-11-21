//! The packaging crate's boundary (plan 11 #platform-artifact-reuse). The Platform Build Input
//! Fingerprint counts `tools/stickymd-package` and not this verification tool, which is sound
//! only while no release byte can depend on verification code. The crate dependency makes the
//! compiler refuse any reference from the packaging crate to this one; this check covers the edges
//! the compiler cannot see:
//! - Cargo's own view of the workspace (`cargo metadata`): the packaging crate declares no
//!   dependency and no build script, every member's build script is the `build.rs` beside its
//!   manifest (which the fingerprint counts), and no two targets share a binary name, so no
//!   excluded binary can take the place of the product in the workspace build's output;
//! - the packaging crate's Rust files: `include!` and `#[path]` stay inside the crate, and
//!   `include_str!`/`include_bytes!` name, from the file's directory, a path that is in the Git
//!   index exactly as spelled and is counted for both platforms; `cfg_attr` cannot carry a path;
//! - every member's `build.rs` is self-contained (no `mod`, `include!` or `#[path]`) and includes
//!   data only from such counted paths;
//! - that code (comments aside, read by a lexer that tells strings, raw strings and characters
//!   from comments) never names the verification tool, its package or its directory, so it can
//!   neither spawn it nor read its files;
//! - the scripts the packaging crate launches, the Windows scripts that build bytes, every script
//!   they reference (resolved through `$PSScriptRoot` or a repository path into `tools/release`;
//!   anything else is refused) and `build-app.sh` never call the verification tool, and a
//!   dot-source this check cannot resolve is refused;
//! - each release workflow has the three jobs of plan 11 (`self-check`, then `build-package`,
//!   then `verify-artifact`, which downloads the uploaded artifact), read from the jobs' own
//!   fields, and the `build-package` job, with every script it runs, never calls the tool.
//!
//! The checks are lexical. Code built to evade them (a name assembled at run time, say) is left to
//! review; what they catch is the ordinary way such an edge is written.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use crate::release::json::{self, Value};
use std::collections::BTreeSet;
use std::path::Path;

const CRATE: &str = "tools/stickymd-package";
const SCRIPTS: &str = "tools/release";
/// The Windows scripts that build release bytes, called by `release` or by other scripts.
const BYTE_SCRIPTS: &[&str] = &[
    "package.ps1",
    "generate-sbom.ps1",
    "generate-third-party-notices.ps1",
    "invoke-package.ps1",
];
const RELEASE_WORKFLOWS: &[&str] = &[
    ".github/workflows/release.yml",
    ".github/workflows/release-macos.yml",
];
/// How Rust code names the verification tool: its package, its crate and its directory.
const TOOL_NAMES: &[&str] = &[
    "\"stickymd-smoke\"",
    "-p stickymd-smoke",
    "stickymd_smoke",
    "stickymd-smoke/",
];
/// How a script or a workflow step reaches the verification tool: by name, through its
/// PowerShell wrapper, or through its per-phase entry points.
const SCRIPT_TOOL_NAMES: &[&str] = &[
    "stickymd-smoke",
    "invoke-smoke.ps1",
    "Invoke-StickyMdReleaseTool",
    "tools/smoke/",
];

pub(super) fn verify(root: &Path) -> Result<(), String> {
    check_metadata(&cargo_metadata(root)?)?;
    let tracked: BTreeSet<String> = stickymd_package::app_inputs::tracked_files(root)?
        .into_iter()
        .collect();
    let is_tracked = |path: &str| tracked.contains(path);
    let mut sources = Vec::new();
    collect_rust(root, &root.join(CRATE), &mut sources)?;
    sources.sort();
    if sources.is_empty() {
        return Err(format!(
            "{CRATE} has no Rust sources; a check of nothing proves nothing"
        ));
    }
    let mut scripts: Vec<String> = BYTE_SCRIPTS.iter().map(|name| (*name).to_owned()).collect();
    for file in &sources {
        let lexed = Lexed::read(root, file)?;
        check_names(&lexed.code).map_err(|e| format!("{file}: {e}"))?;
        check_includes(file, &lexed, Source::Packaging, &is_tracked)
            .map_err(|e| format!("{file}: {e}"))?;
        scripts.extend(scripts_launched(&lexed.code));
    }
    for module in crate::headless::Module::ALL {
        if module.package().is_none() {
            continue;
        }
        let script = format!("{}build.rs", module.root());
        if root.join(&script).is_file() {
            let lexed = Lexed::read(root, &script)?;
            check_names(&lexed.code).map_err(|e| format!("{script}: {e}"))?;
            check_includes(&script, &lexed, Source::BuildScript, &is_tracked)
                .map_err(|e| format!("{script}: {e}"))?;
        }
    }
    check_scripts(root, scripts)?;
    let build_app = "apps/stickymd-mac/packaging/build-app.sh";
    check_script(&read(root, build_app)?, build_app).map_err(|e| format!("{build_app}: {e}"))?;
    for workflow in RELEASE_WORKFLOWS {
        let referenced =
            check_workflow(&read(root, workflow)?).map_err(|e| format!("{workflow}: {e}"))?;
        check_scripts(root, referenced)?;
    }
    Ok(())
}

fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("{relative}: {e}"))
}

/// The repository-relative paths of the Rust files below `directory`, without following links:
/// a linked directory inside the packaging crate is refused.
fn collect_rust(root: &Path, directory: &Path, into: &mut Vec<String>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            return Err(format!(
                "{} is a link; the packaging crate holds only plain files",
                path.display()
            ));
        }
        if kind.is_dir() {
            collect_rust(root, &path, into)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn cargo_metadata(root: &Path) -> Result<Value, String> {
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `cargo metadata`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata --no-deps --locked` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("`cargo metadata` output is not UTF-8: {error}"))?;
    json::parse(&text)
}

/// Cargo's decoded view, so no TOML spelling (a quoted table or key) can hide a dependency or a
/// build script: the packaging crate has neither, every member's build script is the `build.rs`
/// beside its manifest, and binary names are unique in the workspace.
fn check_metadata(metadata: &Value) -> Result<(), String> {
    let mut found = false;
    let mut binaries = BTreeSet::new();
    for package in metadata.field("packages")?.array()? {
        let name = package.field("name")?.string()?;
        let manifest = package.field("manifest_path")?.string()?.replace('\\', "/");
        let directory = manifest
            .strip_suffix("/Cargo.toml")
            .ok_or_else(|| format!("{name}: manifest path {manifest} is not a Cargo.toml"))?;
        if name == "stickymd-package" {
            found = true;
            if !package.field("dependencies")?.array()?.is_empty() {
                return Err(
                    "stickymd-package declares a dependency; the packaging crate is std-only"
                        .to_owned(),
                );
            }
        }
        for target in package.field("targets")?.array()? {
            let kinds = target.field("kind")?.array()?;
            let is = |wanted: &str| kinds.iter().any(|kind| kind.string() == Ok(wanted));
            if is("bin") {
                let binary = target.field("name")?.string()?;
                if !binaries.insert(binary.to_ascii_lowercase()) {
                    return Err(format!(
                        "two workspace targets build the binary {binary}; one could take the other's place in the build output"
                    ));
                }
            }
            if !is("custom-build") {
                continue;
            }
            if name == "stickymd-package" {
                return Err("stickymd-package has a build script".to_owned());
            }
            let source = target.field("src_path")?.string()?.replace('\\', "/");
            if source != format!("{directory}/build.rs") {
                return Err(format!(
                    "{name}'s build script is {source}; only the build.rs beside a member's manifest is counted"
                ));
            }
        }
    }
    if found {
        Ok(())
    } else {
        Err("cargo metadata lists no stickymd-package".to_owned())
    }
}

/// A Rust file with its comments removed (`code`: literals kept) and, byte for byte aligned with
/// it, the same text with the contents of every string and character literal masked (`tokens`).
struct Lexed {
    code: String,
    tokens: String,
}

impl Lexed {
    fn read(root: &Path, file: &str) -> Result<Self, String> {
        lex(&read(root, file)?).map_err(|e| format!("{file}: {e}"))
    }
}

/// Lexes Rust source: comments (line, nested block) become spaces with newlines kept, string
/// literals (plain, byte, C, raw in any of those forms) and character literals are kept in `code`
/// and masked in `tokens`, a lifetime stays code. An unterminated literal or comment is an error.
fn lex(text: &str) -> Result<Lexed, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut code = String::with_capacity(text.len());
    let mut tokens = String::with_capacity(text.len());
    let push_code = |code: &mut String, tokens: &mut String, c: char| {
        code.push(c);
        tokens.push(c);
    };
    let mask = |tokens: &mut String, c: char| {
        if c == '\n' {
            tokens.push('\n');
        } else {
            tokens.extend(std::iter::repeat_n('_', c.len_utf8()));
        }
    };
    let blank = |code: &mut String, tokens: &mut String, c: char| {
        let filler = if c == '\n' { '\n' } else { ' ' };
        for _ in 0..c.len_utf8() {
            code.push(filler);
            tokens.push(filler);
            if filler == '\n' {
                break;
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
                blank(&mut code, &mut tokens, chars[i]);
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
                    blank(&mut code, &mut tokens, '/');
                    blank(&mut code, &mut tokens, '*');
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    blank(&mut code, &mut tokens, '*');
                    blank(&mut code, &mut tokens, '/');
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut code, &mut tokens, chars[i]);
                    i += 1;
                }
            }
        } else if let Some(end) =
            raw_string_end(&chars, i).filter(|_| !previous.is_some_and(identifier))
        {
            let open = chars[i..end].iter().position(|ch| *ch == '"').unwrap() + i;
            for ch in &chars[i..=open] {
                push_code(&mut code, &mut tokens, *ch);
            }
            let close_len = chars[open + 1..end]
                .iter()
                .rev()
                .take_while(|ch| **ch == '#')
                .count()
                + 1;
            for ch in &chars[open + 1..end - close_len] {
                code.push(*ch);
                mask(&mut tokens, *ch);
            }
            for ch in &chars[end - close_len..end] {
                push_code(&mut code, &mut tokens, *ch);
            }
            i = end;
        } else if c == '"' {
            push_code(&mut code, &mut tokens, c);
            i += 1;
            loop {
                let Some(&s) = chars.get(i) else {
                    return Err("unterminated string literal".to_owned());
                };
                i += 1;
                if s == '"' {
                    push_code(&mut code, &mut tokens, s);
                    break;
                }
                code.push(s);
                mask(&mut tokens, s);
                if s == '\\'
                    && let Some(&escaped) = chars.get(i)
                {
                    code.push(escaped);
                    mask(&mut tokens, escaped);
                    i += 1;
                }
            }
        } else if c == '\''
            && (!previous.is_some_and(identifier)
                || (previous == Some('b')
                    && !i.checked_sub(2).map(|p| chars[p]).is_some_and(identifier)))
        {
            // A character literal ('x', '\n', '\'', '"', b'x'), or a lifetime, which stays code.
            let literal_end = if next == Some('\\') {
                (i + 3..chars.len().min(i + 12)).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match literal_end {
                Some(end) => {
                    push_code(&mut code, &mut tokens, c);
                    for ch in &chars[i + 1..end] {
                        code.push(*ch);
                        mask(&mut tokens, *ch);
                    }
                    push_code(&mut code, &mut tokens, '\'');
                    i = end + 1;
                }
                None => {
                    push_code(&mut code, &mut tokens, c);
                    i += 1;
                }
            }
        } else {
            push_code(&mut code, &mut tokens, c);
            i += 1;
        }
    }
    Ok(Lexed { code, tokens })
}

/// The end (exclusive) of a raw string literal starting at `i` (`r`, `br` or `cr`, any number of
/// `#`), or `None` when none starts there. An unterminated one ends at the text's end, which the
/// caller then treats like any other code; `None` keeps the lexer moving.
fn raw_string_end(chars: &[char], i: usize) -> Option<usize> {
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
    if chars.get(j) != Some(&'"') {
        return None;
    }
    let mut k = j + 1;
    while k < chars.len() {
        if chars[k] == '"'
            && chars[k + 1..]
                .iter()
                .take(hashes)
                .filter(|c| **c == '#')
                .count()
                == hashes
            && chars.len() >= k + 1 + hashes
        {
            return Some(k + 1 + hashes);
        }
        k += 1;
    }
    None
}

/// Refuses code that names the verification tool.
fn check_names(code: &str) -> Result<(), String> {
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

#[derive(Clone, Copy, PartialEq)]
enum Source {
    /// A file of the packaging crate: Rust includes stay in the crate.
    Packaging,
    /// A member's `build.rs`: self-contained, data includes only.
    BuildScript,
}

/// Every include and `#[path]` of `file` (found in the masked tokens, read from the code), each
/// checked: a Rust include or `#[path]` stays inside the packaging crate (and a build script has
/// none, nor any `mod` declaration); a data include names a path in the Git index exactly as
/// spelled that both platforms count; `cfg_attr` cannot carry a path.
fn check_includes(
    file: &str,
    lexed: &Lexed,
    source: Source,
    is_tracked: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    let line_of = |at: usize| lexed.tokens[..at].matches('\n').count() + 1;
    if let Some(at) = find_cfg_attr_path(&lexed.tokens) {
        return Err(format!(
            "line {}: `cfg_attr` carries a `path`; name modules plainly",
            line_of(at)
        ));
    }
    if source == Source::BuildScript
        && let Some(at) = find_mod_declaration(&lexed.tokens)
    {
        return Err(format!(
            "line {}: a build script declares a module; keep it in build.rs",
            line_of(at)
        ));
    }
    for marker in ["include_str!", "include_bytes!", "include!", "#[path"] {
        let mut from = 0;
        while let Some(found) = lexed.tokens[from..].find(marker) {
            let at = from + found;
            from = at + marker.len();
            let after = &lexed.code[from..];
            let argument = after
                .trim_start_matches(['(', ' ', '=', '\n', '\r', '\t'])
                .strip_prefix('"')
                .and_then(|value| value.split_once('"'))
                .map(|(value, _)| value)
                .ok_or_else(|| {
                    format!(
                        "line {}: `{marker}` takes a plain string literal here",
                        line_of(at)
                    )
                })?;
            let rust = matches!(marker, "include!" | "#[path");
            if rust && source == Source::BuildScript {
                return Err(format!(
                    "line {}: a build script uses `{marker}`; keep it in build.rs",
                    line_of(at)
                ));
            }
            let resolved = resolve(directory, argument).ok_or_else(|| {
                format!(
                    "line {}: `{marker}` reaches `{argument}` outside the repository",
                    line_of(at)
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
                    line_of(at),
                    if rust {
                        "a tracked file of the packaging crate"
                    } else {
                        "a tracked build input of both platforms, spelled as Git records it"
                    }
                ));
            }
        }
    }
    Ok(())
}

/// The position of a `cfg_attr(...)` whose arguments include `path =`.
fn find_cfg_attr_path(tokens: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(found) = tokens[from..].find("cfg_attr") {
        let at = from + found;
        from = at + "cfg_attr".len();
        let rest = &tokens[from..];
        let end = rest.find(']').unwrap_or(rest.len());
        let inside = &rest[..end];
        if inside
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '='))
            .any(|word| word == "path" || word.starts_with("path="))
            || inside.contains("path =")
        {
            return Some(at);
        }
    }
    None
}

/// The position of an out-of-line `mod name;` declaration.
fn find_mod_declaration(tokens: &str) -> Option<usize> {
    let bytes = tokens.as_bytes();
    let mut from = 0;
    while let Some(found) = tokens[from..].find("mod") {
        let at = from + found;
        from = at + 3;
        let before_ok =
            at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let rest = &tokens[from..];
        if !before_ok || !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let rest = rest.trim_start();
        let name_len = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        if name_len > 0 && rest[name_len..].trim_start().starts_with(';') {
            return Some(at);
        }
    }
    None
}

fn counted_by_both(path: &str) -> bool {
    use crate::release::platform::Platform;
    use crate::release::platform_inputs::is_input;
    is_input(Platform::WindowsX64, path) && is_input(Platform::MacOsArm64, path)
}

/// `relative` joined to the repository directory `directory` and normalized, or `None` when it
/// is absolute, uses a backslash or a drive, or climbs out of the repository.
fn resolve(directory: &str, relative: &str) -> Option<String> {
    if relative.starts_with('/') || relative.contains('\\') || relative.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The PowerShell scripts the packaging crate launches: bare `*.ps1` names in string literals,
/// which its Windows adapter resolves in `tools/release`.
fn scripts_launched(code: &str) -> Vec<String> {
    code.split('"')
        .skip(1)
        .step_by(2)
        .filter(|piece| is_script_name(piece))
        .map(str::to_owned)
        .collect()
}

fn is_script_name(name: &str) -> bool {
    name.len() > 4
        && name.ends_with(".ps1")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Checks each named script of `tools/release` and every script it references, transitively.
fn check_scripts(root: &Path, mut scripts: Vec<String>) -> Result<(), String> {
    let mut checked = BTreeSet::new();
    while let Some(name) = scripts.pop() {
        if !checked.insert(name.clone()) {
            continue;
        }
        let relative = format!("{SCRIPTS}/{name}");
        if !root.join(&relative).is_file() {
            return Err(format!(
                "a script that builds release bytes references {relative}, which does not exist"
            ));
        }
        let text = read(root, &relative)?;
        scripts.extend(check_script(&text, &relative).map_err(|e| format!("{relative}: {e}"))?);
    }
    Ok(())
}

/// The `*.ps1` tokens of a line, as written (with any directory).
fn ps1_tokens(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '(' | ')' | ',' | ';'))
        .filter(|token| token.ends_with(".ps1"))
}

/// A script, `build-app.sh` or a workflow job that builds release bytes: no call to the
/// verification tool, and every script reference resolved into `tools/release` (returned by file
/// name). A reference is resolved when it names `$PSScriptRoot` on its line (in a script of
/// `tools/release`) or is a repository path into `tools/release`; any other reference, and a
/// dot-source with no reference, is refused. Comment lines are skipped.
fn check_script(text: &str, origin: &str) -> Result<Vec<String>, String> {
    let in_release = origin.starts_with(&format!("{SCRIPTS}/"));
    let mut references = Vec::new();
    let mut block_comment = false;
    for (index, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if block_comment {
            block_comment = !code.contains("#>");
            continue;
        }
        if code.starts_with("<#") {
            block_comment = !code.contains("#>");
            continue;
        }
        if code.starts_with('#') {
            continue;
        }
        for name in SCRIPT_TOOL_NAMES {
            if code.contains(name) {
                return Err(format!(
                    "line {} calls the verification tool (`{name}`); building release bytes calls stickymd-package",
                    index + 1
                ));
            }
        }
        let mut found = false;
        for token in ps1_tokens(code) {
            let relative = token.strip_prefix("./").unwrap_or(token);
            let name = if let Some(name) = relative.strip_prefix(&format!("{SCRIPTS}/")) {
                name
            } else if let Some(name) = token
                .strip_prefix("$PSScriptRoot/")
                .or_else(|| token.strip_prefix("$PSScriptRoot\\"))
            {
                name
            } else if in_release && code.contains("$PSScriptRoot") && is_script_name(token) {
                token
            } else {
                return Err(format!(
                    "line {}: references {token}, which this check cannot resolve into {SCRIPTS}",
                    index + 1
                ));
            };
            if !is_script_name(name) {
                return Err(format!(
                    "line {}: references {token}, which this check cannot resolve into {SCRIPTS}",
                    index + 1
                ));
            }
            references.push(name.to_owned());
            found = true;
        }
        if code.starts_with(". ") && !found {
            return Err(format!(
                "line {}: a dot-source this check cannot follow; name the script literally",
                index + 1
            ));
        }
    }
    Ok(references)
}

/// The release workflow's three jobs in order, the `build-package` job free of the verification
/// tool; returns the scripts that job runs, to be checked in turn. `needs` is read from the job's
/// own fields and the download from a real step, never from comments or step bodies.
fn check_workflow(text: &str) -> Result<Vec<String>, String> {
    let jobs = jobs(text)?;
    let job = |name: &str| {
        jobs.iter()
            .find(|(job, _)| job == name)
            .map(|(_, body)| body.as_str())
            .ok_or_else(|| format!("has no `{name}` job"))
    };
    for (name, needs) in [
        ("self-check", "dependency-policy"),
        ("build-package", "self-check"),
        ("verify-artifact", "build-package"),
    ] {
        let declared = job(name)?
            .lines()
            .filter_map(|line| {
                let field = line.strip_prefix("    ")?;
                (!field.starts_with(' ')).then(|| without_comment(field).trim())
            })
            .any(|field| field == format!("needs: {needs}"));
        if !declared {
            return Err(format!("job `{name}` must have `needs: {needs}`"));
        }
    }
    let downloads = job("verify-artifact")?.lines().any(|line| {
        let step = without_comment(line).trim_start();
        let step = step.strip_prefix("- ").unwrap_or(step);
        step.starts_with("uses: actions/download-artifact@")
    });
    if !downloads {
        return Err("job `verify-artifact` must download the uploaded artifact".to_owned());
    }
    let build = job("build-package")?;
    check_script(build, "build-package").map_err(|e| format!("job `build-package`: {e}"))
}

/// A YAML line without a trailing `#` comment (one preceded by whitespace or at the start).
fn without_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && (at == 0 || bytes[at - 1].is_ascii_whitespace()) {
            return &line[..at];
        }
    }
    line
}

/// The jobs of a workflow: each `  <name>:` header directly under `jobs:` (a trailing comment
/// allowed) with the lines up to the next header. Blank and comment lines belong to the job they
/// sit in, whatever their indent; any other line indented less than a job ends the jobs. A job
/// name given twice is an error.
fn jobs(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.lines().skip_while(|line| *line != "jobs:");
    if lines.next().is_none() {
        return Err("has no `jobs:` section".to_owned());
    }
    let mut jobs: Vec<(String, String)> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if let Some((_, body)) = jobs.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if indent < 2 {
            break;
        }
        if indent == 2 {
            let name = without_comment(trimmed)
                .trim_end()
                .strip_suffix(':')
                .filter(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                })
                .ok_or_else(|| format!("unexpected line under `jobs:`: {trimmed}"))?;
            if jobs.iter().any(|(job, _)| job == name) {
                return Err(format!("job `{name}` is defined twice"));
            }
            jobs.push((name.to_owned(), String::new()));
            continue;
        }
        let (_, body) = jobs
            .last_mut()
            .ok_or_else(|| "a step before the first job".to_owned())?;
        body.push_str(line);
        body.push('\n');
    }
    Ok(jobs)
}

#[cfg(test)]
mod tests;
