//! The Windows version recorded in desktop and manual evidence: a concrete version with
//! its build number, never a placeholder such as the `OS` variable (`Windows_NT`).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

pub(super) const UNKNOWN: &str = "UNKNOWN";

/// `Microsoft Windows <major.minor.build[.revision]>` from `ver`, or [`UNKNOWN`]. `ver`
/// prints in the console code page (for example `[版本 10.0.26200.9457]` in GBK), so its
/// bytes are decoded lossily and only the ASCII version token is kept.
pub(super) fn current() -> String {
    std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            version_token(&String::from_utf8_lossy(&output.stdout))
                .map(|version| format!("Microsoft Windows {version}"))
        })
        .unwrap_or_else(|| UNKNOWN.to_owned())
}

/// Whether a recorded value names a concrete Windows version with a build number.
pub(super) fn is_known(value: &str) -> bool {
    version_token(value).is_some()
}

/// The first dotted token with at least three numeric parts (`major.minor.build`).
fn version_token(text: &str) -> Option<&str> {
    text.split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .find(|token| {
            let parts = token.split('.').collect::<Vec<_>>();
            parts.len() >= 3
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
}

#[cfg(test)]
mod tests {
    use super::{is_known, version_token};

    #[test]
    fn only_a_concrete_version_with_a_build_is_known() {
        for (text, version) in [
            (
                "Microsoft Windows [Version 10.0.26200.9457]",
                "10.0.26200.9457",
            ),
            (
                "Microsoft Windows [\u{fffd}\u{fffd} 10.0.26200.9457]",
                "10.0.26200.9457",
            ),
            ("Microsoft Windows 10.0.22631", "10.0.22631"),
        ] {
            assert_eq!(version_token(text), Some(version), "{text}");
            assert!(is_known(text), "{text}");
        }
        for text in [
            "",
            "UNKNOWN",
            " UNKNOWN ",
            "Windows_NT",
            "Windows 11",
            "10.0",
            "10..26200",
            "Windows test",
        ] {
            assert!(!is_known(text), "{text}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn this_host_reports_a_known_build() {
        let current = super::current();
        assert!(is_known(&current), "{current}");
    }
}
