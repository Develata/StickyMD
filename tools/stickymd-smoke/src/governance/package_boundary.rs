//! The packaging crate's boundary (plan 11 #platform-artifact-reuse). The Platform Build Input
//! Fingerprint counts `tools/stickymd-package` and not this verification tool, which is sound
//! only while no release byte can depend on verification code. The crate dependency makes the
//! compiler refuse any reference from the packaging crate to this one; this check covers the edges
//! the compiler cannot see:
//! - Cargo's own view of the workspace (`cargo metadata`): the packaging crate declares no
//!   dependency and no build script, and every member's build script is its `build.rs`, which the
//!   fingerprint counts;
//! - every `include!`, `include_str!`, `include_bytes!` and `#[path]` reached from the packaging
//!   crate names a plain string literal that resolves, from its file's directory, to a path the
//!   fingerprint counts for both platforms; a Rust file reached that way is checked in turn;
//! - that code and every member's `build.rs` (comments aside, read with string and character
//!   literals told apart from comments) never names the verification tool, its package or its
//!   directory, so it can neither spawn it nor read its files;
//! - the scripts the packaging crate launches and the Windows scripts that build bytes, with every
//!   script they reference, and `build-app.sh`, never call the verification tool; a dot-source
//!   this check cannot read is refused rather than skipped;
//! - each release workflow has the three jobs of plan 11 (`self-check`, then `build-package`,
//!   then `verify-artifact` on the uploaded artifact), and the `build-package` job, with every
//!   script it runs, never calls the verification tool.
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
    let mut sources = Vec::new();
    collect_rust(root, &root.join(CRATE), &mut sources)?;
    if sources.is_empty() {
        return Err(format!(
            "{CRATE} has no Rust sources; a check of nothing proves nothing"
        ));
    }
    let launched = check_rust_tree(root, sources)?;
    let mut scripts: Vec<String> = BYTE_SCRIPTS.iter().map(|name| (*name).to_owned()).collect();
    scripts.extend(launched);
    check_scripts(root, scripts)?;
    check_build_scripts(root)?;
    let build_app = "apps/stickymd-mac/packaging/build-app.sh";
    check_script(&read(root, build_app)?).map_err(|e| format!("{build_app}: {e}"))?;
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

