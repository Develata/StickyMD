//! The Platform Build Input Fingerprint and the reuse rule built on it: a release commit may carry
//! a platform's artifact built on an ancestor when that platform's inputs are identical on both.
//! The fingerprint reads Git trees, never the worktree, so it is defined for any commit.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::platform::Platform;
use super::temporary::TemporaryDirectory;
use crate::integrity;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// The algorithm version: it names the serialization and this exclusion table, so a change to
/// either bumps it and values of two versions are never compared (plan 11
/// #platform-artifact-reuse). v2 stopped counting the verification tool.
pub(crate) const ALGORITHM: &str = "v2";

/// Paths that neither enter any platform's artifact nor produce its bytes.
const COMMON_EXCLUDED_PREFIXES: &[&str] = &[
    // Documentation: the package README is generated from a template in the tool.
    "docs/",
    // Not workspace members.
    "experiments/",
    // Steps for people.
    "tools/manual/",
    // Pictures in the repository README.
    "assets/readme/",
    // The verification tool. The release bytes come from `tools/stickymd-package`, which it
    // depends on and which cannot depend on it; its manifest stays an input (see below).
    "tools/stickymd-smoke/",
];
const COMMON_EXCLUDED_FILES: &[&str] = &[
    "AGENTS.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "README.md",
    "README.en.md",
    "README.zh-CN.md",
    "SECURITY.md",
];
/// Compiled only for macOS: on Windows these crates are empty. Their manifests are not excluded
/// (see [`is_input`]).
const WINDOWS_EXCLUDED_PREFIXES: &[&str] = &["apps/stickymd-mac/", "crates/stickymd-posix/"];
/// The Windows host and its PowerShell packaging scripts.
const MACOS_EXCLUDED_PREFIXES: &[&str] = &["apps/stickymd-win/", "tools/release/"];
/// Files under excluded directories that are inputs of both platforms anyway:
/// - every workspace member's manifest: another member's manifest can change the feature set Cargo
///   merges without changing `Cargo.lock`, and the macOS product input fingerprint written into
///   the bundle reads every member's manifest;
/// - every member's build script: Cargo finds `build.rs` without a manifest entry and runs it when
///   it compiles the member, which the Windows workspace build does for every member (governance
///   checks with Cargo metadata that a member's build script can only be the `build.rs` beside its
///   manifest, and that it is self-contained);
/// - this classifier: the exclusion table and the serialization are the reuse rule itself, so a
///   change to them is never "only the verification tool".
///
/// The other members' manifests and build scripts are inputs anyway.
const COUNTED_DESPITE_EXCLUSION: &[&str] = &[
    "apps/stickymd-mac/Cargo.toml",
    "apps/stickymd-mac/build.rs",
    "apps/stickymd-win/Cargo.toml",
    "apps/stickymd-win/build.rs",
    "crates/stickymd-posix/Cargo.toml",
    "crates/stickymd-posix/build.rs",
    "tools/stickymd-smoke/Cargo.toml",
    "tools/stickymd-smoke/build.rs",
    "tools/stickymd-smoke/src/release/platform_inputs.rs",
];

/// Whether `path` (repository-relative, `/`-separated) is an input of `platform`'s artifact.
/// Anything not listed as excluded is an input, a new path included.
pub(crate) fn is_input(platform: Platform, path: &str) -> bool {
    is_input_path(platform, path.as_bytes())
}

/// [`is_input`] on the raw path bytes Git stores, so an excluded path is excluded whatever its
/// encoding.
fn is_input_path(platform: Platform, path: &[u8]) -> bool {
    if let Some(rest) = path.strip_prefix(b".github/") {
        // Only the platform's own release workflow builds; CI, schedules and promotion do not.
        return rest == own_workflow(platform).as_bytes();
    }
    let listed = |names: &[&str]| names.iter().any(|name| name.as_bytes() == path);
    let under = |prefixes: &[&str]| {
        prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix.as_bytes()))
    };
    // Case-insensitively: on a case-insensitive volume (Windows, macOS by default) Cargo finds
    // `Build.rs` when it looks for `build.rs`, so every spelling of a counted file is counted.
    if COUNTED_DESPITE_EXCLUSION
        .iter()
        .any(|name| name.as_bytes().eq_ignore_ascii_case(path))
    {
        return true;
    }
    if listed(COMMON_EXCLUDED_FILES) || under(COMMON_EXCLUDED_PREFIXES) {
        return false;
    }
    !under(match platform {
        Platform::WindowsX64 => WINDOWS_EXCLUDED_PREFIXES,
        Platform::MacOsArm64 => MACOS_EXCLUDED_PREFIXES,
    })
}

