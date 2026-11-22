//! The packaging crate's boundary (plan 11 #platform-artifact-reuse, "边界的证明"). The Platform
//! Build Input Fingerprint counts `tools/stickymd-package` and not this verification tool, which
//! is sound only while no release byte depends on verification code. The crate dependency makes
//! the compiler refuse any reference from the packaging crate to this one; the edges the compiler
//! cannot see are checked here as a closed world. Each rule is an exact comparison with a fixed
//! list or a pinned copy, a raw search of a file's whole text, a fact Cargo or rustc reports, or
//! a path and mode of the Git index; nothing here parses Rust, PowerShell or YAML, and whatever is
//! not on a list is refused:
//! - Cargo metadata: the packaging crate has no dependency, build script or feature; no member is
//!   a procedural macro; no two targets share a binary name; every member's manifest is a regular
//!   file of the index counted by both platforms; the build scripts are exactly
//!   [`BUILD_SCRIPTS`], and no member depends on a member that has one;
//! - the files rustc registers compiling the packaging crate's lib and bin targets afresh in each
//!   profile (`compiled` module) are regular files of the index inside the crate, spelled as Git
//!   records them, that both platforms count;
//! - the listed build scripts name no parent directory;
//! - the Windows scripts that build bytes are exactly [`BYTE_SCRIPTS`], valid UTF-8, and every
//!   `.ps1` name in them or in the compiled files is one of them; `build-app.sh` names none;
//! - each release workflow equals its pinned copy, whole, and the copies are exactly the
//!   `.github/` files the classifier counts (`workflow` module);
//! - none of these files, no tracked file under `crates/` or `apps/` and no pinned `build-package`
//!   job names the verification tool, in any letter case, comments included.
//!
//! What a listed file does at run time is not provable statically, and the check observes only
//! its own job's compilation: plan 11 states both as the premise and the residual.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

pub(super) mod compiled;
mod workflow;

use crate::release::json::{self, Value};
use crate::release::platform::Platform;
use crate::release::platform_inputs::is_input;
use compiled::Target;
use std::collections::BTreeMap;
use std::path::Path;

const PACKAGE: &str = "stickymd-package";
const CRATE: &str = "tools/stickymd-package/";
/// The workspace's build scripts, as (member, script). The Windows workspace build runs each; no
/// member depends on these members, and `build-app.sh` builds only `stickymd-mac`, so none runs on
/// macOS.
const BUILD_SCRIPTS: &[(&str, &str)] = &[("stickymd-win", "apps/stickymd-win/build.rs")];
const SCRIPTS: &str = "tools/release/";
/// The Windows scripts that build release bytes: those `build-package` reaches, the independent
/// notices entry and the archive-facts helper the packaging crate launches.
const BYTE_SCRIPTS: &[&str] = &[
    "archive-facts.ps1",
    "generate-sbom.ps1",
    "generate-third-party-notices.ps1",
    "invoke-package.ps1",
    "package-archive.ps1",
    "package.ps1",
    "syft-execute.ps1",
    "syft-fetch.ps1",
];
pub(super) const BUILD_APP: &str = "apps/stickymd-mac/packaging/build-app.sh";
/// How a file reaches the verification tool: its package or crate name, its PowerShell wrapper
/// script and function, or its per-phase entry points. Lower case; matched in any case.
const TOOL_NAMES: &[&str] = &[
    "stickymd-smoke",
    "stickymd_smoke",
    "invoke-smoke.ps1",
    "invoke-stickymdreleasetool",
    "tools/smoke/",
    "tools\\smoke\\",
];
/// Regular files in the Git index (`100644`, `100755`); a link records only where it points.
pub(super) const REGULAR_MODES: &[&str] = &["100644", "100755"];

