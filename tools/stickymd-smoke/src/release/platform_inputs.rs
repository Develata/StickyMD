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
///   refuses a `build` key, so no other file can be one);
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
    if listed(COUNTED_DESPITE_EXCLUSION) {
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
            "tools/stickymd-smoke/src/main.rs",
            "tools/stickymd-smoke/src/release/platform.rs",
            "tools/stickymd-smoke/tests/release_workflow.rs",
            "tools/stickymd-smoke/README.md",
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
            "tools/stickymd-package/src/main.rs",
            "tools/stickymd-package/src/macos/package_readme.txt",
            "tools/stickymd-package/Cargo.toml",
            "tools/stickymd-smoke/Cargo.toml",
            "tools/stickymd-smoke/build.rs",
            "tools/stickymd-smoke/src/release/platform_inputs.rs",
            "apps/stickymd-win/build.rs",
            "tools/smoke/all.ps1",
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
        // A build script Cargo would discover runs in the Windows workspace build.
        assert!(is_input(Win, "apps/stickymd-mac/build.rs"));
        assert!(is_input(Win, "crates/stickymd-posix/build.rs"));
        assert!(is_input(Mac, "apps/stickymd-win/build.rs"));
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
        assert!(verify_reuse(root, win, &first.to_ascii_uppercase(), first).is_err());
        let unknown = "0".repeat(40);
        assert!(verify_reuse(root, win, &unknown, &unknown).is_err());
        // An annotated tag's own object is not a commit, though it peels to one.
        git(root, &["tag", "-a", "-m", "t", "annotated", first]);
        let tag = git(root, &["rev-parse", "annotated"]);
        assert_ne!(&tag, first);
        let error = verify_reuse(root, win, &tag, &tag).unwrap_err();
        assert!(error.contains("not a commit"), "{error}");
        // The fingerprint command alone refuses it too, and a tree id.
        assert!(fingerprint(root, win, &tag).is_err());
        let tree = git(root, &["rev-parse", &format!("{first}^{{tree}}")]);
        assert!(fingerprint(root, win, &tree).is_err());
        // A graft cannot make a non-ancestor an ancestor: X, a document change beside the main
        // line, has the Windows inputs of `doc_change` but is not its ancestor until a graft
        // names X as `doc_change`'s parent. The graft file is ignored.
        git(root, &["checkout", "-q", "-b", "side", first]);
        std::fs::write(root.join("docs/plan/a.md"), "side").unwrap();
        git(root, &["commit", "-q", "-am", "side"]);
        let side = git(root, &["rev-parse", "HEAD"]);
        assert!(verify_reuse(root, win, &side, doc_change).is_err());
        std::fs::write(
            root.join(".git/info/grafts"),
            format!("{doc_change} {side}\n"),
        )
        .unwrap();
        let plain = Command::new("git")
            .current_dir(root)
            .args(["merge-base", "--is-ancestor", &side, doc_change])
            .status()
            .unwrap();
        assert!(plain.success(), "plain Git follows the graft");
        let error = verify_reuse(root, win, &side, doc_change).unwrap_err();
        assert!(error.contains("not an ancestor"), "{error}");
        // Each host's manifest reaches the other platform: features merge, and the macOS bundle
        // records a fingerprint that reads every manifest.
        let error = verify_reuse(root, win, win_change, mac_manifest).unwrap_err();
        assert!(error.contains("apps/stickymd-mac/Cargo.toml"), "{error}");
        let error = verify_reuse(root, mac, mac_manifest, win_manifest).unwrap_err();
        assert!(error.contains("apps/stickymd-win/Cargo.toml"), "{error}");
    }

    /// The crate boundary of plan 11: a change to the verification tool alone leaves both
    /// platforms' fingerprints, and so their artifacts, as they were; a change to the packaging
    /// crate, to the verification tool's manifest (it joins Cargo's feature resolution) or build
    /// script (Cargo runs it), or to this classifier (the reuse rule itself) changes both.
    #[test]
    fn only_the_packaging_crate_and_the_counted_tool_files_reach_the_fingerprints() {
        let directory = TemporaryDirectory::new("platform-inputs-tools").unwrap();
        let root = directory.path();
        git(root, &["init", "-q", "-b", "main"]);
        let commit = |files: &[(&str, &str)]| {
            for (path, text) in files {
                let path = root.join(path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text).unwrap();
            }
            git(root, &["add", "-A"]);
            git(root, &["commit", "-q", "-m", "c"]);
            git(root, &["rev-parse", "HEAD"])
        };
        let base = commit(&[
            ("Cargo.toml", "[workspace]\n"),
            ("tools/stickymd-package/Cargo.toml", "[package]\n"),
            ("tools/stickymd-package/src/lib.rs", "package 1"),
            ("tools/stickymd-smoke/Cargo.toml", "[package]\n"),
            ("tools/stickymd-smoke/src/main.rs", "smoke 1"),
            ("tools/stickymd-smoke/tests/release_workflow.rs", "test 1"),
        ]);
        let verification = commit(&[
            ("tools/stickymd-smoke/src/main.rs", "smoke 2"),
            ("tools/stickymd-smoke/src/release/new_gate.rs", "a new gate"),
            ("tools/stickymd-smoke/tests/release_workflow.rs", "test 2"),
        ]);
        let packaging = commit(&[("tools/stickymd-package/src/lib.rs", "package 2")]);
        let after_packaging = commit(&[("tools/stickymd-smoke/src/main.rs", "smoke 3")]);
        let manifest = commit(&[(
            "tools/stickymd-smoke/Cargo.toml",
            "[package]\n[dependencies]\n",
        )]);
        // Cargo runs a build script it finds without a manifest entry; the classifier is the rule.
        let build_script = commit(&[("tools/stickymd-smoke/build.rs", "fn main() {}")]);
        let classifier = commit(&[(
            "tools/stickymd-smoke/src/release/platform_inputs.rs",
            "a changed table",
        )]);
        for platform in [Platform::WindowsX64, Platform::MacOsArm64] {
            let value = |commit: &str| fingerprint(root, platform, commit).unwrap();
            assert_eq!(value(&base), value(&verification), "{}", platform.label());
            assert_eq!(
                verify_reuse(root, platform, &base, &verification),
                Ok(Reuse::Ancestor {
                    fingerprint: value(&base)
                })
            );
            assert_ne!(
                value(&verification),
                value(&packaging),
                "{}",
                platform.label()
            );
            let error = verify_reuse(root, platform, &verification, &packaging).unwrap_err();
            assert!(
                error.contains("tools/stickymd-package/src/lib.rs"),
                "{error}"
            );
            assert!(verify_reuse(root, platform, &packaging, &after_packaging).is_ok());
            let error = verify_reuse(root, platform, &after_packaging, &manifest).unwrap_err();
            assert!(error.contains("tools/stickymd-smoke/Cargo.toml"), "{error}");
            let error = verify_reuse(root, platform, &manifest, &build_script).unwrap_err();
            assert!(error.contains("tools/stickymd-smoke/build.rs"), "{error}");
            let error = verify_reuse(root, platform, &build_script, &classifier).unwrap_err();
            assert!(error.contains("src/release/platform_inputs.rs"), "{error}");
        }
        directory.close().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_among_the_inputs_fails_the_fingerprint() {
        let repository = repository();
        let root = repository.directory.path();
        std::os::unix::fs::symlink("plan/a.md", root.join("docs/link.md")).unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "excluded link"]);
        let excluded = git(root, &["rev-parse", "HEAD"]);
        fingerprint(root, Platform::WindowsX64, &excluded).unwrap();
        std::fs::create_dir_all(root.join("crates")).unwrap();
        std::os::unix::fs::symlink("../docs/plan/a.md", root.join("crates/input.md")).unwrap();
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", "input link"]);
        let linked = git(root, &["rev-parse", "HEAD"]);
        let error = fingerprint(root, Platform::WindowsX64, &linked).unwrap_err();
        assert!(error.contains("crates/input.md"), "{error}");
    }
}
