//! The packaging crate's boundary (plan 11 #platform-artifact-reuse). The Platform Build Input
//! Fingerprint counts `tools/stickymd-package` and not this verification tool, which is sound
//! only while no release byte can depend on verification code. The crate dependency makes the
//! compiler refuse any reference from the packaging crate to this one; this check covers the edges
//! the compiler cannot see:
//! - Cargo's own view of the workspace (`cargo metadata`): the packaging crate declares no
//!   dependency and no build script; every member's build script is the `build.rs` beside its
//!   manifest, which the fingerprint counts; no member is a procedural macro (code that would run
//!   inside the compiler); no two targets share a binary name, so no excluded binary can take the
//!   product's place in the workspace build's output;
//! - the packaging crate's Rust (every `.rs` file, and every file its `include!` and `#[path]`
//!   reach, whatever its extension): those reach only tracked files of the crate, data includes
//!   name a path in the Git index exactly as spelled that both platforms count, `cfg_attr`
//!   carries no path, and a reached fragment declares no out-of-line module (`rust` module);
//! - every member's build script is self-contained (no out-of-line module, no Rust include) and
//!   includes data only from such counted paths;
//! - that code (comments aside) never names the verification tool, its package or its directory,
//!   so it can neither spawn it nor read its files;
//! - the scripts the packaging crate launches, the Windows scripts that build bytes, every script
//!   they reference and `build-app.sh` never call the verification tool; a reference counts only
//!   when it is bound to `tools/release` (`Join-Path $PSScriptRoot '<name>'`, `$PSScriptRoot/<name>`
//!   or a repository path), anything else, and a dot-source with none, is refused;
//! - each release workflow has the three jobs of plan 11, read from the jobs' own fields
//!   (`self-check`, then `build-package`, then `verify-artifact`, which has a real step using the
//!   download action), and the `build-package` job, with every script it runs, never calls the tool.
//!
//! The checks are lexical. Code built to evade them (a name assembled at run time, say) is left to
//! review; what they catch is the ordinary way such an edge is written.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

mod rust;

