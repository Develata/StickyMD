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

/// Paths no platform's artifact, build, packaging or verification reads.
const COMMON_EXCLUDED_PREFIXES: &[&str] = &[
    // Documentation: the package README is generated from a template in the tool.
    "docs/",
    // Not workspace members.
    "experiments/",
    // Steps for people.
    "tools/manual/",
    // Pictures in the repository README.
    "assets/readme/",
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
/// Every workspace member's manifest is an input of both platforms: another host's manifest can
/// change the feature set Cargo merges without changing `Cargo.lock`, and the macOS product input
/// fingerprint written into the bundle reads every member's manifest.
const MEMBER_MANIFESTS: &[&str] = &[
    "apps/stickymd-mac/Cargo.toml",
    "apps/stickymd-win/Cargo.toml",
    "crates/stickymd-posix/Cargo.toml",
];

/// Whether `path` (repository-relative, `/`-separated) is an input of `platform`'s artifact.
/// Anything not listed as excluded is an input, a new path included.
pub(crate) fn is_input(platform: Platform, path: &str) -> bool {
    if let Some(rest) = path.strip_prefix(".github/") {
        // Only the platform's own release workflow builds; CI, schedules and promotion do not.
        return rest == own_workflow(platform);
    }
    if MEMBER_MANIFESTS.contains(&path) {
        return true;
    }
    if COMMON_EXCLUDED_FILES.contains(&path)
        || COMMON_EXCLUDED_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
    {
        return false;
    }
    let excluded = match platform {
        Platform::WindowsX64 => WINDOWS_EXCLUDED_PREFIXES,
        Platform::MacOsArm64 => MACOS_EXCLUDED_PREFIXES,
    };
    !excluded.iter().any(|prefix| path.starts_with(prefix))
}

fn own_workflow(platform: Platform) -> &'static str {
    match platform {
        Platform::WindowsX64 => "workflows/release.yml",
        Platform::MacOsArm64 => "workflows/release-macos.yml",
    }
}

struct Entry {
    mode: String,
    object: String,
    path: Vec<u8>,
}