pub(super) type Index = BTreeMap<String, String>;

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let index = git_index(root)?;
    let counted_by_both =
        |file: &str| is_input(Platform::WindowsX64, file) && is_input(Platform::MacOsArm64, file);
    let targets = check_metadata(&cargo_metadata(root)?, &index, &counted_by_both)?;
    check_lock(&String::from_utf8_lossy(&read(root, "Cargo.lock")?))?;
    let compiled = compiled::files(root, PACKAGE, &targets)?;
    check_compiled(&compiled, &index, &counted_by_both)?;
    check_byte_scripts(SCRIPTS, BYTE_SCRIPTS, &index, Platform::WindowsX64)?;
    workflow::check_pinned_set(&index, &|path| {
        is_input(Platform::WindowsX64, path) || is_input(Platform::MacOsArm64, path)
    })?;

    // Every file whose whole text the name search covers, with the rules of its kind.
    let mut searched: Vec<(String, Vec<u8>)> = Vec::new();
    for file in &compiled {
        let bytes = read(root, file)?;
        check_ps1_names(&bytes, BYTE_SCRIPTS).map_err(|e| format!("{file}: {e}"))?;
        searched.push((file.clone(), bytes));
    }
    for name in BYTE_SCRIPTS {
        let path = format!("{SCRIPTS}{name}");
        let bytes = read(root, &path)?;
        check_script_encoding(&bytes).map_err(|e| format!("{path}: {e}"))?;
        check_ps1_names(&bytes, BYTE_SCRIPTS).map_err(|e| format!("{path}: {e}"))?;
        searched.push((path, bytes));
    }
    for (_, script) in BUILD_SCRIPTS {
        let bytes = read(root, script)?;
        check_build_script(&bytes).map_err(|e| format!("{script}: {e}"))?;
        searched.push(((*script).to_owned(), bytes));
    }
    let build_app = read(root, BUILD_APP)?;
    if contains(&build_app, b".ps1") {
        return Err(format!(
            "{BUILD_APP} names a PowerShell script; macOS does not count tools/release"
        ));
    }
    searched.push((BUILD_APP.to_owned(), build_app));
    for (path, pin) in workflow::RELEASE_WORKFLOWS {
        let text =
            String::from_utf8(read(root, path)?).map_err(|_| format!("{path} is not UTF-8"))?;
        let job = workflow::check(&text, pin).map_err(|e| format!("{path}: {e}"))?;
        searched.push((format!("{path} (build-package)"), job.into_bytes()));
    }
    // The product crates and every build script, with any module a build script declares.
    for path in index
        .keys()
        .filter(|path| path.starts_with("apps/") || path.starts_with("crates/"))
    {
        searched.push((path.clone(), read(root, path)?));
    }
    for (origin, bytes) in &searched {
        check_names(bytes).map_err(|e| format!("{origin}: {e}"))?;
    }
    Ok(())
}

fn read(root: &Path, relative: &str) -> Result<Vec<u8>, String> {
    std::fs::read(root.join(relative)).map_err(|e| format!("{relative}: {e}"))
}

/// Whether `haystack` contains `needle` in any ASCII letter case (`needle` in lower case).
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn is_regular(index: &Index, path: &str) -> bool {
    index
        .get(path)
        .is_some_and(|mode| REGULAR_MODES.contains(&mode.as_str()))
}

/// The Git index: each path, exactly as Git records it, with its mode. A file only on disk is not
/// a build input of a clean checkout.
pub(super) fn git_index(root: &Path) -> Result<Index, String> {
    let output = crate::release::platform_inputs::git(root)
        .args(["ls-files", "-z", "--cached", "--stage"])
        .output()
        .map_err(|error| format!("cannot run git ls-files: {error}"))?;
    if !output.status.success() {
        return Err("git ls-files --cached --stage failed".to_owned());
    }
    parse_index(&output.stdout)
}

/// `<mode> <object> <stage>\t<path>` records, each ended by NUL.
pub(super) fn parse_index(stdout: &[u8]) -> Result<Index, String> {
    let mut index = BTreeMap::new();
    for record in stdout.split(|byte| *byte == 0).filter(|r| !r.is_empty()) {
        let record =
            std::str::from_utf8(record).map_err(|_| "a tracked path is not UTF-8".to_owned())?;
        let (fields, path) = record
            .split_once('\t')
            .ok_or_else(|| format!("unexpected git ls-files record {record:?}"))?;
        // Git separates with `/`; a `\` is part of a name, which a Windows checkout cannot hold
        // and another host would read as a different file than the rules compare.
        if path.contains('\\') {
            return Err(format!("the tracked path {path:?} holds a backslash"));
        }
        let mode = fields
            .split(' ')
            .next()
            .ok_or_else(|| format!("unexpected git ls-files record {record:?}"))?;
        index.insert(path.to_owned(), mode.to_owned());
    }
    Ok(index)
}

