//! The PowerShell side of the packaging boundary: which scripts of `tools/release` a build runs,
//! followed through their references, and that none reaches the verification tool.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::{Path, Platform, read};
use std::collections::BTreeSet;

pub(super) const SCRIPTS: &str = "tools/release";
/// How a script or a workflow step reaches the verification tool: by name, through its
/// PowerShell wrapper, or through its per-phase entry points.
pub(super) const SCRIPT_TOOL_NAMES: &[&str] = &[
    "stickymd-smoke",
    "invoke-smoke.ps1",
    "Invoke-StickyMdReleaseTool",
    "tools/smoke/",
];

pub(super) fn is_script_name(name: &str) -> bool {
    name.len() > 4
        && name.ends_with(".ps1")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Checks each named script of `tools/release`, called on `platform`'s build, and every script
/// it references, transitively: each is tracked exactly as named and counted for `platform`.
pub(super) fn check_scripts(
    root: &Path,
    is_tracked: &dyn Fn(&str) -> bool,
    platform: Platform,
    mut scripts: Vec<String>,
) -> Result<(), String> {
    let mut checked = BTreeSet::new();
    while let Some(name) = scripts.pop() {
        if !checked.insert(name.clone()) {
            continue;
        }
        let relative = format!("{SCRIPTS}/{name}");
        if !is_tracked(&relative) {
            return Err(format!(
                "a script that builds release bytes references {relative}, which Git does not track under that spelling"
            ));
        }
        if !crate::release::platform_inputs::is_input(platform, &relative) {
            return Err(format!(
                "the {} build references {relative}, which its fingerprint does not count",
                platform.label()
            ));
        }
        let text = read(root, &relative)?;
        scripts.extend(check_script(&text, &relative).map_err(|e| format!("{relative}: {e}"))?);
    }
    Ok(())
}

/// The script a `*.ps1` occurrence at `start..end` of `line` names in `tools/release`, when the
/// line binds it there: `Join-Path $PSScriptRoot '<name>'` (in a script of `tools/release`),
/// `$PSScriptRoot/<name>` or `$PSScriptRoot\<name>` (likewise), or a repository path
/// `tools/release/<name>` (optionally `./`, either separator).
fn bound_reference(line: &str, start: usize, end: usize, in_release: bool) -> Option<String> {
    let token = &line[start..end];
    let repository = token
        .strip_prefix("./")
        .or_else(|| token.strip_prefix(".\\"))
        .unwrap_or(token)
        .replace('\\', "/");
    if let Some(name) = repository.strip_prefix(&format!("{SCRIPTS}/")) {
        return is_script_name(name).then(|| name.to_owned());
    }
    if !in_release {
        return None;
    }
    if let Some(name) = token
        .strip_prefix("$PSScriptRoot/")
        .or_else(|| token.strip_prefix("$PSScriptRoot\\"))
    {
        return is_script_name(name).then(|| name.to_owned());
    }
    // `Join-Path $PSScriptRoot '<name>'`: the quoted name is the second argument.
    let quote = line[..start].chars().next_back()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let before = &line[..start - 1];
    let bound =
        before.trim_end().ends_with("Join-Path $PSScriptRoot") && line[end..].starts_with(quote);
    (bound && is_script_name(token)).then(|| token.to_owned())
}

/// The `*.ps1` occurrences of a line, in any letter case, as (start, end) of their whole token.
fn ps1_tokens(line: &str) -> Vec<(usize, usize)> {
    let delimiter = |c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '(' | ')' | ',' | ';');
    let lower = line.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(".ps1") {
        let end = from + offset + ".ps1".len();
        let start = line[..end]
            .char_indices()
            .rev()
            .find(|(_, c)| delimiter(*c))
            .map_or(0, |(at, c)| at + c.len_utf8());
        if line[end..].chars().next().is_none_or(delimiter) {
            found.push((start, end));
        }
        from = end;
    }
    found
}

/// The code of a PowerShell line: `<# … #>` comments removed (`in_block` carries one across
/// lines) and a `#` comment ended at the line's end; quotes are respected for the latter.
fn script_code(line: &str, in_block: &mut bool) -> String {
    let mut code = String::new();
    let mut rest = line;
    loop {
        if *in_block {
            match rest.find("#>") {
                Some(at) => {
                    rest = &rest[at + 2..];
                    *in_block = false;
                }
                None => return code,
            }
        }
        match rest.find("<#") {
            Some(at) => {
                code.push_str(&rest[..at]);
                rest = &rest[at + 2..];
                *in_block = true;
            }
            None => {
                code.push_str(rest);
                break;
            }
        }
    }
    let mut quote = None;
    for (at, c) in code.char_indices() {
        match (quote, c) {
            (None, '\'' | '"') => quote = Some(c),
            (Some(open), c) if c == open => quote = None,
            (None, '#') if at == 0 || code[..at].ends_with(char::is_whitespace) => {
                return code[..at].to_owned();
            }
            _ => {}
        }
    }
    code
}

/// A script, `build-app.sh` or a workflow job that builds release bytes: no call to the
/// verification tool, and every script reference bound to `tools/release` (returned by file
/// name). An unbound reference, and a dot-source whose own target is not bound, is refused.
pub(super) fn check_script(text: &str, origin: &str) -> Result<Vec<String>, String> {
    let in_release = origin.starts_with(&format!("{SCRIPTS}/"));
    let mut references = Vec::new();
    let mut in_block = false;
    for (index, line) in text.lines().enumerate() {
        let code = script_code(line, &mut in_block);
        let code = code.trim_start();
        if code.is_empty() {
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
        for (start, end) in ps1_tokens(code) {
            let name = bound_reference(code, start, end, in_release).ok_or_else(|| {
                format!(
                    "line {}: references {}, which this check cannot bind to {SCRIPTS}",
                    index + 1,
                    &code[start..end]
                )
            })?;
            references.push(name);
        }
        if let Some(target) = code.strip_prefix(". ") {
            let target = target.trim_start();
            let bound = [
                "(Join-Path $PSScriptRoot",
                "\"$PSScriptRoot",
                "$PSScriptRoot",
            ]
            .iter()
            .any(|form| target.starts_with(form))
                && !ps1_tokens(target).is_empty();
            if !bound {
                return Err(format!(
                    "line {}: a dot-source this check cannot follow; name the script through $PSScriptRoot",
                    index + 1
                ));
            }
        }
    }
    Ok(references)
}
