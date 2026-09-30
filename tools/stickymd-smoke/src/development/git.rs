//! Read-only local Git observation with NUL-delimited path transport.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::{collections::BTreeSet, path::Path, process::Command};

pub(super) struct Facts {
    pub(super) head: Option<String>,
    pub(super) paths: Result<Vec<String>, String>,
}

pub(super) fn inspect(root: &Path) -> Facts {
    let head = read_head(root);
    let paths = head.as_ref().map_err(Clone::clone).and_then(|before| {
        // One porcelain snapshot covers both index and worktree, even when a staged
        // edit has been restored to HEAD in the worktree. `all` expands new dirs.
        // Optional index-refresh locks are disabled so a plan never writes Git state.
        let bytes = output(
            root,
            &[
                "--no-optional-locks",
                "-c",
                "status.renames=false",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=none",
            ],
        )?;
        let paths = decode_status(&bytes)?;
        if &read_head(root)? != before {
            return Err("HEAD changed during local Git observation".to_owned());
        }
        Ok(paths)
    });
    Facts {
        head: head.ok(),
        paths,
    }
}

fn read_head(root: &Path) -> Result<String, String> {
    let bytes = output(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let head = std::str::from_utf8(&bytes)
        .map_err(|_| "non-UTF-8 Git HEAD")?
        .trim();
    if !crate::ci::git::valid_sha(head) {
        return Err("HEAD is not a full commit SHA".to_owned());
    }
    Ok(head.to_owned())
}

fn output(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start Git observation: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git observation failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn decode_status(bytes: &[u8]) -> Result<Vec<String>, String> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err("unterminated Git status output".to_owned());
    }
    let mut entries = bytes.split_inclusive(|byte| *byte == 0);
    let mut paths = BTreeSet::new();
    while let Some(entry) = entries.next() {
        if entry.len() < 5 || entry[2] != b' ' {
            return Err("malformed Git status entry".to_owned());
        }
        let status = &entry[..2];
        if status == b"??" {
            // New paths are included; ignored paths are absent from this query.
        } else if status.iter().any(|byte| !b" MADRCTU".contains(byte)) || status == b"  " {
            return Err("unknown Git status entry".to_owned());
        } else if status.contains(&b'U') || matches!(status, b"AA" | b"DD") {
            return Err("unmerged Git inputs require all local checks".to_owned());
        }
        paths.extend(crate::ci::git::decode_paths(&entry[3..])?);
        if status.contains(&b'R') || status.contains(&b'C') {
            // Support both names even if a Git implementation returns rename records
            // despite status.renames=false; porcelain -z prints the destination first.
            let old = entries.next().ok_or("Git rename lacks its original path")?;
            if old == [0] {
                return Err("Git rename has an empty original path".to_owned());
            }
            paths.extend(crate::ci::git::decode_paths(old)?);
        }
    }
    Ok(paths.into_iter().collect())
}

#[cfg(test)]
mod tests;
