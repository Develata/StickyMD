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

/// The pinned workflows are exactly the `.github/` files of the index that a platform's
/// fingerprint counts: dropping a pin, or a platform counting another workflow, is refused.
pub(super) fn check_pinned_set(
    index: &std::collections::BTreeMap<String, String>,
    counted_by_a_platform: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let counted: Vec<&str> = index
        .keys()
        .map(String::as_str)
        .filter(|path| path.starts_with(".github/") && counted_by_a_platform(path))
        .collect();
    let mut pinned: Vec<&str> = RELEASE_WORKFLOWS.iter().map(|(path, _)| *path).collect();
    pinned.sort_unstable();
    if counted != pinned {
        return Err(format!(
            "the fingerprint counts the workflows {counted:?}, but stickymd-smoke pins {pinned:?}"
        ));
    }
    Ok(())
}

const BUILD_PACKAGE: &str = "  build-package:";
const AFTER_BUILD_PACKAGE: &str = "  verify-artifact:";
/// Environment variables that change what Cargo and rustc compile, or with which toolchain.
const COMPILE_VARIABLES: &[&str] = &[
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_TARGET",
    "CARGO_BUILD_BUILD_DIR",
    "CARGO_TARGET_DIR",
    "RUSTUP_TOOLCHAIN",
    // `RUSTC` itself, as a YAML key, a shell or a PowerShell assignment (a bare `rustc` is the
    // compiler's name in commands).
    "RUSTC:",
    "RUSTC=",
    "env:RUSTC",
];

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
    // The premise that self-check and build-package compile alike rests on the workflow setting
    // none of the variables that change a compilation; Windows reads environment names in any
    // letter case, so the search is too.
    if let Some(variable) = COMPILE_VARIABLES.iter().find(|variable| {
        pin.as_bytes()
            .windows(variable.len())
            .any(|window| window.eq_ignore_ascii_case(variable.as_bytes()))
    }) {
        return Err(format!(
            "its pinned copy names `{variable}`, which could compile build-package unlike self-check"
        ));
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
