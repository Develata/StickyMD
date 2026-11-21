//! The macOS app's product input fingerprint (plan 11 #phase-16-macos-host): every tracked file
//! that can reach the app `build-app.sh` builds, with the release version normalized as for the
//! functional qualification modules, followed by the build identity (toolchain, SDK, target,
//! profile). `build-app.sh` writes it into the bundle's Info.plist, so it is part of the release
//! bytes and lives in this crate; `stickymd-smoke` reads and compares it through the same code.
//!
//! The stream is the v1 macOS app stream the functional module fingerprints also use: a header
//! with the algorithm version and the name `mac-app`, then for every input in sorted order its
//! name length, name, content length and content (all lengths little-endian `u64`), then the
//! identity's length and bytes. Changing any of it changes every recorded fingerprint.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

pub mod build;
mod classify;
pub mod normalize;

pub use classify::is_input;
pub use normalize::VersionNormalizer;

use crate::temporary::TemporaryDirectory;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process::Command;

const HEADER: &[u8] = b"StickyMD macOS app input fingerprint v1\0";
const NAME: &str = "mac-app";

/// The tracked and untracked-but-not-ignored files of the checkout at `root`, `/`-separated and
/// sorted: what a build of the worktree can read.
pub fn tracked_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start git ls-files: {error}"))?;
    if !output.status.success() {
        return Err("git ls-files failed while fingerprinting qualification inputs".to_owned());
    }
    let mut files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map(|path| path.replace('\\', "/"))
                .map_err(|error| format!("tracked path is not UTF-8: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    Ok(files)
}

/// The per-file record of every v1 fingerprint stream: name length, name, length, bytes.
pub fn write_entry(writer: &mut impl Write, relative: &str, content: &[u8]) -> io::Result<()> {
    let name = relative.as_bytes();
    writer.write_all(&(name.len() as u64).to_le_bytes())?;
    writer.write_all(name)?;
    writer.write_all(&(content.len() as u64).to_le_bytes())?;
    writer.write_all(content)
}

/// The product input fingerprint of the checkout at `root` for a build described by
/// `build_identity` (see [`build::identity`]).
pub fn fingerprint(root: &Path, build_identity: &[u8]) -> Result<String, String> {
    let tracked = tracked_files(root)?;
    let scratch = TemporaryDirectory::new("app-inputs")?;
    let path = scratch.path().join("stream");
    let file = File::create_new(&path)
        .map_err(|error| format!("cannot create the app input stream: {error}"))?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);
    write_stream(root, &tracked, build_identity, &mut writer)?;
    // A private scratch stream needs completed writes, not durable publication.
    writer.flush().map_err(io_error)?;
    drop(writer);
    let digest = crate::integrity::sha256(&path)?;
    scratch.close()?;
    Ok(digest)
}

fn write_stream(
    root: &Path,
    tracked: &[String],
    build_identity: &[u8],
    writer: &mut impl Write,
) -> Result<(), String> {
    writer.write_all(HEADER).map_err(io_error)?;
    writer.write_all(NAME.as_bytes()).map_err(io_error)?;
    writer.write_all(&[0]).map_err(io_error)?;
    let normalizer = VersionNormalizer::read(root, tracked);
    for relative in tracked.iter().filter(|relative| is_input(relative)) {
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(normalizer) = normalizer.as_ref()
            && VersionNormalizer::applies_to(relative)
        {
            let raw = fs::read(&path)
                .map_err(|error| format!("cannot read app input {relative}: {error}"))?;
            write_entry(writer, relative, &normalizer.normalize(relative, &raw))
                .map_err(io_error)?;
            continue;
        }
        let mut input = File::open(&path)
            .map_err(|error| format!("cannot open app input {relative}: {error}"))?;
        let length = input
            .metadata()
            .map_err(|error| format!("cannot stat app input {relative}: {error}"))?
            .len();
        let name = relative.as_bytes();
        writer
            .write_all(&(name.len() as u64).to_le_bytes())
            .and_then(|()| writer.write_all(name))
            .and_then(|()| writer.write_all(&length.to_le_bytes()))
            .map_err(io_error)?;
        let copied = io::copy(&mut input, writer)
            .map_err(|error| format!("cannot hash app input {relative}: {error}"))?;
        if copied != length {
            return Err(format!("app input {relative} changed length while reading"));
        }
    }
    writer
        .write_all(&(build_identity.len() as u64).to_le_bytes())
        .and_then(|()| writer.write_all(build_identity))
        .map_err(io_error)
}

/// The product input fingerprint of this checkout as `build-app.sh` builds it: the tracked inputs
/// of the macOS app, then the build identity (the Cargo and Rust toolchain, the Xcode tools). A
/// variable or a Cargo configuration outside the repository that could change the build
/// unrecorded is refused; `build-app.sh` runs this with a clean environment. `home` is the
/// caller's home directory, which a Cargo configuration's `[env]` cannot rewrite.
pub fn compute(root: &Path, home: Option<&Path>) -> Result<String, String> {
    let refused = build::refused(std::env::vars());
    if !refused.is_empty() {
        return Err(format!(
            "unset {}: they change the app's build but are not part of its input fingerprint",
            refused.join(", ")
        ));
    }
    // Both streams: `ld -v` reports its version on stderr.
    let output = |program: &str, args: &[&str]| -> Result<String, String> {
        let output = Command::new(program)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if !output.status.success() {
            return Err(format!("{program} {} failed", args.join(" ")));
        }
        let mut text = String::from_utf8(output.stdout).map_err(|e| format!("{program}: {e}"))?;
        text.push_str(&String::from_utf8(output.stderr).map_err(|e| format!("{program}: {e}"))?);
        Ok(text)
    };
    let xcrun = "/usr/bin/xcrun";
    // A home given by the caller wins over this process's environment, which a Cargo
    // configuration's `[env]` can rewrite before this program starts.
    let cargo_home = match home {
        Some(home) => home.join(".cargo"),
        None => build::cargo_home(
            std::env::var_os("CARGO_HOME"),
            std::env::var_os("HOME"),
            &std::env::current_dir().map_err(|e| format!("current directory: {e}"))?,
        )
        .ok_or("neither CARGO_HOME nor a non-empty HOME is set")?,
    };
    let outside = build::outside_cargo_configs(root, &cargo_home);
    if !outside.is_empty() {
        return Err(format!(
            "remove or move {}: Cargo configuration outside the repository can change the build but is not part of its input fingerprint",
            outside
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let tools = [
        ("cargo -V", output("cargo", &["-V"])?),
        ("rustc -vV", output("rustc", &["-vV"])?),
        ("xcode-select -p", output("/usr/bin/xcode-select", &["-p"])?),
        ("sdk path", output(xcrun, &["--show-sdk-path"])?),
        ("sdk version", output(xcrun, &["--show-sdk-version"])?),
        ("clang --version", output(xcrun, &["clang", "--version"])?),
        ("ld -v", output(xcrun, &["ld", "-v"])?),
    ];
    fingerprint(root, &build::identity(&tools))
}

fn io_error(error: io::Error) -> String {
    format!("app input stream I/O failed: {error}")
}

#[cfg(test)]
mod tests;
