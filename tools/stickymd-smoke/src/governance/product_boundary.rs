//! The product crates' compile-time inputs (plan 11 #platform-artifact-reuse, "产品发布图"): every
//! file rustc registers compiling the shipped binary's dependency closure is an input the
//! platform's fingerprint counts, so changing only an excluded file (`docs/` and the like) cannot
//! change the release bytes while the fingerprint stays. On a Windows x64 or macOS arm64 host the
//! check repeats that platform's pinned release build as a fresh `cargo check` in a private
//! directory, scopes it with Cargo metadata, and reads each unit's dep-info; elsewhere only the
//! pinned commands are compared. Facts only: Cargo's metadata and messages, rustc's dep-info, the
//! Git index and raw searches; nothing here parses Rust.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::package_boundary::compiled;
use super::package_boundary::{BUILD_APP, Index, REGULAR_MODES, git_index};
use crate::release::json::{self, Value};
use crate::release::platform::Platform;
use crate::release::platform_inputs::is_input;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One platform's pinned release build, as the check repeats it.
pub(super) struct Build {
    pub(super) platform: Platform,
    /// The platform `cargo metadata --filter-platform` resolves for; `None` takes every platform.
    pub(super) triple: Option<&'static str>,
    /// What follows `cargo check --locked --release`: the pinned build's selection and target.
    pub(super) selection: &'static [&'static str],
    /// The member whose binary ships.
    pub(super) shipped: &'static str,
    /// The workspace members the pinned selection compiles outside the shipped closure.
    pub(super) outside: &'static [&'static str],
    /// The pinned build's command, found by raw search in the file that runs it.
    pub(super) pinned: &'static [(&'static str, &'static str)],
}

pub(super) const WINDOWS: Build = Build {
    platform: Platform::WindowsX64,
    triple: Some("x86_64-pc-windows-msvc"),
    // The release job builds the workspace for the host, so features merge over every member.
    selection: &["--workspace"],
    shipped: "stickymd-win",
    outside: &[
        "stickymd-mac",
        "stickymd-package",
        "stickymd-posix",
        "stickymd-smoke",
    ],
    pinned: &[(
        ".github/workflows/release.yml",
        "cargo build --workspace --release --locked",
    )],
};

pub(super) const MACOS: Build = Build {
    platform: Platform::MacOsArm64,
    triple: Some("aarch64-apple-darwin"),
    selection: &["-p", "stickymd-mac", "--target", "aarch64-apple-darwin"],
    shipped: "stickymd-mac",
    outside: &[],
    pinned: &[
        (BUILD_APP, "target=aarch64-apple-darwin\n"),
        (
            BUILD_APP,
            "cargo build -p stickymd-mac --release --locked --target \"$target\"",
        ),
    ],
};

/// The build this host can repeat: each platform checks its own, any other host neither.
fn host_build() -> Option<&'static Build> {
    if cfg!(all(windows, target_arch = "x86_64", target_env = "msvc")) {
        Some(&WINDOWS)
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(&MACOS)
    } else {
        None
    }
}

pub(super) fn verify(root: &Path) -> Result<(), String> {
    for build in [&WINDOWS, &MACOS] {
        check_pinned(root, build)?;
    }
    let Some(build) = host_build() else {
        return Ok(());
    };
    let index = git_index(root)?;
    let files = compiled_files(root, build)?;
    check_files(&files, &index, build.platform)
}

/// The check repeats the pinned build only while that build still runs the same command.
fn check_pinned(root: &Path, build: &Build) -> Result<(), String> {
    for (file, command) in build.pinned {
        let text = std::fs::read(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let found = text
            .windows(command.len())
            .any(|window| window == command.as_bytes());
        if !found {
            return Err(format!(
                "{file} no longer runs `{}`, the build plan 11's product check repeats for {}",
                command.trim_end(),
                build.platform.label()
            ));
        }
    }
    Ok(())
}

/// A unit the closure must compile exactly once: a lib, the shipped bin, or a build script.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Unit {
    package: String,
    /// `lib`, `bin` or `build-script`.
    kind: &'static str,
    name: String,
}

/// The workspace's view for one build: member ids by name, the closure's expected units, and the
/// members outside the closure.
struct Scope {
    members: BTreeMap<String, String>,
    expected: BTreeSet<Unit>,
    closure: BTreeSet<String>,
}

