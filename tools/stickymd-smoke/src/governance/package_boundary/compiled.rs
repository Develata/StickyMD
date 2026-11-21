//! The files rustc reads compiling a package's lib and bin targets, as rustc itself lists them in
//! the dep-info it writes: source modules, `#[path]` files and everything `include!`,
//! `include_str!` and `include_bytes!` read, however the code spells, renames or expands them.
//! The paths are kept exactly as written there (relative to the workspace root, `/`-separated),
//! so a `..` or an absolute path stays visible to the caller's rules.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use crate::release::json;
use std::path::{Path, PathBuf};

/// The Cargo that runs this tool when there is one (Cargo sets `CARGO` for what it runs), else the
/// one on `PATH`.
pub(super) fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// The profiles the pinned release workflows compile the packaging crate in: `cargo run` (dev)
/// runs it, the Windows workspace build (release) builds it too.
const PROFILES: &[&str] = &["dev", "release"];

/// Checks `package`'s lib and bin targets for the host in the workspace at `root`, in each of
/// [`PROFILES`], with its own `target_dir` so that it never waits on the build directory of a
/// caller's Cargo, and returns the union of their dep-info paths, sorted.
pub(super) fn files(root: &Path, package: &str, target_dir: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for profile in PROFILES {
        files.extend(files_in(root, package, target_dir, profile)?);
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn files_in(
    root: &Path,
    package: &str,
    target_dir: &Path,
    profile: &str,
) -> Result<Vec<String>, String> {
    let output = std::process::Command::new(cargo())
        .current_dir(root)
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
        .arg(target_dir)
        .output()
        .map_err(|error| format!("cannot start `cargo check`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "`cargo check -p {package} --profile {profile}` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "`cargo check` output is not UTF-8".to_owned())?;
    let mut files = Vec::new();
    let mut artifacts = 0;
    for line in stdout.lines() {
        let Ok(message) = json::parse(line) else {
            continue;
        };
        if message.field("reason").and_then(|r| r.string()).ok() != Some("compiler-artifact") {
            continue;
        }
        let rmeta = message
            .field("filenames")?
            .array()?
            .iter()
            .filter_map(|name| name.string().ok())
            .find(|name| name.ends_with(".rmeta"))
            .ok_or_else(|| format!("a `cargo check` artifact of {package} has no .rmeta"))?
            .to_owned();
        let dep_info = dep_info_path(Path::new(&rmeta))?;
        let text = std::fs::read_to_string(&dep_info)
            .map_err(|e| format!("{}: {e}", dep_info.display()))?;
        files.extend(parse(&text).map_err(|e| format!("{}: {e}", dep_info.display()))?);
        artifacts += 1;
    }
    if artifacts == 0 || files.is_empty() {
        return Err(format!(
            "`cargo check -p {package} --profile {profile}` reported no compiled files; a check of nothing proves nothing"
        ));
    }
    Ok(files)
}

/// `deps/libNAME-HASH.rmeta` → `deps/NAME-HASH.d`, where rustc writes the dep-info.
fn dep_info_path(rmeta: &Path) -> Result<PathBuf, String> {
    let stem = rmeta
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.strip_prefix("lib"))
        .ok_or_else(|| format!("unexpected artifact name {}", rmeta.display()))?;
    Ok(rmeta.with_file_name(format!("{stem}.d")))
}

/// The dependencies of a Makefile-style dep-info: rustc writes one `path:` line per file it read
/// (besides the rules that list them all). `#` lines (`# env-dep:`) are environment reads. An
/// escaped space or a `$` is refused rather than unescaped; repository paths have neither.
pub(super) fn parse(text: &str) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') || line.contains(": ") {
            continue;
        }
        let Some(path) = line.strip_suffix(':') else {
            return Err(format!("unexpected dep-info line {line:?}"));
        };
        if path.contains("\\ ") || path.contains('$') || path.is_empty() {
            return Err(format!("dep-info path {path:?} needs unescaping"));
        }
        paths.push(path.replace('\\', "/"));
    }
    Ok(paths)
}
