//! The source identity the runner and the gate both require (plan 11 #macos-readiness-check):
//! HEAD as committed (no replacement ref or graft can substitute another tree), a clean status
//! against that tree, and no index flag that hides a change from the status. Read-only: the
//! status takes no optional index lock.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Identity {
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

pub(super) fn capture(root: &Path) -> Result<Identity, String> {
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
pub(super) fn require(root: &Path, release: &str, what: &str) -> Result<Identity, String> {
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
}