fn cargo_metadata(root: &Path) -> Result<Value, String> {
    let output = compiled::cargo()
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

/// Cargo's decoded view, so no TOML spelling (a quoted table or key, a target `path`) can hide a
/// dependency, a target or a build script. Returns the packaging crate's lib and bin targets.
fn check_metadata(
    metadata: &Value,
    index: &Index,
    counted_by_both: &dyn Fn(&str) -> bool,
) -> Result<Vec<Target>, String> {
    let workspace_root = format!(
        "{}/",
        compiled::normalize(metadata.field("workspace_root")?.string()?)?.trim_end_matches('/')
    );
    let relative = |path: &str| -> Result<String, String> {
        compiled::normalize(path)?
            .strip_prefix(&workspace_root)
            .map(str::to_owned)
            .ok_or_else(|| format!("{path} is outside the workspace"))
    };
    let mut found = false;
    let mut binaries = std::collections::BTreeSet::new();
    let mut build_scripts = Vec::new();
    let mut targets = Vec::new();
    for package in metadata.field("packages")?.array()? {
        let name = package.field("name")?.string()?;
        // Another member's manifest can change the features Cargo merges into the product.
        let manifest = relative(package.field("manifest_path")?.string()?)?;
        if !is_regular(index, &manifest) || !counted_by_both(&manifest) {
            return Err(format!(
                "{name}'s manifest {manifest} is not a regular tracked file counted by both platforms; a member in an excluded tree needs the classifier changed first"
            ));
        }
        let packaging = name == PACKAGE;
        if packaging {
            found = true;
            if !package.field("dependencies")?.array()?.is_empty() {
                return Err(format!(
                    "{PACKAGE} declares a dependency; the packaging crate is std-only"
                ));
            }
            if !matches!(package.field("features")?, Value::Object(features) if features.is_empty())
            {
                return Err(format!(
                    "{PACKAGE} declares a feature; the workspace's feature merging must not change it"
                ));
            }
        }
        for target in package.field("targets")?.array()? {
            let kinds = target.field("kind")?.array()?;
            let is = |wanted: &str| kinds.iter().any(|kind| kind.string() == Ok(wanted));
            let source = relative(target.field("src_path")?.string()?)?;
            let target_name = target.field("name")?.string()?;
            if is("proc-macro") {
                return Err(format!(
                    "{name} is a procedural macro, which runs inside the compiler; plan 11 admits no such member"
                ));
            }
            if is("bin") {
                // Windows compares file names by its own upcase table; ASCII names compared
                // without case cannot collide there, whatever the table.
                if !target_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                {
                    return Err(format!(
                        "the binary name {target_name:?} is not ASCII letters, digits, `_` and `-`"
                    ));
                }
                if !binaries.insert(target_name.to_ascii_lowercase()) {
                    return Err(format!(
                        "two workspace targets build the binary {target_name}; one could take the other's place in the build output"
                    ));
                }
            }
            if is("custom-build") {
                if packaging {
                    return Err(format!("{PACKAGE} has a build script"));
                }
                build_scripts.push((name.to_owned(), source));
            } else if packaging {
                // Tests, benches and examples ship nothing; `cargo check --lib --bins` skips them.
                if let Some(kind) = compiled::lib_or_bin(kinds) {
                    targets.push(Target {
                        kind: kind.to_owned(),
                        name: target_name.to_owned(),
                        source: source.clone(),
                    });
                }
            }
        }
    }
    if !found {
        return Err(format!("cargo metadata lists no {PACKAGE}"));
    }
    build_scripts.sort();
    let listed: Vec<(String, String)> = BUILD_SCRIPTS
        .iter()
        .map(|(member, script)| ((*member).to_owned(), (*script).to_owned()))
        .collect();
    if build_scripts != listed {
        return Err(format!(
            "the workspace's build scripts are {build_scripts:?}; plan 11 lists exactly {listed:?}"
        ));
    }
    Ok(targets)
}

/// No workspace member, nor anything outside the workspace, depends on a member that has a listed
/// build script: `Cargo.lock` holds every package and dependency edge of the resolved graph, so
/// the owner's name appears there only as its own `name = "…"` line.
fn check_lock(lock: &str) -> Result<(), String> {
    for (owner, _) in BUILD_SCRIPTS {
        let own = format!("name = \"{owner}\"");
        let lines: Vec<&str> = lock.lines().filter(|line| line.contains(owner)).collect();
        if lines != [own.as_str()] {
            return Err(format!(
                "Cargo.lock names {owner} on {lines:?}; plan 11 lets its build script run only in the Windows workspace build, so nothing may depend on it"
            ));
        }
    }
    Ok(())
}

/// A listed script is UTF-8: in another encoding (UTF-16 with or without its mark) a name's bytes
/// need not be contiguous, and the raw searches would miss it.
fn check_script_encoding(bytes: &[u8]) -> Result<(), String> {
    if std::str::from_utf8(bytes).is_err() || bytes.contains(&0) {
        return Err(
            "is not UTF-8 text; in another encoding a name's bytes need not be contiguous"
                .to_owned(),
        );
    }
    Ok(())
}

/// Each file rustc registered compiling the packaging crate is a regular file of the Git index
/// inside the crate, spelled exactly as Git records it, that both platforms count. Exact spelling
/// also refuses `..`, an absolute path and a case variant found on a case-insensitive volume.
fn check_compiled(
    compiled: &[String],
    index: &Index,
    counted_by_both: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    for file in compiled {
        if !file.starts_with(CRATE) {
            return Err(format!("compiling {PACKAGE} reads {file}, outside {CRATE}"));
        }
        match index.get(file) {
            None => {
                return Err(format!(
                    "compiling {PACKAGE} reads {file}, which Git does not track under that spelling"
                ));
            }
            Some(mode) if !REGULAR_MODES.contains(&mode.as_str()) => {
                return Err(format!(
                    "compiling {PACKAGE} reads {file}, which Git records as mode {mode}, not a regular file"
                ));
            }
            Some(_) => {}
        }
        if !counted_by_both(file) {
            return Err(format!(
                "compiling {PACKAGE} reads {file}, which a platform's fingerprint does not count"
            ));
        }
    }
    Ok(())
}

/// The listed scripts are regular files of the Git index in `directory`, spelled as listed, that
/// `platform`'s fingerprint counts.
fn check_byte_scripts(
    directory: &str,
    scripts: &[&str],
    index: &Index,
    platform: Platform,
) -> Result<(), String> {
    for name in scripts {
        let path = format!("{directory}{name}");
        if !is_regular(index, &path) {
            return Err(format!(
                "{path}: a listed script that is not a regular file Git tracks under that spelling"
            ));
        }
        if !is_input(platform, &path) {
            return Err(format!(
                "{path}: a listed script that the {} fingerprint does not count",
                platform.label()
            ));
        }
    }
    Ok(())
}

/// Every `.ps1`, in any letter case, ends a file name (the longest run of ASCII letters, digits,
/// `.`, `_` and `-` before it) that is on the list, spelled as listed; the name starts the file or
/// follows an ASCII byte that is not a path separator, so neither a directory nor a non-ASCII
/// letter can extend it.
fn check_ps1_names(bytes: &[u8], allowed: &[&str]) -> Result<(), String> {
    let name_byte = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-');
    let mut from = 0;
    while let Some(offset) = bytes[from..]
        .windows(4)
        .position(|window| window.eq_ignore_ascii_case(b".ps1"))
    {
        let end = from + offset + 4;
        let mut start = from + offset;
        while start > 0 && name_byte(bytes[start - 1]) {
            start -= 1;
        }
        let name = String::from_utf8_lossy(&bytes[start..end]);
        if !allowed.contains(&name.as_ref()) {
            return Err(format!(
                "names {name}, which is not one of the listed scripts {allowed:?}"
            ));
        }
        if start > 0 && matches!(bytes[start - 1], b'/' | b'\\') {
            return Err(format!(
                "names {name} after a path separator; the listed scripts are named bare"
            ));
        }
        if start > 0 && !bytes[start - 1].is_ascii() {
            return Err(format!(
                "names {name} after a non-ASCII byte, which could be part of a longer name"
            ));
        }
        from = end;
    }
    Ok(())
}

/// A listed build script names no parent directory.
fn check_build_script(bytes: &[u8]) -> Result<(), String> {
    for parent in [&b"../"[..], b"..\\", b"\"..\""] {
        if contains(bytes, parent) {
            return Err(format!(
                "names a parent directory (`{}`); a build script reads only its own member",
                String::from_utf8_lossy(parent)
            ));
        }
    }
    Ok(())
}

/// No name of the verification tool, in any letter case, anywhere in the text.
fn check_names(bytes: &[u8]) -> Result<(), String> {
    match TOOL_NAMES
        .iter()
        .find(|name| contains(bytes, name.as_bytes()))
    {
        Some(name) => Err(format!(
            "names the verification tool (`{name}`, any letter case, comments included)"
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests;
