//! The release workflows' side of the packaging boundary: each release workflow equals its pinned
//! copy, whole, after line endings are made LF. A line alone cannot show where it sits in the
//! YAML, so no part is checked apart from the rest; the copy was reviewed to hold the
//! `dependency-policy` job and the three release jobs of plan 11, and changing the workflow means
//! changing the copy. The copy's `build-package` section, between two exact lines, is returned
//! for the name search, so that a pin update cannot carry the tool into the build job unnoticed.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

pub(super) const RELEASE_WORKFLOWS: &[(&str, &str)] = &[
    (
        ".github/workflows/release.yml",
        include_str!("pins/release.yml"),
    ),
    (
        ".github/workflows/release-macos.yml",
        include_str!("pins/release-macos.yml"),
    ),
];

const BUILD_PACKAGE: &str = "  build-package:";
const AFTER_BUILD_PACKAGE: &str = "  verify-artifact:";

/// Checks a release workflow against its pinned copy and returns the copy's `build-package`
/// section: the lines from the one line `  build-package:` up to the one line
/// `  verify-artifact:` after it.
pub(super) fn check(text: &str, pin: &str) -> Result<String, String> {
    let pin = pin.replace("\r\n", "\n");
    if text.replace("\r\n", "\n") != pin {
        return Err(
            "differs from its pinned copy in stickymd-smoke; review the change and update the copy"
                .to_owned(),
        );
    }
    let lines: Vec<&str> = pin.lines().collect();
    let only = |wanted: &str| -> Result<usize, String> {
        let found: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == wanted).collect();
        match found[..] {
            [at] => Ok(at),
            _ => Err(format!(
                "its pinned copy has {} lines `{wanted}`; exactly one is expected",
                found.len()
            )),
        }
    };
    let start = only(BUILD_PACKAGE)?;
    let end = only(AFTER_BUILD_PACKAGE)?;
    if end <= start {
        return Err(format!(
            "its pinned copy has `{AFTER_BUILD_PACKAGE}` before `{BUILD_PACKAGE}`"
        ));
    }
    Ok(lines[start..end]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect())
}