fn cargo_metadata(root: &Path, triple: Option<&str>) -> Result<Value, String> {
    let mut command = compiled::cargo();
    command
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--locked"]);
    if let Some(triple) = triple {
        command.args(["--filter-platform", triple]);
    }
    let output = command
        .output()
        .map_err(|error| format!("cannot start `cargo metadata`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo metadata --locked` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("`cargo metadata` output is not UTF-8: {error}"))?;
    json::parse(&text)
}

/// The members the shipped member reaches through normal and build edges, and the units they
/// must compile: the shipped member's bins, every other member's lib, every build script.
fn scope(metadata: &Value, build: &Build) -> Result<Scope, String> {
    let workspace: BTreeSet<&str> = metadata
        .field("workspace_members")?
        .array()?
        .iter()
        .map(Value::string)
        .collect::<Result<_, _>>()?;
    let mut members = BTreeMap::new();
    let mut targets: BTreeMap<String, &[Value]> = BTreeMap::new();
    for package in metadata.field("packages")?.array()? {
        let id = package.field("id")?.string()?;
        if workspace.contains(id) {
            let name = package.field("name")?.string()?.to_owned();
            targets.insert(id.to_owned(), package.field("targets")?.array()?);
            members.insert(name, id.to_owned());
        }
    }
    let shipped = members
        .get(build.shipped)
        .ok_or_else(|| format!("cargo metadata lists no {}", build.shipped))?
        .clone();
    for name in build.outside {
        if !members.contains_key(*name) {
            return Err(format!(
                "plan 11 lists {name} outside the closure, which is not a workspace member"
            ));
        }
    }
    let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in metadata.field("resolve")?.field("nodes")?.array()? {
        let mut next = Vec::new();
        for dependency in node.field("deps")?.array()? {
            let mut linked = false;
            for kind in dependency.field("dep_kinds")?.array()? {
                match kind.field("kind")? {
                    Value::Null => linked = true,
                    Value::String(kind) if kind == "build" => linked = true,
                    Value::String(kind) if kind == "dev" => {}
                    other => return Err(format!("unexpected dependency kind {other:?}")),
                }
            }
            if linked {
                next.push(dependency.field("pkg")?.string()?);
            }
        }
        edges.insert(node.field("id")?.string()?, next);
    }
    let mut reached = BTreeSet::new();
    let mut stack = vec![shipped.as_str()];
    while let Some(id) = stack.pop() {
        if reached.insert(id) {
            stack.extend(edges.get(id).into_iter().flatten().copied());
        }
    }
    let mut closure = BTreeSet::new();
    let mut expected = BTreeSet::new();
    for (name, id) in &members {
        if !reached.contains(id.as_str()) {
            continue;
        }
        if build.outside.contains(&name.as_str()) {
            return Err(format!(
                "{name} is in {}'s closure; plan 11 lists it outside",
                build.shipped
            ));
        }
        closure.insert(id.clone());
        for target in targets[id] {
            let kinds = target.field("kind")?.array()?;
            let kind = if kinds.iter().any(|k| k.string() == Ok("custom-build")) {
                Some("build-script")
            } else {
                // A member's bins ship only for the shipped member; its lib links into it.
                match compiled::lib_or_bin(kinds) {
                    Some("bin") if *id == shipped => Some("bin"),
                    Some("lib") => Some("lib"),
                    _ => None,
                }
            };
            if let Some(kind) = kind {
                expected.insert(Unit {
                    package: id.clone(),
                    kind,
                    name: target.field("name")?.string()?.to_owned(),
                });
            }
        }
    }
    Ok(Scope {
        members,
        expected,
        closure,
    })
}

/// Repeats `build` afresh in a new private directory and returns the files the closure's units
/// registered.
fn compiled_files(root: &Path, build: &Build) -> Result<Vec<String>, String> {
    let scope = scope(&cargo_metadata(root, build.triple)?, build)?;
    let directory = stickymd_package::temporary::TemporaryDirectory::new("product-boundary")?;
    let files = files_in(root, build, &scope, directory.path())?;
    directory.close()?;
    Ok(files)
}

fn files_in(
    root: &Path,
    build: &Build,
    scope: &Scope,
    directory: &Path,
) -> Result<Vec<String>, String> {
    let output = compiled::cargo()
        .current_dir(root)
        .env("CARGO_BUILD_BUILD_DIR", directory.join("build"))
        .args([
            "check",
            "--locked",
            "--quiet",
            "--release",
            "--message-format=json",
        ])
        .args(build.selection)
        .arg("--target-dir")
        .arg(directory.join("target"))
        .output()
        .map_err(|error| format!("cannot start `cargo check`: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() {
        let messages: Vec<Value> = stdout.lines().filter_map(|l| json::parse(l).ok()).collect();
        return Err(format!(
            "`cargo check --release {}` failed: {}{}",
            build.selection.join(" "),
            String::from_utf8_lossy(&output.stderr).trim(),
            compiled::compiler_errors(&messages)
        ));
    }
    let names: BTreeMap<&str, &str> = scope
        .members
        .iter()
        .map(|(name, id)| (id.as_str(), name.as_str()))
        .collect();
    let mut seen = BTreeSet::new();
    let mut files = BTreeSet::new();
    for line in stdout.lines() {
        let message = json::parse(line)
            .map_err(|e| format!("`cargo check` wrote a line that is not JSON: {e}"))?;
        if message.field("reason").and_then(|r| r.string()).ok() != Some("compiler-artifact") {
            continue;
        }
        let package = message.field("package_id")?.string()?;
        // A package outside the workspace comes from the registry, pinned by `Cargo.lock`.
        let Some(name) = names.get(package) else {
            continue;
        };
        if !scope.closure.contains(package) {
            if build.outside.contains(name) {
                continue;
            }
            return Err(format!(
                "`cargo check` compiled {name}, which is neither in {}'s closure nor listed outside it",
                build.shipped
            ));
        }
        let target = message.field("target")?;
        let kinds = target.field("kind")?.array()?;
        let kind = if kinds.iter().any(|k| k.string() == Ok("custom-build")) {
            "build-script"
        } else {
            compiled::lib_or_bin(kinds)
                .ok_or_else(|| format!("`cargo check` built an unexpected unit of {name}"))?
        };
        let unit = Unit {
            package: package.to_owned(),
            kind,
            name: target.field("name")?.string()?.to_owned(),
        };
        if !scope.expected.contains(&unit) {
            return Err(format!(
                "`cargo check` built the {kind} `{}` of {name}, which is not a unit of {}'s binary",
                unit.name, build.shipped
            ));
        }
        if !matches!(message.field("fresh")?, Value::Bool(false)) {
            return Err(format!(
                "the {kind} `{}` of {name} was not compiled afresh; its dep-info could be from another source state",
                unit.name
            ));
        }
        let paths = dep_info(&message, kind)
            .map_err(|e| format!("the {kind} `{}` of {name}: {e}", unit.name))?;
        if !seen.insert(unit.clone()) {
            return Err(format!(
                "`cargo check` reported the {kind} `{}` of {name} twice",
                unit.name
            ));
        }
        files.extend(paths);
    }
    if let Some(missing) = scope.expected.iter().find(|unit| !seen.contains(*unit)) {
        return Err(format!(
            "`cargo check --release {}` compiled no unit for the {} `{}` of {}",
            build.selection.join(" "),
            missing.kind,
            missing.name,
            names.get(missing.package.as_str()).unwrap_or(&"?")
        ));
    }
    Ok(files.into_iter().collect())
}

/// The files a unit's dep-info registers: next to its `.rmeta` for a lib or bin, in the build
/// directory of the executable Cargo reports for a build script.
fn dep_info(message: &Value, kind: &str) -> Result<Vec<String>, String> {
    let filenames: Vec<&str> = message
        .field("filenames")?
        .array()?
        .iter()
        .filter_map(|name| name.string().ok())
        .collect();
    if kind != "build-script" {
        let rmeta = filenames
            .iter()
            .find(|name| name.ends_with(".rmeta"))
            .ok_or("no .rmeta")?;
        let path = compiled::dep_info_path(Path::new(rmeta))?;
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        return compiled::parse(&text).map_err(|e| format!("{}: {e}", path.display()));
    }
    // On Windows Cargo may also report the program database; the executable is the hard link
    // `build-script-build`, next to the hashed executable and its dep-info.
    let executables: Vec<&&str> = filenames
        .iter()
        .filter(|name| {
            Path::new(name).file_name().is_some_and(|file| {
                let file = file.to_string_lossy().to_ascii_lowercase();
                file == "build-script-build" || file == "build-script-build.exe"
            })
        })
        .collect();
    let [executable] = executables.as_slice() else {
        return Err(format!(
            "Cargo reported {filenames:?}, not one build-script executable"
        ));
    };
    let directory = Path::new(executable)
        .parent()
        .ok_or("the executable has no directory")?;
    let mut dep_infos = Vec::new();
    for entry in
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?
    {
        let entry = entry.map_err(|e| format!("{}: {e}", directory.display()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("build_script_build-") && name.ends_with(".d") {
            dep_infos.push(entry.path());
        }
    }
    let [path] = dep_infos.as_slice() else {
        return Err(format!(
            "{} holds {} build-script dep-infos, not one",
            directory.display(),
            dep_infos.len()
        ));
    };
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let directory = compiled::normalize(&directory.to_string_lossy())?;
    compiled::parse_build_script(&text, &directory).map_err(|e| format!("{}: {e}", path.display()))
}

/// Each registered file is a regular file of the Git index, spelled as Git records it (so no
/// `..`, absolute path or case variant), that `platform`'s fingerprint counts.
fn check_files(files: &[String], index: &Index, platform: Platform) -> Result<(), String> {
    for file in files {
        match index.get(file) {
            None => {
                return Err(format!(
                    "the {} product reads {file} at compile time, which Git does not track under that spelling",
                    platform.label()
                ));
            }
            Some(mode) if !REGULAR_MODES.contains(&mode.as_str()) => {
                return Err(format!(
                    "the {} product reads {file} at compile time, which Git records as mode {mode}, not a regular file",
                    platform.label()
                ));
            }
            Some(_) => {}
        }
        if !is_input(platform, file) {
            return Err(format!(
                "the {} product reads {file} at compile time, which its fingerprint does not count",
                platform.label()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