use crate::release::json::{self, Value};
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
const RELEASE_WORKFLOWS: &[&str] = &[
    ".github/workflows/release.yml",
    ".github/workflows/release-macos.yml",
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
    let build_scripts = check_metadata(&cargo_metadata(root)?)?;
    let tracked = git_index(root)?;
    let is_tracked = |path: &str| tracked.contains(path);
    let mut sources = Vec::new();
    collect_rust(root, &root.join(CRATE), &mut sources)?;
    if sources.is_empty() {
        return Err(format!(
            "{CRATE} has no Rust sources; a check of nothing proves nothing"
        ));
    }
    sources.sort();
    let mut scripts: Vec<String> = BYTE_SCRIPTS.iter().map(|name| (*name).to_owned()).collect();
    let mut queue: Vec<(String, Source)> = sources
        .into_iter()
        .map(|file| (file, Source::Packaging))
        .collect();
    let mut seen = BTreeSet::new();
    while let Some((file, source)) = queue.pop() {
        if !seen.insert(file.clone()) {
            continue;
        }
        let lexed = lex_file(root, &file)?;
        check_names(&lexed.code).map_err(|e| format!("{file}: {e}"))?;
        for reached in check_includes(&file, &lexed, source, &is_tracked)
            .map_err(|e| format!("{file}: {e}"))?
        {
            let source = if reached.ends_with(".rs") {
                Source::Packaging
            } else {
                Source::Fragment
            };
            queue.push((reached, source));
        }
        scripts.extend(scripts_launched(&lexed).map_err(|e| format!("{file}: {e}"))?);
    }
    for script in build_scripts {
        let lexed = lex_file(root, &script)?;
        check_names(&lexed.code).map_err(|e| format!("{script}: {e}"))?;
        check_includes(&script, &lexed, Source::BuildScript, &is_tracked)
            .map_err(|e| format!("{script}: {e}"))?;
    }
    let build_app = "apps/stickymd-mac/packaging/build-app.sh";
    scripts.extend(
        check_script(&read(root, build_app)?, build_app)
            .map_err(|e| format!("{build_app}: {e}"))?,
    );
    check_scripts(root, scripts)?;
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

/// Cargo's decoded view, so no TOML spelling (a quoted table or key) can hide a dependency or a
/// build script. Returns the repository paths of the members' build scripts.
fn check_metadata(metadata: &Value) -> Result<Vec<String>, String> {
    let workspace = format!(
        "{}/",
        metadata
            .field("workspace_root")?
            .string()?
            .replace('\\', "/")
            .trim_end_matches('/')
    );
    let mut found = false;
    let mut binaries = BTreeSet::new();
    let mut scripts = Vec::new();
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
            if is("proc-macro") {
                return Err(format!(
                    "{name} is a procedural macro, which runs inside the compiler; the fingerprint counts no such member"
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
            let relative = source.strip_prefix(&workspace).ok_or_else(|| {
                format!("{name}'s build script {source} is outside the workspace")
            })?;
            scripts.push(relative.to_owned());
        }
    }
    if found {
        Ok(scripts)
    } else {
        Err("cargo metadata lists no stickymd-package".to_owned())
    }
}

pub(super) fn counted_by_both(path: &str) -> bool {
    use crate::release::platform::Platform;
    use crate::release::platform_inputs::is_input;
    is_input(Platform::WindowsX64, path) && is_input(Platform::MacOsArm64, path)
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
    let before = line[..start].strip_suffix(['\'', '"'])?;
    let quote = &line[start - 1..start];
    let bound =
        before.trim_end().ends_with("Join-Path $PSScriptRoot") && line[end..].starts_with(quote);
    (bound && is_script_name(token)).then(|| token.to_owned())
}

/// The `*.ps1` occurrences of a line as (start, end) of their whole token.
fn ps1_tokens(line: &str) -> Vec<(usize, usize)> {
    let delimiter = |c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '(' | ')' | ',' | ';');
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = line[from..].find(".ps1") {
        let end = from + offset + ".ps1".len();
        let start = line[..end].rfind(delimiter).map_or(0, |at| at + 1);
        if line[end..].chars().next().is_none_or(delimiter) {
            found.push((start, end));
        }
        from = end;
    }
    found
}

/// A script, `build-app.sh` or a workflow job that builds release bytes: no call to the
/// verification tool, and every script reference bound to `tools/release` (returned by file
/// name); an unbound reference, and a dot-source with none, is refused. Comment lines are skipped.
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
        for (start, end) in ps1_tokens(code) {
            let name = bound_reference(code, start, end, in_release).ok_or_else(|| {
                format!(
                    "line {}: references {}, which this check cannot bind to {SCRIPTS}",
                    index + 1,
                    &code[start..end]
                )
            })?;
            references.push(name);
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
/// own fields and the download from a step's own `uses`, never from comments or step bodies.
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
        let declared = fields_at(job(name)?, 4).any(|field| field == format!("needs: {needs}"));
        if !declared {
            return Err(format!("job `{name}` must have `needs: {needs}`"));
        }
    }
    // A step's own `uses`: on its `- ` line (indent 6) or as a field of it (indent 8). Lines of a
    // block scalar are indented more than the step's fields.
    let body = job("verify-artifact")?;
    let downloads = fields_at(body, 6)
        .filter_map(|field| field.strip_prefix("- "))
        .chain(fields_at(body, 8))
        .any(|field| field.starts_with("uses: actions/download-artifact@"));
    if !downloads {
        return Err("job `verify-artifact` must download the uploaded artifact".to_owned());
    }
    let build = job("build-package")?;
    check_script(build, "build-package").map_err(|e| format!("job `build-package`: {e}"))
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
