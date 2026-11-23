//! The product crates' compile-time inputs (plan 11 #platform-artifact-reuse, "产品发布图" and
//! "Cargo 配置"): every file the Rust front end registers compiling the shipped binary's
//! dependency closure is an input the platform's fingerprint counts, so changing only an excluded
//! file (`docs/` and the like) cannot change what rustc reads while the fingerprint stays. On a
//! Windows x64 or macOS arm64 host the check repeats that platform's pinned release build as a
//! fresh `cargo check` in a private directory, scopes it with Cargo metadata and reads each unit's
//! dep-info; on every host it compares the macOS build script and the Cargo configuration with
//! their pinned copies. Facts only: Cargo's metadata and messages, rustc's dep-info, the Git
//! index, pinned copies and raw searches; nothing here parses Rust, shell or TOML.
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
    /// The host member, whose one bin target, named like it, ships.
    pub(super) shipped: &'static str,
    /// The workspace members the pinned selection compiles outside the shipped closure.
    pub(super) outside: &'static [&'static str],
    /// The file that runs the pinned build and its one `cargo build` line, trimmed.
    pub(super) command: (&'static str, &'static str),
    /// Where, under the build directory, Cargo puts the units for this platform's target: a host
    /// build (no `--target`, no effective `build.target`) uses `release/deps`.
    pub(super) deps: &'static str,
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
    command: (
        ".github/workflows/release.yml",
        "run: cargo build --workspace --release --locked",
    ),
    deps: "release/deps",
};

pub(super) const MACOS: Build = Build {
    platform: Platform::MacOsArm64,
    triple: Some("aarch64-apple-darwin"),
    selection: &["-p", "stickymd-mac", "--target", "aarch64-apple-darwin"],
    shipped: "stickymd-mac",
    outside: &[],
    command: (
        BUILD_APP,
        "clean cargo build -p stickymd-mac --release --locked --target \"$target\" --target-dir \"$repo/target\"",
    ),
    deps: "aarch64-apple-darwin/release/deps",
};

/// The one line of the macOS build script that sets the target its `cargo build` uses.
const MACOS_TARGET_LINE: &str = "target=aarch64-apple-darwin";
/// The reviewed macOS build script; the Windows workflow has its own pin (`workflow` module).
const BUILD_APP_PIN: &str = include_str!("product_boundary/pins/build-app.sh");
const CARGO_CONFIG: &str = ".cargo/config.toml";
/// The reviewed Cargo configuration: no `include`, no `@file` argument.
const CARGO_CONFIG_PIN: &str = include_str!("product_boundary/pins/cargo-config.toml");

/// The build this host repeats: the Windows x64 build on Windows, the macOS arm64 build on macOS,
/// none on another system. The compiler's own host must be the platform's triple, whatever the
/// checker was compiled for or the processor emulates: the Windows build names no `--target`,
/// and procedural macros run on the host, so a cross check could register other files.
fn host_build(root: &Path) -> Result<Option<&'static Build>, String> {
    let build = if cfg!(windows) {
        &WINDOWS
    } else if cfg!(target_os = "macos") {
        &MACOS
    } else {
        return Ok(None);
    };
    let host = rustc_host(root)?;
    if Some(host.as_str()) == build.triple {
        Ok(Some(build))
    } else {
        Err(format!(
            "the compiler's host is {host}; plan 11's product check for {} needs a {} compiler",
            build.platform.label(),
            build.triple.unwrap_or("?")
        ))
    }
}

/// The `host:` line of `-vV` from the compiler Cargo starts: `RUSTC` when set, otherwise `rustc`
/// from `PATH` in the workspace (through rustup, the pinned toolchain). A `build.rustc` in Cargo
/// home is a premise of plan 11, not observed here.
fn rustc_host(root: &Path) -> Result<String, String> {
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = std::process::Command::new(compiler)
        .current_dir(root)
        .arg("-vV")
        .output()
        .map_err(|error| format!("cannot start `rustc -vV`: {error}"))?;
    if !output.status.success() {
        return Err("`rustc -vV` failed".to_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "`rustc -vV` names no host".to_owned())
}

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let index = git_index(root)?;
    let build_app = read_text(root, BUILD_APP)?;
    check_pin(BUILD_APP, &build_app, BUILD_APP_PIN)?;
    check_cargo_config(&index, &read_text(root, CARGO_CONFIG)?)?;
    check_command(
        &read_text(root, WINDOWS.command.0)?,
        WINDOWS.command.1,
        None,
    )
    .map_err(|e| format!("{}: {e}", WINDOWS.command.0))?;
    check_command(&build_app, MACOS.command.1, Some(MACOS_TARGET_LINE))
        .map_err(|e| format!("{BUILD_APP}: {e}"))?;
    let Some(build) = host_build(root)? else {
        return Ok(());
    };
    let files = compiled_files(root, build)?;
    check_files(&files, &index, build.platform)
}

fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    let bytes = std::fs::read(root.join(relative)).map_err(|e| format!("{relative}: {e}"))?;
    String::from_utf8(bytes).map_err(|_| format!("{relative} is not UTF-8"))
}

/// `text` equals the reviewed copy once both end lines with LF.
fn check_pin(path: &str, text: &str, pin: &str) -> Result<(), String> {
    if text.replace("\r\n", "\n") == pin.replace("\r\n", "\n") {
        Ok(())
    } else {
        Err(format!(
            "{path} differs from its pinned copy; plan 11 has the product check repeat the reviewed build, so change both"
        ))
    }
}

/// The repository's Cargo configuration is exactly `.cargo/config.toml`, as reviewed. The
/// directory is matched without ASCII case: on a case-insensitive volume Cargo reads
/// `.CARGO/config` as `.cargo/config`, and prefers it.
fn check_cargo_config(index: &Index, text: &str) -> Result<(), String> {
    let files: Vec<&String> = index
        .keys()
        .filter(|path| {
            path.split('/')
                .next()
                .is_some_and(|first| first.eq_ignore_ascii_case(".cargo"))
        })
        .collect();
    if files != [CARGO_CONFIG] || !REGULAR_MODES.contains(&index[CARGO_CONFIG].as_str()) {
        return Err(format!(
            "the tracked Cargo configuration is {files:?}; plan 11 admits exactly the regular file {CARGO_CONFIG}"
        ));
    }
    check_pin(CARGO_CONFIG, text, CARGO_CONFIG_PIN)
}

