//! The packaging crate's boundary (plan 11 #platform-artifact-reuse). The Platform Build Input
//! Fingerprint counts `tools/stickymd-package` and not this verification tool, which is sound
//! only while no release byte can depend on verification code. The crate dependency makes the
//! compiler refuse any reference from the packaging crate to this one; this check covers the edges
//! the compiler cannot see, kept few on purpose so that each can be checked completely:
//! - Cargo's own view of the workspace (`cargo metadata`): the packaging crate declares no
//!   dependency and no build script, and every target of it lies in its directory as a tracked
//!   file; a member with a build script has its whole directory counted for Windows (whose
//!   workspace build compiles every member) and the script is its `build.rs`; no member is a
//!   procedural macro (code that runs inside the compiler); no two targets share a binary name, so
//!   no excluded binary can take the product's place in the workspace build's output;
//! - the packaging crate's Rust is its own `.rs` files: no `include!`, no `#[path]`, no `cfg_attr`
//!   path; data includes name a path in the Git index exactly as spelled that both platforms
//!   count (`rust` module);
//! - every member's build script keeps its code in itself, names no path that leaves its member
//!   and includes data only from paths that every platform counting the script counts;
//! - that code (comments aside) never names the verification tool's package or crate, in a
//!   literal or out of one;
//! - the scripts the packaging crate launches, the Windows scripts that build bytes and every
//!   script they reference never call the verification tool; a reference counts only when bound
//!   to `tools/release` (`Join-Path $PSScriptRoot '<name>'`, `$PSScriptRoot/<name>` or a
//!   repository path), is spelled as Git records it and is counted for the calling platform, so
//!   macOS builds reference no PowerShell script; anything else, and a dot-source not bound that
//!   way, is refused;
//! - each release workflow has the dependency-policy job and the three release jobs of plan 11
//!   (`self-check`, then `build-package`, then `verify-artifact`, which has a step whose own
//!   `uses` is the download action), and the `build-package` job never calls the tool.
//!
//! The checks are lexical. Code built to evade them (a name assembled at run time, a script run
//! through a variable) is left to review; what they catch is the ordinary way such an edge is
//! written.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

mod rust;

use crate::release::json::{self, Value};
use crate::release::platform::Platform;
use rust::{Lexed, Source, check_includes, check_names, lex, scripts_launched};
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
const RELEASE_WORKFLOWS: &[(&str, Platform)] = &[
    (".github/workflows/release.yml", Platform::WindowsX64),
    (".github/workflows/release-macos.yml", Platform::MacOsArm64),
];
const BUILD_APP: &str = "apps/stickymd-mac/packaging/build-app.sh";
/// How a script or a workflow step reaches the verification tool: by name, through its
/// PowerShell wrapper, or through its per-phase entry points.
const SCRIPT_TOOL_NAMES: &[&str] = &[
    "stickymd-smoke",
    "invoke-smoke.ps1",
    "Invoke-StickyMdReleaseTool",
    "tools/smoke/",
];

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let tracked = git_index(root)?;
    let is_tracked = |path: &str| tracked.contains(path);
    let workspace = check_metadata(&cargo_metadata(root)?)?;
    for target in &workspace.package_targets {
        if !is_tracked(target) {
            return Err(format!(
                "{target}: a packaging target that Git does not track"
            ));
        }
    }
    let mut sources = Vec::new();
    collect_rust(root, &root.join(CRATE), &mut sources)?;
    if sources.is_empty() {
        return Err(format!(
            "{CRATE} has no Rust sources; a check of nothing proves nothing"
        ));
    }
    sources.sort();
    let mut scripts: Vec<String> = BYTE_SCRIPTS.iter().map(|name| (*name).to_owned()).collect();
    for file in &sources {
        let lexed = lex_file(root, file)?;
        check_names(&lexed.code).map_err(|e| format!("{file}: {e}"))?;
        check_includes(file, &lexed, Source::Packaging, &is_tracked)
            .map_err(|e| format!("{file}: {e}"))?;
        scripts.extend(scripts_launched(&lexed).map_err(|e| format!("{file}: {e}"))?);
    }
    for script in &workspace.build_scripts {
        let lexed = lex_file(root, script)?;
        check_names(&lexed.code).map_err(|e| format!("{script}: {e}"))?;
        check_includes(script, &lexed, Source::BuildScript, &is_tracked)
            .map_err(|e| format!("{script}: {e}"))?;
    }
    check_scripts(root, &is_tracked, Platform::WindowsX64, scripts)?;
    let macos = check_script(&read(root, BUILD_APP)?, BUILD_APP)
        .map_err(|e| format!("{BUILD_APP}: {e}"))?;
    check_scripts(root, &is_tracked, Platform::MacOsArm64, macos)
        .map_err(|e| format!("{BUILD_APP}: {e}"))?;
    for (workflow, platform) in RELEASE_WORKFLOWS {
        let referenced =
            check_workflow(&read(root, workflow)?).map_err(|e| format!("{workflow}: {e}"))?;
        check_scripts(root, &is_tracked, *platform, referenced)
            .map_err(|e| format!("{workflow}: {e}"))?;
    }
    Ok(())
}

fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("{relative}: {e}"))
}

fn lex_file(root: &Path, file: &str) -> Result<Lexed, String> {
    lex(&read(root, file)?).map_err(|e| format!("{file}: {e}"))
}

/// The paths of the Git index, exactly as Git records them: a file only on disk is not a build
/// input of a clean checkout.
fn git_index(root: &Path) -> Result<BTreeSet<String>, String> {
    let output = crate::release::platform_inputs::git(root)
        .args(["ls-files", "-z", "--cached"])
        .output()
        .map_err(|error| format!("cannot run git ls-files: {error}"))?;
    if !output.status.success() {
        return Err("git ls-files --cached failed".to_owned());
    }
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec()).map_err(|_| "a tracked path is not UTF-8".to_owned())
        })
        .collect()
}

/// The repository-relative paths of the Rust files below `directory`, without following links:
/// a link inside the packaging crate is refused.
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

/// What the workspace's metadata leaves to check by content, as repository paths.
#[derive(Debug, PartialEq)]
struct Workspace {
    /// The source file of every target of the packaging crate.
    package_targets: Vec<String>,
    /// The members' build scripts.
    build_scripts: Vec<String>,
}

/// Cargo's decoded view, so no TOML spelling (a quoted table or key) can hide a dependency, a
/// target or a build script.
fn check_metadata(metadata: &Value) -> Result<Workspace, String> {
    let workspace_root = format!(
        "{}/",
        metadata
            .field("workspace_root")?
            .string()?
            .replace('\\', "/")
            .trim_end_matches('/')
    );
    let relative = |path: &str| -> Result<String, String> {
        path.replace('\\', "/")
            .strip_prefix(&workspace_root)
            .map(str::to_owned)
            .ok_or_else(|| format!("{path} is outside the workspace"))
    };
    let mut found = false;
    let mut binaries = BTreeSet::new();
    let mut checked = Workspace {
        package_targets: Vec::new(),
        build_scripts: Vec::new(),
    };
    for package in metadata.field("packages")?.array()? {
        let name = package.field("name")?.string()?;
        let manifest = relative(package.field("manifest_path")?.string()?)?;
        let directory = manifest
            .strip_suffix("/Cargo.toml")
            .ok_or_else(|| format!("{name}: manifest path {manifest} is not a Cargo.toml"))?;
        let packaging = name == "stickymd-package";
        if packaging {
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
            let source = relative(target.field("src_path")?.string()?)?;
            if is("proc-macro") {
                return Err(format!(
                    "{name} is a procedural macro, which runs inside the compiler; plan 11 admits no such member"
                ));
            }
            if is("bin") {
                let binary = target.field("name")?.string()?;
                if !binaries.insert(binary.to_ascii_lowercase()) {
                    return Err(format!(
                        "two workspace targets build the binary {binary}; one could take the other's place in the build output"
                    ));
                }
            }
            if is("custom-build") {
                if packaging {
                    return Err("stickymd-package has a build script".to_owned());
                }
                if source != format!("{directory}/build.rs") {
                    return Err(format!(
                        "{name}'s build script is {source}; only the build.rs beside a member's manifest is counted"
                    ));
                }
                // The Windows workspace build runs it, so all it can read must count for Windows.
                if !crate::release::platform_inputs::is_input(
                    Platform::WindowsX64,
                    &format!("{directory}/src/probe.rs"),
                ) {
                    return Err(format!(
                        "{name} has a build script, but its directory is not counted for Windows, whose workspace build runs it"
                    ));
                }
                checked.build_scripts.push(source);
            } else if packaging {
                if !source.starts_with(&format!("{CRATE}/")) {
                    return Err(format!(
                        "a stickymd-package target is {source}, outside the packaging crate"
                    ));
                }
                checked.package_targets.push(source);
            }
        }
    }
    if found {
        Ok(checked)
    } else {
        Err("cargo metadata lists no stickymd-package".to_owned())
    }
}