/// The repository-relative paths of the Rust files below `directory`.
fn collect_rust(root: &Path, directory: &Path, into: &mut Vec<String>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_rust(root, &path, into)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path.strip_prefix(root).map_err(|e| e.to_string())?;
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    into.sort();
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
/// build script: the packaging crate has neither, and every member's build script is the
/// `build.rs` beside its manifest.
fn check_metadata(metadata: &Value) -> Result<(), String> {
    let mut found = false;
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
            let custom_build = target
                .field("kind")?
                .array()?
                .iter()
                .any(|kind| kind.string() == Ok("custom-build"));
            if !custom_build {
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

/// Checks the Rust files `files` and every Rust file their `include!` and `#[path]` reach, and
/// returns the PowerShell scripts their code names.
fn check_rust_tree(root: &Path, mut files: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = BTreeSet::new();
    let mut launched = Vec::new();
    while let Some(file) = files.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let code = strip_comments(&read(root, &file)?).map_err(|e| format!("{file}: {e}"))?;
        check_names(&code).map_err(|e| format!("{file}: {e}"))?;
        for (marker, target) in includes(&file, &code).map_err(|e| format!("{file}: {e}"))? {
            if marker != "include_str!" && marker != "include_bytes!" {
                // Rust source: its own includes and names are checked too.
                files.push(target);
            }
        }
        launched.extend(ps1_references(&code));
    }
    Ok(launched)
}

/// `text` with every comment replaced by spaces (newlines kept, so line numbers hold), string
/// and character literals kept as written. An unterminated string or block comment is an error.
fn strip_comments(text: &str) -> Result<String, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
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
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r'
            && matches!(next, Some('"' | '#'))
            && !i
                .checked_sub(1)
                .and_then(|p| chars.get(p))
                .is_some_and(|p| p.is_alphanumeric() || *p == '_')
        {
            // A raw string: r"...", r#"..."#; a `b` before it stays plain code.
            let mut hashes = 0;
            let mut j = i + 1;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            let close: Vec<char> = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            let mut k = j + 1;
            loop {
                if k + close.len() > chars.len() {
                    return Err("unterminated raw string".to_owned());
                }
                if chars[k..k + close.len()] == close[..] {
                    break;
                }
                k += 1;
            }
            let end = k + close.len();
            out.extend(&chars[i..end]);
            i = end;
        } else if c == '"' {
            out.push(c);
            i += 1;
            loop {
                let Some(&s) = chars.get(i) else {
                    return Err("unterminated string literal".to_owned());
                };
                out.push(s);
                i += 1;
                if s == '\\' {
                    if let Some(&escaped) = chars.get(i) {
                        out.push(escaped);
                        i += 1;
                    }
                } else if s == '"' {
                    break;
                }
            }
        } else if c == '\'' {
            // A character literal ('x', '\n', '\'', '"'), or a lifetime, which stays code.
            let literal_end = if next == Some('\\') {
                (i + 3..chars.len().min(i + 12)).find(|&k| chars[k] == '\'')
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match literal_end {
                Some(end) => {
                    out.extend(&chars[i..=end]);
                    i = end + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok(out)
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

/// Every include and `#[path]` in `code` (comments removed) with its target, resolved from the
/// directory of `file`; each must be a plain literal naming a path both platforms count.
fn includes(file: &str, code: &str) -> Result<Vec<(&'static str, String)>, String> {
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    let mut found = Vec::new();
    for (number, line) in code.lines().enumerate() {
        for marker in ["include_str!", "include_bytes!", "include!", "#[path"] {
            let mut rest = line;
            while let Some(at) = rest.find(marker) {
                let after = &rest[at + marker.len()..];
                let argument = after
                    .trim_start_matches(['(', ' ', '='])
                    .strip_prefix('"')
                    .and_then(|value| value.split_once('"'))
                    .map(|(value, _)| value)
                    .ok_or_else(|| {
                        format!(
                            "line {}: `{marker}` takes a plain string literal here",
                            number + 1
                        )
                    })?;
                let resolved = resolve(directory, argument)
                    .filter(|path| counted_by_both(path))
                    .ok_or_else(|| {
                        format!(
                            "line {}: `{marker}` reaches `{argument}`, which is not a build input of both platforms",
                            number + 1
                        )
                    })?;
                found.push((marker, resolved));
                rest = after;
            }
        }
    }
    Ok(found)
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

/// No member's `build.rs` (all of them run in the Windows workspace build) names the
/// verification tool, so a build script cannot read its files.
fn check_build_scripts(root: &Path) -> Result<(), String> {
    for module in crate::headless::Module::ALL {
        if module.package().is_none() {
            continue;
        }
        let script = format!("{}build.rs", module.root());
        if root.join(&script).is_file() {
            let code =
                strip_comments(&read(root, &script)?).map_err(|e| format!("{script}: {e}"))?;
            check_names(&code).map_err(|e| format!("{script}: {e}"))?;
        }
    }
    Ok(())
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
                "a script that builds release bytes references {name}, which is not in {SCRIPTS}"
            ));
        }
        let text = read(root, &relative)?;
        check_script(&text).map_err(|e| format!("{relative}: {e}"))?;
        scripts.extend(ps1_references(&text));
    }
    Ok(())
}

/// The PowerShell script file names `text` mentions: every `*.ps1` token, quoted or not, by its
/// last path component.
fn ps1_references(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '(' | ')' | ',' | ';'))
        .filter_map(|token| {
            let name = token.rsplit(['/', '\\']).next()?;
            (name.len() > 4
                && name.ends_with(".ps1")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')))
            .then(|| name.to_owned())
        })
        .collect()
}

/// A script or workflow job that builds release bytes: no call to the verification tool, and
/// every dot-source names a script this check can follow.
fn check_script(text: &str) -> Result<(), String> {
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
        if code.starts_with(". ") && ps1_references(code).is_empty() {
            return Err(format!(
                "line {}: a dot-source this check cannot follow; name the script literally",
                index + 1
            ));
        }
    }
    Ok(())
}

/// The release workflow's three jobs in order, the `build-package` job free of the verification
/// tool; returns the scripts that job runs, to be checked in turn.
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
        if !job(name)?
            .lines()
            .any(|line| line.trim() == format!("needs: {needs}"))
        {
            return Err(format!("job `{name}` must have `needs: {needs}`"));
        }
    }
    if !job("verify-artifact")?.contains("uses: actions/download-artifact@") {
        return Err("job `verify-artifact` must download the uploaded artifact".to_owned());
    }
    let build = job("build-package")?;
    check_script(build).map_err(|e| format!("job `build-package`: {e}"))?;
    Ok(ps1_references(build))
}

/// The jobs of a workflow: each `  <name>:` header directly under `jobs:` with the lines up to the
/// next header. Blank and comment lines belong to the job they sit in, whatever their indent; any
/// other line indented less than a job ends the jobs. A job name given twice is an error.
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
            let name = trimmed
                .strip_suffix(':')
                .filter(|name| {
                    name.bytes()
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
