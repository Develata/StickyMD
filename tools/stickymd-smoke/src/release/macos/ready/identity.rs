//! The source identity the runner and the gate both require (plan 11 #macos-readiness-check):
//! HEAD as committed (no replacement ref or graft can substitute another tree), a clean status
//! against that tree, and no index flag that hides a change from the status. Read-only: the
//! status takes no optional index lock.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::release::macos) struct Identity {
    pub head: String,
    pub clean: bool,
    /// Why the checkout is not clean, when it is not.
    pub reason: Option<String>,
}

fn git(root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = crate::release::platform_inputs::git(root)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(format!("git {arguments:?} failed"));
    }
    String::from_utf8(output.stdout).map_err(|_| "git printed non-UTF-8".to_owned())
}

/// Whether a `git ls-files -v` listing marks an entry assume-unchanged (a lowercase tag) or
/// skip-worktree (`S`): either can differ from the tree while the status stays clean.
pub(super) fn hides_changes(listing: &str) -> bool {
    listing.lines().any(|line| {
        line.chars()
            .next()
            .is_some_and(|tag| tag.is_ascii_lowercase() || tag == 'S')
    })
}

/// Environment entries that point Git at another repository, work tree, index or object store
/// than the checkout the tools read (review round 3, B1).
const REDIRECTIONS: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
];

/// The first redirection `set` reports as present in the environment.
fn redirection(set: impl Fn(&str) -> bool) -> Option<&'static str> {
    REDIRECTIONS.into_iter().find(|name| set(name))
}

pub(super) fn capture(root: &Path) -> Result<Identity, String> {
    if let Some(set) = redirection(|name| std::env::var_os(name).is_some()) {
        return Err(format!(
            "the environment sets {set}, which points Git elsewhere than this checkout; unset it"
        ));
    }
    let top = git(root, &["rev-parse", "--show-toplevel"])?;
    let same = |a: &Path, b: &Path| match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same(Path::new(top.trim()), root) {
        return Err(format!(
            "Git's work tree is {}, not {}",
            top.trim(),
            root.display()
        ));
    }
    let head = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?
        .trim()
        .to_ascii_lowercase();
    let status = git(
        root,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=normal",
        ],
    )?;
    let flags = git(root, &["--no-optional-locks", "ls-files", "-v"])?;
    let reason = if !status.trim().is_empty() {
        Some("the work tree or index differs from HEAD".to_owned())
    } else if hides_changes(&flags) {
        Some("the index marks files assume-unchanged or skip-worktree".to_owned())
    } else {
        None
    };
    Ok(Identity {
        head,
        clean: reason.is_none(),
        reason,
    })
}

/// A clean checkout of `release`, or why not.
pub(in crate::release::macos) fn require(
    root: &Path,
    release: &str,
    what: &str,
) -> Result<Identity, String> {
    let identity = capture(root)?;
    if identity.head != release || !identity.clean {
        return Err(format!(
            "{what} runs on a clean checkout of {release}; HEAD is {}{}",
            identity.head,
            identity
                .reason
                .as_ref()
                .map_or_else(String::new, |reason| format!(" and {reason}"))
        ));
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    #[test]
    fn assume_unchanged_and_skip_worktree_entries_hide_changes() {
        assert!(!super::hides_changes("H Cargo.toml\nH src/lib.rs\n"));
        assert!(super::hides_changes("H Cargo.toml\nh src/lib.rs\n"));
        assert!(super::hides_changes("S src/lib.rs\n"));
    }

    /// Review round 3, B1: each variable that points Git at another repository, work tree, index
    /// or object store is refused on its own; others are not.
    #[test]
    fn each_git_redirection_is_refused_alone() {
        // Named here, not read from the table under test, so a name left out of it is caught
        // (review round 4).
        let names = [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_COMMON_DIR",
        ];
        assert_eq!(super::REDIRECTIONS, names);
        for name in names {
            assert_eq!(super::redirection(|set| set == name), Some(name));
        }
        assert_eq!(
            super::redirection(|set| set == "GIT_NO_REPLACE_OBJECTS"),
            None
        );
    }
}
