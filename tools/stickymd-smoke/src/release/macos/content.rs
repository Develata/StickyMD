//! What the macOS ZIP holds, exactly, and the README it carries. Pure, so the promotion workflow
//! on a Windows runner checks the same list the macOS packager writes.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use std::collections::BTreeSet;

/// Where a file beside the app comes from.
pub(crate) enum Source {
    Readme,
    Notices,
    /// A repository file, line endings normalized to LF.
    License(&'static str),
}

/// The files beside the app.
pub(crate) const FILES: &[(&str, Source)] = &[
    ("StickyMD/README.txt", Source::Readme),
    ("StickyMD/LICENSE.txt", Source::License("LICENSE")),
    ("StickyMD/THIRD_PARTY_NOTICES.txt", Source::Notices),
    (
        "StickyMD/licenses/SIL-OFL-1.1.txt",
        Source::License("assets/licenses/SIL-OFL-1.1.txt"),
    ),
    (
        "StickyMD/licenses/KaTeX-fonts-NOTICE.txt",
        Source::License("assets/licenses/KaTeX-fonts-NOTICE.txt"),
    ),
];

pub(crate) const APP: &str = "StickyMD/StickyMD.app";
pub(crate) const README: &str = "StickyMD/README.txt";

/// The files of the ad-hoc signed bundle `build-app.sh` makes, and nothing else.
pub(crate) const APP_FILES: &[&str] = &[
    "StickyMD/StickyMD.app/Contents/Info.plist",
    "StickyMD/StickyMD.app/Contents/MacOS/StickyMD",
    "StickyMD/StickyMD.app/Contents/_CodeSignature/CodeResources",
];

/// Checks the regular files found in the archive (paths with `/`, directories left out) against
/// the inventory: nothing missing, nothing extra — no `note/`, no `__MACOSX/`, no `.DS_Store`.
pub(crate) fn check_files(found: &BTreeSet<String>) -> Result<(), String> {
    let expected = FILES
        .iter()
        .map(|(name, _)| *name)
        .chain(APP_FILES.iter().copied())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let missing = expected.difference(found).cloned().collect::<Vec<_>>();
    let extra = found.difference(&expected).cloned().collect::<Vec<_>>();
    if missing.is_empty() && extra.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the macOS package does not hold exactly its inventory; missing {missing:?}, extra {extra:?}"
        ))
    }
}

/// Every directory a listing may also name: the parents of the inventory's files.
pub(crate) fn is_inventory_directory(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    FILES
        .iter()
        .map(|(name, _)| *name)
        .chain(APP_FILES.iter().copied())
        .any(|file| {
            file.strip_prefix(path)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

pub(crate) fn readme(version: &str, commit: &str, exact_candidate: bool) -> String {
    let title = if exact_candidate {
        "StickyMD exact workflow candidate for macOS 27+ arm64"
    } else {
        "StickyMD portable release for macOS 27+ arm64"
    };
    format!(
        "{title}\nVersion: {version}\nSource commit: {commit}\n\n{}",
        include_str!("package_readme.txt")
    )
}

/// The one `Source commit:` line of a package README.
pub(crate) fn source_commit(readme: &str) -> Result<String, String> {
    let mut commits = readme
        .lines()
        .filter_map(|line| line.strip_prefix("Source commit: "));
    match (commits.next(), commits.next()) {
        (Some(commit), None)
            if commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
        {
            Ok(commit.to_ascii_lowercase())
        }
        _ => Err("the package README must name exactly one full source commit".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory() -> BTreeSet<String> {
        FILES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .chain(APP_FILES.iter().map(|name| (*name).to_owned()))
            .collect()
    }

    #[test]
    fn exactly_the_inventory_passes_and_anything_else_names_the_difference() {
        check_files(&inventory()).unwrap();
        for extra in [
            "StickyMD/note/note.md",
            "__MACOSX/StickyMD/._README.txt",
            "StickyMD/.DS_Store",
            "StickyMD/StickyMD.app/Contents/Resources/x",
        ] {
            let mut found = inventory();
            found.insert(extra.to_owned());
            let error = check_files(&found).unwrap_err();
            assert!(error.contains(extra), "{error}");
        }
        let mut found = inventory();
        found.remove("StickyMD/THIRD_PARTY_NOTICES.txt");
        assert!(
            check_files(&found)
                .unwrap_err()
                .contains("THIRD_PARTY_NOTICES")
        );
    }

    #[test]
    fn only_the_parents_of_inventory_files_are_directories_it_names() {
        for directory in [
            "StickyMD/",
            "StickyMD/licenses",
            "StickyMD/StickyMD.app/",
            "StickyMD/StickyMD.app/Contents/_CodeSignature/",
        ] {
            assert!(is_inventory_directory(directory), "{directory}");
        }
        for directory in [
            "StickyMD/note/",
            "__MACOSX/",
            "StickyMD/StickyMD.app/Contents/Res",
        ] {
            assert!(!is_inventory_directory(directory), "{directory}");
        }
    }

    #[test]
    fn the_readme_names_its_source_once_and_the_first_open_steps() {
        let commit = "a".repeat(40);
        let text = readme("0.3.0", &commit, true);
        assert!(text.starts_with(&format!(
            "StickyMD exact workflow candidate for macOS 27+ arm64\nVersion: 0.3.0\nSource commit: {commit}\n"
        )));
        assert_eq!(source_commit(&text).unwrap(), commit);
        assert!(!text.contains('\r'));
        for phrase in [
            "drag StickyMD.app into Applications",
            "~/Library/Application Support/StickyMD/StickyMD/note/",
            "move the whole StickyMD folder",
            "ad-hoc signed",
            "Open Anyway",
            "SHA-256",
            "当前目录不可写",
        ] {
            assert!(text.contains(phrase), "{phrase}");
        }
        assert!(source_commit(&format!("{text}Source commit: {commit}\n")).is_err());
        assert!(source_commit("Source commit: abc\n").is_err());
    }
}
