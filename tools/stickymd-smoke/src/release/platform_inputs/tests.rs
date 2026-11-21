//! The exclusion table, the reuse rule and the crate boundary over throwaway repositories. Kept
//! out of the classifier file, which both platforms count: a change here changes no fingerprint.

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
    // Any spelling Cargo would find on a case-insensitive volume.
    for path in [
        "tools/stickymd-smoke/Build.rs",
        "tools/stickymd-smoke/CARGO.TOML",
        "apps/stickymd-mac/BUILD.RS",
    ] {
        assert!(is_input(Win, path) && is_input(Mac, path), "{path}");
    }
    // The classifier's tests are not the classifier.
    assert!(!is_input(
        Win,
        "tools/stickymd-smoke/src/release/platform_inputs/tests.rs"
    ));
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
