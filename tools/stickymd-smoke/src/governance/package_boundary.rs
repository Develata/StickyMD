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
//!   (no `..` component, no absolute path) and includes data only from paths both platforms
//!   count;
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
mod scripts;
mod workflow;

use crate::release::json::{self, Value};
use crate::release::platform::Platform;
use rust::{Lexed, Source, check_includes, check_names, lex, scripts_launched};
#[cfg(test)]
use scripts::SCRIPTS;
use scripts::{check_script, check_scripts};
use std::collections::BTreeSet;
use std::path::Path;
use workflow::check_workflow;

const CRATE: &str = "tools/stickymd-package";
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

/// Whether both platforms count `path`. Both count every file that may include data (the
/// packaging crate and every member's `build.rs`), so what such a file brings in must be counted
/// by both too.
pub(super) fn counted_by_both(path: &str) -> bool {
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

#[cfg(test)]
mod tests;