/// Git on `root` reading the history as committed: a replacement ref cannot substitute another
/// commit, tree or blob for the one a SHA names, and a graft file cannot give a commit other
/// parents (the null device is an empty graft file). Every release and qualification query of
/// commits, trees, blobs and ancestry goes through this.
pub(crate) fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env(
            "GIT_GRAFT_FILE",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        );
    command
}

/// `commit` must be a full lowercase SHA naming a commit object of this repository: an
/// annotated tag's own id would peel to a commit, and a tree id would list a tree, under a
/// name that claims a commit.
pub(crate) fn require_commit(root: &Path, label: &str, commit: &str) -> Result<(), String> {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be a full lowercase commit SHA: {commit}"
        ));
    }
    let is_commit = git(root)
        .args(["cat-file", "-t", commit])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"commit\n");
    if is_commit {
        Ok(())
    } else {
        Err(format!("{commit} is not a commit in this repository"))
    }
}

fn own_workflow(platform: Platform) -> &'static str {
    match platform {
        Platform::WindowsX64 => "workflows/release.yml",
        Platform::MacOsArm64 => "workflows/release-macos.yml",
    }
}

struct Entry {
    mode: String,
    kind: String,
    object: String,
    path: Vec<u8>,
}

/// The tree of `commit`, every entry with its mode, kind and object id, sorted by path bytes.
fn tree(root: &Path, commit: &str) -> Result<Vec<Entry>, String> {
    let output = git(root)
        .args(["ls-tree", "-r", "-z", "--full-tree", commit])
        .output()
        .map_err(|error| format!("cannot run git ls-tree: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree {commit} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut entries = Vec::new();
    for record in output.stdout.split(|byte| *byte == 0) {
        if record.is_empty() {
            continue;
        }
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or("malformed git ls-tree record")?;
        let header = std::str::from_utf8(&record[..tab]).map_err(|_| "malformed git ls-tree")?;
        let mut fields = header.split(' ');
        let (Some(mode), Some(kind), Some(object), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("malformed git ls-tree header `{header}`"));
        };
        entries.push(Entry {
            mode: mode.to_owned(),
            kind: kind.to_owned(),
            object: object.to_owned(),
            path: record[tab + 1..].to_vec(),
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

/// The SHA-256 of `platform`'s build inputs in `commit`: a stream of the algorithm version and
/// platform, then for every input path in byte order its path, mode, length and content.
pub(crate) fn fingerprint(root: &Path, platform: Platform, commit: &str) -> Result<String, String> {
    require_commit(root, "the fingerprinted commit", commit)?;
    let mut inputs = Vec::new();
    for entry in tree(root, commit)? {
        if !is_input_path(platform, &entry.path) {
            continue;
        }
        // An input is a regular file. A symbolic link's blob is only its target's name, so the
        // target's bytes would escape the fingerprint; a submodule's content is not in this tree.
        if entry.kind != "blob" || !matches!(entry.mode.as_str(), "100644" | "100755") {
            return Err(format!(
                "{} input {} in {commit} is a {} of mode {}, not a regular file",
                platform.label(),
                String::from_utf8_lossy(&entry.path),
                entry.kind,
                entry.mode
            ));
        }
        inputs.push(entry);
    }
    let scratch = TemporaryDirectory::new("platform-inputs")?;
    let stream_path = scratch.path().join("stream");
    let mut stream = BufWriter::new(
        File::create(&stream_path).map_err(|error| format!("cannot create the stream: {error}"))?,
    );
    let io = |error: std::io::Error| format!("cannot write the input stream: {error}");
    stream
        .write_all(format!("StickyMD platform build inputs {ALGORITHM}\0").as_bytes())
        .map_err(io)?;
    stream.write_all(platform.label().as_bytes()).map_err(io)?;
    stream.write_all(b"\0").map_err(io)?;
    write_blobs(root, &inputs, &mut stream)?;
    stream.flush().map_err(io)?;
    drop(stream);
    let digest = integrity::sha256(&stream_path)?;
    scratch.close()?;
    Ok(digest)
}

/// Streams every entry's content from one `git cat-file --batch`, writing the ids from another
/// thread so neither pipe can fill while the other waits.
fn write_blobs(root: &Path, entries: &[Entry], stream: &mut impl Write) -> Result<(), String> {
    let mut child = git(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run git cat-file: {error}"))?;
    let mut input = child.stdin.take().ok_or("git cat-file has no stdin")?;
    let ids = entries
        .iter()
        .map(|entry| format!("{}\n", entry.object))
        .collect::<String>();
    let writer = std::thread::spawn(move || input.write_all(ids.as_bytes()));
    let mut output = BufReader::new(child.stdout.take().ok_or("git cat-file has no stdout")?);
    let result = (|| {
        let io = |error: std::io::Error| format!("cannot read git cat-file: {error}");
        for entry in entries {
            let mut header = String::new();
            output.read_line(&mut header).map_err(io)?;
            let mut fields = header.trim_end().split(' ');
            let (Some(object), Some("blob"), Some(size)) =
                (fields.next(), fields.next(), fields.next())
            else {
                return Err(format!(
                    "unexpected git cat-file header `{}`",
                    header.trim_end()
                ));
            };
            if object != entry.object {
                return Err(format!(
                    "git cat-file answered {object} for {}",
                    entry.object
                ));
            }
            let size: u64 = size.parse().map_err(|_| "invalid git cat-file size")?;
            stream.write_all(&entry.path).map_err(io)?;
            stream.write_all(b"\0").map_err(io)?;
            stream.write_all(entry.mode.as_bytes()).map_err(io)?;
            stream.write_all(b"\0").map_err(io)?;
            stream.write_all(&size.to_le_bytes()).map_err(io)?;
            let copied = std::io::copy(&mut (&mut output).take(size), stream).map_err(io)?;
            if copied != size {
                return Err(format!("git cat-file ended inside {}", entry.object));
            }
            let mut newline = [0_u8; 1];
            output.read_exact(&mut newline).map_err(io)?;
            if newline != *b"\n" {
                return Err("git cat-file record does not end with a newline".to_owned());
            }
        }
        Ok(())
    })();
    drop(output);
    if result.is_err() {
        // Git may be blocked writing to the pipe just closed while the writer thread waits for
        // Git to read: stop Git first, so the join below cannot wait forever.
        let _ = child.kill();
    }
    let written = writer
        .join()
        .map_err(|_| "the git cat-file writer panicked")?;
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for git cat-file: {error}"))?;
    result?;
    written.map_err(|error| format!("cannot write to git cat-file: {error}"))?;
    if !status.success() {
        return Err(format!("git cat-file failed with {status}"));
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Reuse {
    /// The artifact was built on the release commit itself.
    SameCommit,
    /// Built on an ancestor whose inputs for this platform equal the release commit's.
    Ancestor { fingerprint: String },
}

/// Whether `release` may carry `platform`'s artifact built on `build` (plan 11 reuse rule): the
/// same commit, or an ancestor with the same Platform Build Input Fingerprint.
pub(crate) fn verify_reuse(
    root: &Path,
    platform: Platform,
    build: &str,
    release: &str,
) -> Result<Reuse, String> {
    require_commit(root, "build source", build)?;
    require_commit(root, "release commit", release)?;
    if build == release {
        return Ok(Reuse::SameCommit);
    }
    let ancestor = git(root)
        .args(["merge-base", "--is-ancestor", build, release])
        .status()
        .map_err(|error| format!("cannot run git merge-base: {error}"))?;
    match ancestor.code() {
        Some(0) => {}
        Some(1) => {
            return Err(format!(
                "{} artifact built on {build} cannot be reused: it is not an ancestor of {release}",
                platform.label()
            ));
        }
        _ => return Err(format!("git merge-base failed for {build} and {release}")),
    }
    let built = fingerprint(root, platform, build)?;
    let released = fingerprint(root, platform, release)?;
    if built != released {
        return Err(format!(
            "{} artifact built on {build} cannot be reused on {release}: its build inputs differ ({})",
            platform.label(),
            changed_inputs(root, platform, build, release)?.join(", ")
        ));
    }
    Ok(Reuse::Ancestor {
        fingerprint: released,
    })
}

/// The input paths `git diff` reports between the two commits, for the refusal message.
fn changed_inputs(
    root: &Path,
    platform: Platform,
    from: &str,
    to: &str,
) -> Result<Vec<String>, String> {
    let output = git(root)
        .args(["diff", "--name-only", "--no-renames", "-z", from, to])
        .output()
        .map_err(|error| format!("cannot run git diff: {error}"))?;
    if !output.status.success() {
        return Err(format!("git diff {from} {to} failed"));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter_map(|path| std::str::from_utf8(path).ok())
        .filter(|path| !path.is_empty() && is_input(platform, path))
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests;
