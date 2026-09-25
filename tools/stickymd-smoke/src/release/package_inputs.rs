//! Package naming and source-state checks; a plan does not validate an artifact.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::cli::PackageInputOptions;
use crate::{evidence::escape_json, integrity, package_path, repository};
use std::path::Path;

pub(super) fn execute(root: &Path, options: &PackageInputOptions) -> Result<(), String> {
    let version = options
        .version
        .clone()
        .map(Ok)
        .unwrap_or_else(|| repository::workspace_version(root))?;
    let commit = options
        .commit
        .clone()
        .map(Ok)
        .unwrap_or_else(|| super::identity::source(root))?;
    let dirty = !repository::command_text(root, "git", &["status", "--porcelain"])?.is_empty();
    let (archive, state) = plan(&version, &commit, dirty, options)?;
    println!(
        "{{\"schema_version\":1,\"status\":\"NOT_RUN\",\"version\":\"{}\",\"source_commit\":\"{}\",\"archive_name\":\"{}\",\"source_tree_state\":\"{state}\"}}",
        escape_json(&version),
        commit.to_ascii_lowercase(),
        escape_json(&archive)
    );
    Ok(())
}

fn plan(
    version: &str,
    commit: &str,
    dirty: bool,
    options: &PackageInputOptions,
) -> Result<(String, &'static str), String> {
    // Preserve package.ps1's accepted version/override interface.
    let (numeric, suffix) = version.find(['-', '+']).map_or((version, None), |index| {
        (&version[..index], Some(&version[index + 1..]))
    });
    let parts = numeric.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        || suffix.is_some_and(|suffix| {
            suffix.is_empty()
                || !suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
        })
    {
        return Err(format!("Invalid workspace version: {version}"));
    }
    integrity::validate_hex(commit, 40, "full commit SHA")?;
    if dirty && !options.allow_dirty {
        return Err("Refusing to label a dirty working tree as a local RC; commit first or use -AllowDirtyValidation for non-RC script validation".to_owned());
    }
    if options.tag.is_some() && options.exact {
        return Err("ReleaseTag and ExactCandidate are mutually exclusive".to_owned());
    }
    if options.exact {
        if dirty {
            return Err(
                "An exact workflow candidate cannot be built from a dirty working tree".to_owned(),
            );
        }
        Ok((
            format!("StickyMD-{version}-windows-x64-portable.zip"),
            "EXACT_WORKFLOW_CANDIDATE",
        ))
    } else if let Some(tag) = &options.tag {
        super::identity::release_tag(version, tag)?;
        if dirty {
            return Err(
                "A tagged release package cannot be built from a dirty working tree".to_owned(),
            );
        }
        Ok((
            format!("StickyMD-{tag}-windows-x64-portable.zip"),
            "TAGGED_RELEASE",
        ))
    } else {
        Ok((
            package_path::local_archive_name(version, commit, dirty)?,
            if dirty {
                "DIRTY_VALIDATION"
            } else {
                "CLEAN_PREFLIGHT"
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_planning_preserves_local_dirty_tag_and_exact_names_and_failure_boundaries() {
        let sha = "a".repeat(40);
        let mut options = PackageInputOptions::default();
        assert_eq!(
            plan("0.1.0", &sha, false, &options).unwrap().0,
            "StickyMD-0.1.0-local-rc-aaaaaaaaaaaa-windows-x64-portable.zip"
        );
        assert!(plan("0.1.0", &sha, true, &options).is_err());
        options.allow_dirty = true;
        assert_eq!(
            plan("0.1.0", &sha, true, &options).unwrap().1,
            "DIRTY_VALIDATION"
        );
        options.exact = true;
        assert!(plan("0.1.0", &sha, true, &options).is_err());
        assert_eq!(
            plan("0.1.0", &sha, false, &options).unwrap().0,
            "StickyMD-0.1.0-windows-x64-portable.zip"
        );
        options.tag = Some("v0.1.0".to_owned());
        assert!(plan("0.1.0", &sha, false, &options).is_err());
        options.exact = false;
        assert_eq!(
            plan("0.1.0", &sha, false, &options).unwrap().0,
            "StickyMD-v0.1.0-windows-x64-portable.zip"
        );
        assert!(plan("0.2.0", &sha, false, &options).is_err());
        for invalid in ["../bad", "1.2", "1.2.3-", "1.2.3 x"] {
            assert!(plan(invalid, &sha, false, &options).is_err());
        }
        assert!(plan("0.1.0", "abc", false, &options).is_err());
    }
}