/// Exactly one line runs `cargo build`, and it is `command` (both trimmed); with `target`, exactly
/// one line starts with `target=`, and it is that one. A comment keeping an old command, a flag
/// added on the same line or a second build all fail.
fn check_command(text: &str, command: &str, target: Option<&str>) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let builds: Vec<&&str> = lines
        .iter()
        .filter(|line| line.contains("cargo build"))
        .collect();
    if builds != [&command] {
        return Err(format!(
            "runs `cargo build` on {builds:?}; plan 11's product check repeats exactly `{command}`"
        ));
    }
    if let Some(target) = target {
        let targets: Vec<&&str> = lines
            .iter()
            .filter(|line| line.starts_with("target="))
            .collect();
        if targets != [&target] {
            return Err(format!(
                "sets the target on {targets:?}; plan 11's product check repeats `{target}`"
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

/// The workspace's view for one build: member ids by name, the closure's expected units with
/// their entry files, and the closure's member ids.
struct Scope {
    members: BTreeMap<String, String>,
    expected: BTreeMap<Unit, String>,
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
/// must compile: the shipped member's one bin named like it and any lib, every other member's
/// lib, every build script. No local package outside the workspace may be in the graph.
fn scope(metadata: &Value, build: &Build) -> Result<Scope, String> {
    let workspace_root = format!(
        "{}/",
        compiled::normalize(metadata.field("workspace_root")?.string()?)?.trim_end_matches('/')
    );
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
        let name = package.field("name")?.string()?;
        if workspace.contains(id) {
            targets.insert(id.to_owned(), package.field("targets")?.array()?);
            members.insert(name.to_owned(), id.to_owned());
        } else if matches!(package.field("source")?, Value::Null) {
            // A path package outside the workspace (through `[patch]`, or a path dependency
            // that `workspace.exclude` keeps out): no member rule and no dep-info check sees it.
            return Err(format!(
                "{name} ({id}) is a local package outside the workspace; plan 11 admits none"
            ));
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
    let mut expected = BTreeMap::new();
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
        let mut bins = Vec::new();
        for target in targets[id] {
            let kinds = target.field("kind")?.array()?;
            let target_name = target.field("name")?.string()?;
            let kind = if kinds.iter().any(|k| k.string() == Ok("custom-build")) {
                Some("build-script")
            } else {
                match compiled::lib_or_bin(kinds) {
                    Some("bin") if *id == shipped => {
                        bins.push(target_name.to_owned());
                        Some("bin")
                    }
                    // A dependency's bins are not built for the shipped binary; its lib links in.
                    Some("lib") => Some("lib"),
                    _ => None,
                }
            };
            if let Some(kind) = kind {
                let entry = compiled::normalize(target.field("src_path")?.string()?)?
                    .strip_prefix(&workspace_root)
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!("{name}'s target {target_name} is outside the workspace")
                    })?;
                expected.insert(
                    Unit {
                        package: id.clone(),
                        kind,
                        name: target_name.to_owned(),
                    },
                    entry,
                );
            }
        }
        // The pinned packaging step takes the binary by this name, so the check must be about it.
        if *id == shipped && bins != [build.shipped] {
            return Err(format!(
                "{} has the bin targets {bins:?}; plan 11 ships exactly one, named {}",
                build.shipped, build.shipped
            ));
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
    let mut units = Vec::new();
    for line in stdout.lines() {
        let message = json::parse(line)
            .map_err(|e| format!("`cargo check` wrote a line that is not JSON: {e}"))?;
        if message.field("reason").and_then(|r| r.string()).ok() != Some("compiler-artifact") {
            continue;
        }
        let package = message.field("package_id")?.string()?;
        // Not a member and, by `scope`, not local either: a registry or git package, whose
        // content `Cargo.lock` pins.
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
        let Some(entry) = scope.expected.get(&unit) else {
            return Err(format!(
                "`cargo check` built the {kind} `{}` of {name}, which is not a unit of {}'s binary",
                unit.name, build.shipped
            ));
        };
        if !matches!(message.field("fresh")?, Value::Bool(false)) {
            return Err(format!(
                "the {kind} `{}` of {name} was not compiled afresh; its dep-info could be from another source state",
                unit.name
            ));
        }
        if !seen.insert(unit.clone()) {
            return Err(format!(
                "`cargo check` reported the {kind} `{}` of {name} twice",
                unit.name
            ));
        }
        units.push((unit, entry, name, message));
    }
    // The set of units first, so a target compiled twice is reported as such, whichever copy
    // Cargo reports first; then each unit's dep-info.
    if let Some(missing) = scope.expected.keys().find(|unit| !seen.contains(*unit)) {
        return Err(format!(
            "`cargo check --release {}` compiled no unit for the {} `{}` of {}",
            build.selection.join(" "),
            missing.kind,
            missing.name,
            names.get(missing.package.as_str()).unwrap_or(&"?")
        ));
    }
    let deps = format!(
        "{}/{}",
        compiled::normalize(&directory.join("build").to_string_lossy())?,
        build.deps
    );
    let mut files = BTreeSet::new();
    for (unit, entry, name, message) in &units {
        let paths = dep_info(message, unit.kind, &deps)
            .and_then(|paths| require_entry(paths, entry))
            .map_err(|e| format!("the {} `{}` of {name}: {e}", unit.kind, unit.name))?;
        files.extend(paths);
    }
    Ok(files.into_iter().collect())
}

/// A dep-info belongs to its unit only if it registers the unit's own entry file.
fn require_entry(paths: Vec<String>, entry: &str) -> Result<Vec<String>, String> {
    if paths.iter().any(|path| path == entry) {
        Ok(paths)
    } else {
        Err(format!("its dep-info does not name its entry {entry}"))
    }
}

/// The files a unit's dep-info registers: next to its `.rmeta` for a lib or bin; for a build
/// script, the one hashed dep-info in the directory of the executable Cargo reports, whose rules
/// may name only that dep-info and the hashed executable beside it.
fn dep_info(message: &Value, kind: &str, deps: &str) -> Result<Vec<String>, String> {
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
        // A unit compiled for another target (an effective `build.target`) lands elsewhere.
        let directory = Path::new(rmeta)
            .parent()
            .ok_or("the .rmeta has no directory")?;
        if compiled::normalize(&directory.to_string_lossy())? != deps {
            return Err(format!(
                "Cargo put it in {}, not in {deps}, where this platform's target puts it",
                directory.display()
            ));
        }
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
    let dep_info = compiled::normalize(&path.to_string_lossy())?;
    compiled::parse_build_script(&text, &dep_info).map_err(|e| format!("{}: {e}", path.display()))
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