/// Whether every platform that counts `file` also counts `path`: what `file` brings into a build
/// is then an input of each build that compiles `file`. For a file of the packaging crate, which
/// both platforms count, this is "counted by both".
pub(super) fn counted_like(file: &str, path: &str) -> bool {
    use crate::release::platform_inputs::is_input;
    [Platform::WindowsX64, Platform::MacOsArm64]
        .into_iter()
        .all(|platform| !is_input(platform, file) || is_input(platform, path))
}

/// `relative` joined to the repository directory `directory` and normalized, or `None` when it
/// is absolute, uses a backslash or a drive, or climbs out of the repository.
pub(super) fn resolve(directory: &str, relative: &str) -> Option<String> {
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

pub(super) fn is_script_name(name: &str) -> bool {
    name.len() > 4
        && name.ends_with(".ps1")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Checks each named script of `tools/release`, called on `platform`'s build, and every script
/// it references, transitively: each is tracked exactly as named and counted for `platform`.
fn check_scripts(
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
fn check_script(text: &str, origin: &str) -> Result<Vec<String>, String> {
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

/// The release workflow's jobs in order, the `build-package` job free of the verification tool;
/// returns the scripts that job runs, to be checked in turn. `needs` is read from the job's own
/// fields and the download from a step's own `uses`, never from comments, scalars or step bodies.
fn check_workflow(text: &str) -> Result<Vec<String>, String> {
    let jobs = jobs(text)?;
    let job = |name: &str| {
        jobs.iter()
            .find(|(job, _)| job == name)
            .map(|(_, body)| body.as_str())
            .ok_or_else(|| format!("has no `{name}` job"))
    };
    job("dependency-policy")?;
    for (name, needs) in [
        ("self-check", "dependency-policy"),
        ("build-package", "self-check"),
        ("verify-artifact", "build-package"),
    ] {
        let declared = fields_at(job(name)?, 4).any(|field| field == format!("needs: {needs}"));
        if !declared {
            return Err(format!("job `{name}` must have `needs: {needs}`"));
        }
    }
    let downloads = step_fields(job("verify-artifact")?)
        .iter()
        .any(|field| field.starts_with("uses: actions/download-artifact@"));
    if !downloads {
        return Err("job `verify-artifact` must download the uploaded artifact".to_owned());
    }
    let build = job("build-package")?;
    check_script(build, "build-package").map_err(|e| format!("job `build-package`: {e}"))
}

/// The direct fields of a job's steps: the field on each `- ` item line (indent 6) and the lines
/// at indent 8 below it, inside the job's `steps:` (indent 4). A block scalar's lines are indented
/// more than its key, so none of them is read as a field.
fn step_fields(body: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_steps = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent <= 4 {
            in_steps = indent == 4 && without_comment(trimmed).trim() == "steps:";
            continue;
        }
        if !in_steps {
            continue;
        }
        let field = without_comment(trimmed).trim();
        if indent == 6 {
            if let Some(first) = field.strip_prefix("- ") {
                fields.push(first.trim().to_owned());
            }
        } else if indent == 8 {
            fields.push(field.to_owned());
        }
    }
    fields
}

/// The lines of `body` indented by exactly `indent` spaces, comments removed and trimmed.
fn fields_at(body: &str, indent: usize) -> impl Iterator<Item = &str> {
    body.lines().filter_map(move |line| {
        let field = line.get(indent..)?;
        (line[..indent].bytes().all(|b| b == b' ') && !field.starts_with(' '))
            .then(|| without_comment(field).trim())
    })
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