/// The tree of `commit`, every entry with its mode and blob id, sorted by path bytes.
fn tree(root: &Path, commit: &str) -> Result<Vec<Entry>, String> {
    let output = Command::new("git")
        .current_dir(root)
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
        if kind != "blob" {
            // A submodule's content is not in this repository's tree.
            return Err(format!(
                "unsupported {kind} entry in {commit}: {}",
                String::from_utf8_lossy(&record[tab + 1..])
            ));
        }
        entries.push(Entry {
            mode: mode.to_owned(),
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
    let mut inputs = Vec::new();
    for entry in tree(root, commit)? {
        let path = std::str::from_utf8(&entry.path)
            .map_err(|_| format!("a path in {commit} is not UTF-8"))?;
        if is_input(platform, path) {
            inputs.push(entry);
        }
    }
    let scratch = TemporaryDirectory::new("platform-inputs")?;
    let stream_path = scratch.path().join("stream");
    let mut stream = BufWriter::new(
        File::create(&stream_path).map_err(|error| format!("cannot create the stream: {error}"))?,
    );
    let io = |error: std::io::Error| format!("cannot write the input stream: {error}");
    stream
        .write_all(b"StickyMD platform build inputs v1\0")
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
    let mut child = Command::new("git")
        .current_dir(root)
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
    let written = writer
        .join()
        .map_err(|_| "the git cat-file writer panicked")?;
    drop(output);
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
    for (label, commit) in [("build source", build), ("release commit", release)] {
        if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("{label} must be a full commit SHA: {commit}"));
        }
    }
    let exists = |commit: &str| {
        Command::new("git")
            .current_dir(root)
            .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
            .status()
            .is_ok_and(|status| status.success())
    };
    for commit in [build, release] {
        if !exists(commit) {
            return Err(format!("{commit} is not a commit in this repository"));
        }
    }
    if build.eq_ignore_ascii_case(release) {
        return Ok(Reuse::SameCommit);
    }
    let ancestor = Command::new("git")
        .current_dir(root)
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
    let output = Command::new("git")
        .current_dir(root)
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
mod tests {
    use super::*;

    #[test]
    fn the_exclusion_table_keeps_everything_else_as_an_input() {
        use Platform::{MacOsArm64 as Mac, WindowsX64 as Win};
        for path in [
            "docs/plan/11_testing_and_release.md",
            "README.md",
            "experiments/phase-01/x/src/main.rs",
            "tools/manual/macos-r03-translocation.md",
            "assets/readme/screenshot.png",
            ".github/workflows/ci.yml",
            ".github/workflows/promote-release.yml",
            ".github/actions/rust-cache/action.yml",
        ] {
            assert!(!is_input(Win, path) && !is_input(Mac, path), "{path}");
        }
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "deny.toml",
            "LICENSE",
            "THIRD_PARTY_NOTICES.md",
            "assets/licenses/SIL-OFL-1.1.txt",
            "crates/stickymd-app/src/lib.rs",
            "tools/stickymd-smoke/src/main.rs",
            "a-new-top-level-file",
            ".gitattributes",
            "tests/fixtures/a.md",
            "apps/stickymd-mac/Cargo.toml",
            "apps/stickymd-win/Cargo.toml",
            "crates/stickymd-posix/Cargo.toml",
        ] {
            assert!(is_input(Win, path) && is_input(Mac, path), "{path}");
        }
        assert!(!is_input(Win, "apps/stickymd-mac/src/main.rs"));
        assert!(!is_input(Win, "crates/stickymd-posix/src/lib.rs"));
        assert!(!is_input(Win, ".github/workflows/release-macos.yml"));
        assert!(is_input(Win, ".github/workflows/release.yml"));
        assert!(is_input(Win, "apps/stickymd-win/src/main.rs"));
        assert!(is_input(Win, "tools/release/package.ps1"));
        assert!(!is_input(Mac, "apps/stickymd-win/src/main.rs"));
        assert!(!is_input(Mac, "tools/release/package.ps1"));
        assert!(!is_input(Mac, ".github/workflows/release.yml"));
        assert!(is_input(Mac, ".github/workflows/release-macos.yml"));
        assert!(is_input(Mac, "apps/stickymd-mac/packaging/build-app.sh"));
        assert!(is_input(Mac, "crates/stickymd-posix/src/lib.rs"));
    }

    /// A throwaway repository with three commits: one changing only the macOS host, one only a
    /// document, one the Windows host.
    struct Repository {
        directory: TemporaryDirectory,
        commits: Vec<String>,
    }

    fn git(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {arguments:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn repository() -> Repository {
        let directory = TemporaryDirectory::new("platform-inputs-repo").unwrap();
        let root = directory.path();
        git(root, &["init", "-q", "-b", "main"]);
        let mut commits = Vec::new();
        let mut commit = |files: &[(&str, &str)]| {
            for (path, text) in files {
                let path = root.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
            git(root, &["add", "-A"]);
            git(root, &["commit", "-q", "-m", "c"]);
            commits.push(git(root, &["rev-parse", "HEAD"]));
        };
        commit(&[
            ("Cargo.toml", "[workspace]\n"),
            ("apps/stickymd-win/src/main.rs", "win 1"),
            ("apps/stickymd-mac/src/main.rs", "mac 1"),
            ("docs/plan/a.md", "doc 1"),
        ]);
        commit(&[("apps/stickymd-mac/src/main.rs", "mac 2")]);
        commit(&[("docs/plan/a.md", "doc 2")]);
        commit(&[("apps/stickymd-win/src/main.rs", "win 2")]);
        commit(&[(
            "apps/stickymd-mac/Cargo.toml",
            "[features]\ndefault = [\"x\"]\n",
        )]);
        commit(&[("apps/stickymd-win/Cargo.toml", "version = \"0.3.0\"\n")]);
        Repository { directory, commits }
    }

    #[test]
    fn a_platform_reuses_an_ancestor_only_while_its_own_inputs_are_unchanged() {
        let repository = repository();
        let root = repository.directory.path();
        let [
            first,
            mac_change,
            doc_change,
            win_change,
            mac_manifest,
            win_manifest,
        ] = repository.commits.as_slice()
        else {
            panic!("six commits");
        };
        let win = Platform::WindowsX64;
        let mac = Platform::MacOsArm64;
        // A macOS change and a document leave Windows reusable.
        assert!(matches!(
            verify_reuse(root, win, first, doc_change),
            Ok(Reuse::Ancestor { .. })
        ));
        assert_eq!(
            verify_reuse(root, win, doc_change, doc_change),
            Ok(Reuse::SameCommit)
        );
        // The macOS change makes the macOS artifact of the first commit unusable.
        let error = verify_reuse(root, mac, first, doc_change).unwrap_err();
        assert!(error.contains("apps/stickymd-mac/src/main.rs"), "{error}");
        // After it, a document and a Windows change leave macOS reusable.
        assert!(matches!(
            verify_reuse(root, mac, mac_change, win_change),
            Ok(Reuse::Ancestor { .. })
        ));
        assert!(verify_reuse(root, win, first, win_change).is_err());
        // Never backwards: a descendant is not an ancestor.
        let backwards = verify_reuse(root, win, doc_change, first).unwrap_err();
        assert!(backwards.contains("not an ancestor"), "{backwards}");
        // The two platforms' fingerprints differ even over the same tree.
        assert_ne!(
            fingerprint(root, win, first).unwrap(),
            fingerprint(root, mac, first).unwrap()
        );
        assert!(verify_reuse(root, win, "short", first).is_err());
        let unknown = "0".repeat(40);
        assert!(verify_reuse(root, win, &unknown, &unknown).is_err());
        // Each host's manifest reaches the other platform: features merge, and the macOS bundle
        // records a fingerprint that reads every manifest.
        let error = verify_reuse(root, win, win_change, mac_manifest).unwrap_err();
        assert!(error.contains("apps/stickymd-mac/Cargo.toml"), "{error}");
        let error = verify_reuse(root, mac, mac_manifest, win_manifest).unwrap_err();
        assert!(error.contains("apps/stickymd-win/Cargo.toml"), "{error}");
    }
}
