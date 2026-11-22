//! The files rustc registers compiling a package's lib and bin targets, as rustc itself lists them
//! in the dep-info it writes: source modules, `#[path]` files and everything `include!`,
//! `include_str!` and `include_bytes!` read, however the code spells, renames or expands them.
//! Each check compiles afresh in a private directory, in every profile the pinned workflows use,
//! and proves one fresh unit per expected target whose dep-info names that target's own entry.
//! Paths are kept as written (relative to the workspace root), so a `..` or an absolute path
//! stays visible to the caller's rules; only Windows reads `\` as a separator.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use crate::release::json::{self, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The profiles the pinned release workflows compile the packaging crate in: `cargo run` (dev)
/// runs it, the Windows workspace build (release) builds it too.
pub(super) const PROFILES: &[&str] = &["dev", "release"];

/// A lib or bin target the compilation must produce a unit for.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Target {
    /// `lib` or `bin`.
    pub(super) kind: String,
    pub(super) name: String,
    /// The entry file, relative to the workspace root.
    pub(super) source: String,
}

/// `lib` for any library kind Cargo reports (`lib`, `rlib`, `dylib`, `cdylib`, `staticlib`),
/// `bin` for a binary, `None` for anything else (a procedural macro is refused elsewhere).
pub(super) fn lib_or_bin(kinds: &[Value]) -> Option<&'static str> {
    let is = |wanted: &str| kinds.iter().any(|kind| kind.string() == Ok(wanted));
    if ["lib", "rlib", "dylib", "cdylib", "staticlib"]
        .iter()
        .any(|kind| is(kind))
    {
        Some("lib")
    } else if is("bin") {
        Some("bin")
    } else {
        None
    }
}

/// Cargo from `PATH`: through rustup it is the toolchain `rust-toolchain.toml` pins. An inherited
/// `CARGO` could name any executable.
pub(super) fn cargo() -> std::process::Command {
    std::process::Command::new("cargo")
}

/// A path as rustc or Cargo wrote it, with `/` as the separator: on Windows `\` is one too; on
/// other hosts a `\` is a character of the name, which no tracked input of this repository uses,
/// so such a path is refused rather than read as another file.
pub(super) fn normalize(path: &str) -> Result<String, String> {
    if cfg!(windows) {
        Ok(path.replace('\\', "/"))
    } else if path.contains('\\') {
        Err(format!(
            "{path:?} holds a backslash, which names no tracked input here"
        ))
    } else {
        Ok(path.to_owned())
    }
}

/// Checks `package` in the workspace at `root` in each of [`PROFILES`], in a new private
/// directory, and returns the union of the files the expected targets' units registered.
pub(super) fn files(root: &Path, package: &str, targets: &[Target]) -> Result<Vec<String>, String> {
    let directory = stickymd_package::temporary::TemporaryDirectory::new("package-boundary")?;
    let mut files = BTreeSet::new();
    for profile in PROFILES {
        files.extend(files_in(root, package, targets, directory.path(), profile)?);
    }
    directory.close()?;
    Ok(files.into_iter().collect())
}

