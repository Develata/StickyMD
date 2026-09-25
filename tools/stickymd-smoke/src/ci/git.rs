//! Observable Git inputs for CI; ambiguity always requests the legacy full plan.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::{path::Path, process::Command};

pub(super) struct Facts {
    pub(super) head: String,
    pub(super) dirty: bool,
    pub(super) paths: Result<Vec<String>, String>,
}

pub(super) fn inspect(root: &Path, base: Option<&str>) -> Result<Facts, String> {
    let head = crate::repository::command_text(root, "git", &["rev-parse", "HEAD"])?;
    if !valid_sha(&head) {
        return Err("current HEAD is not a full commit SHA".to_owned());
    }
    let status = output(
        root,
        &["status", "--porcelain", "-z", "--untracked-files=normal"],
    );
    let dirty = status.as_ref().is_none_or(|bytes| !bytes.is_empty());
    let paths = match (base, dirty) {
        (None, _) => Err("explicit full CI request".to_owned()),
        (_, true) => {
            Err("dirty or unreadable worktree; commit-only selection is unsafe".to_owned())
        }
        (Some(base), false) => changed_paths(root, base, &head),
    };
    Ok(Facts { head, dirty, paths })
}

fn valid_sha(value: &str) -> bool {
    value.len() == 40
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        && value.bytes().any(|byte| byte != b'0')
}

fn output(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn changed_paths(root: &Path, base: &str, head: &str) -> Result<Vec<String>, String> {
    if !valid_sha(base) || !valid_sha(head) {
        return Err("missing, zero or invalid comparison SHA".to_owned());
    }
    for sha in [base, head] {
        if output(root, &["cat-file", "-e", &format!("{sha}^{{commit}}")]).is_none() {
            return Err("comparison commit is not available; full CI required".to_owned());
        }
    }
    // No rename guessing: a rename becomes deletion + addition, retaining both owners.
    let bytes = output(
        root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            base,
            head,
            "--",
        ],
    )
    .ok_or("Git comparison failed; full CI required")?;
    decode_paths(&bytes)
}

fn decode_paths(bytes: &[u8]) -> Result<Vec<String>, String> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err("unterminated Git path output".to_owned());
    }
    bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|bytes| {
            let path = std::str::from_utf8(bytes).map_err(|_| "non-UTF-8 Git path".to_owned())?;
            if path.starts_with('/')
                || path.contains('\\')
                || path
                    .split('/')
                    .any(|part| part == ".." || part == "." || part.is_empty())
            {
                return Err("noncanonical Git path".to_owned());
            }
            Ok(path.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests;
