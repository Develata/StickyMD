//! Pinned external Actions and same-checkout local workflow/action references.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::path::Path;

pub(super) fn verify_uses(root: &Path, source: &Path, content: &str) -> Result<(), String> {
    for (index, line) in content.lines().enumerate() {
        let line = line.trim();
        let line = line.strip_prefix("- ").unwrap_or(line);
        let Some(action) = line.strip_prefix("uses:") else {
            continue;
        };
        let action = action
            .split_once('#')
            .map_or(action, |(value, _)| value)
            .trim();
        verify_reference(root, action)
            .map_err(|error| format!("{}:{} {error}", source.display(), index + 1))?;
    }
    Ok(())
}

fn verify_reference(root: &Path, action: &str) -> Result<(), String> {
    if let Some(local) = action.strip_prefix("./") {
        if (!local.starts_with(".github/actions/") && !local.starts_with(".github/workflows/"))
            || local.contains(['\\', '@', ':'])
            || local.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(format!("invalid local Actions reference: {action}"));
        }
        let path = root.join(local);
        let target = if path.is_dir() {
            [path.join("action.yml"), path.join("action.yaml")]
                .into_iter()
                .find(|file| file.is_file())
                .ok_or_else(|| format!("local action definition is missing: {action}"))?
        } else {
            path
        };
        let canonical = target
            .canonicalize()
            .map_err(|error| format!("invalid local Actions target {action}: {error}"))?;
        let boundary = root
            .join(".github")
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if !canonical.starts_with(boundary)
            || !target.is_file()
            || !matches!(
                target.extension().and_then(|value| value.to_str()),
                Some("yml" | "yaml")
            )
        {
            return Err(format!(
                "local Actions target leaves the checked-out definitions: {action}"
            ));
        }
        return Ok(());
    }
    let (_, revision) = action
        .rsplit_once('@')
        .ok_or_else(|| format!("action is not pinned: {action}"))?;
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "action must use a full immutable commit SHA: {action}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_action_pins_cover_step_lists_and_job_level_uses() {
        for prefix in ["  uses:", "      - uses:"] {
            let root = Path::new(".");
            assert!(verify_uses(root, root, &format!("{prefix} actions/cache@v5")).is_err());
            assert!(
                verify_uses(
                    root,
                    root,
                    &format!("{prefix} actions/cache@{} # pinned", "a".repeat(40))
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn ci_local_actions_bind_existing_definitions_inside_the_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        for local in ["./.github/actions/rust-cache", "./.github/workflows/ci.yml"] {
            assert!(verify_reference(root, local).is_ok());
        }
        for invalid in [
            "./.github/actions/missing",
            "./.github/actions/../../Cargo.toml",
            "./.github/workflows/ci.yml@main",
            "./outside/action.yml",
            "./.github/actions/rust-cache/../rust-cache",
        ] {
            assert!(verify_reference(root, invalid).is_err());
        }
    }
}