pub(super) fn files_in(
    root: &Path,
    package: &str,
    targets: &[Target],
    directory: &Path,
    profile: &str,
) -> Result<Vec<String>, String> {
    // The build directory holds the units and their dep-info; pinning it to the private directory
    // too keeps an inherited `CARGO_BUILD_BUILD_DIR` from pointing at a shared, older one.
    let output = cargo()
        .current_dir(root)
        .env("CARGO_BUILD_BUILD_DIR", directory.join("build"))
        .args([
            "check",
            "--locked",
            "--quiet",
            "-p",
            package,
            "--lib",
            "--bins",
            "--message-format=json",
            "--profile",
            profile,
        ])
        .arg("--target-dir")
        .arg(directory.join("target"))
        .output()
        .map_err(|error| format!("cannot start `cargo check`: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        // Whatever Cargo said, the JSON lines among it included.
        let messages: Vec<Value> = stdout.lines().filter_map(|l| json::parse(l).ok()).collect();
        return Err(format!(
            "`cargo check -p {package} --profile {profile}` failed: {}{}",
            String::from_utf8_lossy(&output.stderr).trim(),
            compiler_errors(&messages)
        ));
    }
    let messages: Vec<Value> = stdout
        .lines()
        .map(json::parse)
        .collect::<Result<_, _>>()
        .map_err(|e| format!("`cargo check` wrote a line that is not JSON: {e}"))?;
    let mut units: BTreeMap<Target, Vec<String>> = BTreeMap::new();
    for message in &messages {
        if message.field("reason").and_then(|r| r.string()).ok() != Some("compiler-artifact") {
            continue;
        }
        let target = message.field("target")?;
        let kinds = target.field("kind")?.array()?;
        let kind = lib_or_bin(kinds)
            .ok_or_else(|| format!("`cargo check` built an unexpected unit: {kinds:?}"))?;
        let name = target.field("name")?.string()?.to_owned();
        let expected = targets
            .iter()
            .find(|t| t.kind == kind && t.name == name)
            .ok_or_else(|| {
                format!("`cargo check` built the {kind} `{name}`, which metadata does not list")
            })?;
        if !matches!(message.field("fresh")?, Value::Bool(false)) {
            return Err(format!(
                "the {kind} `{name}` ({profile}) was not compiled afresh; its dep-info could be from another source state"
            ));
        }
        let rmeta = message
            .field("filenames")?
            .array()?
            .iter()
            .filter_map(|name| name.string().ok())
            .find(|name| name.ends_with(".rmeta"))
            .ok_or_else(|| format!("the {kind} `{name}` ({profile}) has no .rmeta"))?;
        let dep_info = dep_info_path(Path::new(rmeta))?;
        let text = std::fs::read_to_string(&dep_info)
            .map_err(|e| format!("{}: {e}", dep_info.display()))?;
        let paths = parse(&text).map_err(|e| format!("{}: {e}", dep_info.display()))?;
        if !paths.contains(&expected.source) {
            return Err(format!(
                "the dep-info of the {kind} `{name}` ({profile}) does not name its entry {}",
                expected.source
            ));
        }
        if units.insert(expected.clone(), paths).is_some() {
            return Err(format!(
                "`cargo check` reported the {kind} `{name}` ({profile}) twice"
            ));
        }
    }
    for target in targets {
        if !units.contains_key(target) {
            return Err(format!(
                "`cargo check --profile {profile}` compiled no unit for the {} `{}`",
                target.kind, target.name
            ));
        }
    }
    Ok(units.into_values().flatten().collect())
}

/// The rendered error diagnostics among Cargo's messages, at most a few, for a failed check.
fn compiler_errors(messages: &[Value]) -> String {
    let errors: Vec<&str> = messages
        .iter()
        .filter(|m| m.field("reason").and_then(|r| r.string()).ok() == Some("compiler-message"))
        .filter_map(|m| m.field("message").ok())
        .filter(|m| m.field("level").and_then(|l| l.string()).ok() == Some("error"))
        .filter_map(|m| m.field("rendered").and_then(|r| r.string()).ok())
        .take(3)
        .collect();
    if errors.is_empty() {
        String::new()
    } else {
        format!("\n{}", errors.join("\n"))
    }
}

/// `deps/libNAME-HASH.rmeta` → `deps/NAME-HASH.d`, where rustc writes the dep-info.
fn dep_info_path(rmeta: &Path) -> Result<std::path::PathBuf, String> {
    let stem = rmeta
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("lib"))
        .ok_or_else(|| format!("unexpected artifact name {}", rmeta.display()))?;
    Ok(rmeta.with_file_name(format!("{stem}.d")))
}

/// The files of a Makefile-style dep-info as rustc writes it: one rule per output
/// (`OUTPUT: FILE FILE …`) and one `FILE:` line per file. Every rule and the file lines must name
/// the same set, so no file can be dropped by a line the reader skips. An environment read
/// (`# env-dep:NAME`) is refused, since a file chosen through `env!` at compile time can differ
/// between the check and the release job; an escaped space or a `$` is refused rather than
/// unescaped (repository paths have neither); any other line is refused.
pub(super) fn parse(text: &str) -> Result<Vec<String>, String> {
    // First, so that no other diagnostic (which may quote a path built from the value) runs.
    if let Some(read) = text
        .lines()
        .find_map(|line| line.strip_prefix("# env-dep:"))
    {
        let variable = read.split('=').next().unwrap_or(read);
        return Err(format!(
            "the compilation reads the environment variable {variable}; plan 11 admits no environment read"
        ));
    }
    let mut rules: Vec<BTreeSet<String>> = Vec::new();
    let mut files = BTreeSet::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        if line.contains("\\ ") || line.contains('$') {
            return Err(format!("dep-info line {line:?} needs unescaping"));
        }
        if let Some((output, inputs)) = line.split_once(": ") {
            if !(output.ends_with(".d") || output.ends_with(".rmeta")) {
                return Err(format!("unexpected dep-info rule for {output:?}"));
            }
            rules.push(
                inputs
                    .split(' ')
                    .filter(|input| !input.is_empty())
                    .map(normalize)
                    .collect::<Result<_, _>>()?,
            );
        } else if let Some(file) = line.strip_suffix(':') {
            files.insert(normalize(file)?);
        } else {
            return Err(format!("unexpected dep-info line {line:?}"));
        }
    }
    if rules.is_empty() || files.is_empty() {
        return Err("dep-info without a rule or a file".to_owned());
    }
    if let Some(rule) = rules.iter().find(|rule| **rule != files) {
        return Err(format!(
            "a dep-info rule lists {rule:?}, the file lines {files:?}"
        ));
    }
    Ok(files.into_iter().collect())
}
