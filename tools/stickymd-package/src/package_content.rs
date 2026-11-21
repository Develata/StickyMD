//! One portable inventory for staging, archive validation and SBOM coverage.
//! plan_ref: docs/plan/11_testing_and_release.md#portable-windows-runtime

use super::package_inputs::Inputs;

pub(super) enum Content {
    Executable,
    Readme,
    Notices,
    License(&'static str),
}

pub(super) struct Member {
    pub name: &'static str,
    pub content: Content,
    pub sbom_required: bool,
}

// Keep the existing validation diagnostic order; ZIP entries use ordered_members().
pub(super) const MEMBERS: &[Member] = &[
    Member {
        name: "StickyMD/StickyMD.exe",
        content: Content::Executable,
        sbom_required: true,
    },
    Member {
        name: "StickyMD/README.txt",
        content: Content::Readme,
        sbom_required: false,
    },
    Member {
        name: "StickyMD/LICENSE.txt",
        content: Content::License("LICENSE"),
        sbom_required: false,
    },
    Member {
        name: "StickyMD/THIRD_PARTY_NOTICES.txt",
        content: Content::Notices,
        sbom_required: true,
    },
    Member {
        name: "StickyMD/licenses/SIL-OFL-1.1.txt",
        content: Content::License("assets/licenses/SIL-OFL-1.1.txt"),
        sbom_required: true,
    },
    Member {
        name: "StickyMD/licenses/KaTeX-fonts-NOTICE.txt",
        content: Content::License("assets/licenses/KaTeX-fonts-NOTICE.txt"),
        sbom_required: true,
    },
];

pub(super) fn ordered_members() -> Vec<&'static Member> {
    let mut members = MEMBERS.iter().collect::<Vec<_>>();
    members.sort_by_key(|member| member.name);
    members
}

pub(super) fn readme(inputs: &Inputs) -> Result<String, String> {
    let title = match inputs.state {
        "TAGGED_RELEASE" => "StickyMD portable release for Windows 11 x64",
        "EXACT_WORKFLOW_CANDIDATE" => "StickyMD exact workflow candidate for Windows 11 x64",
        "CLEAN_PREFLIGHT" | "DIRTY_VALIDATION" => {
            "StickyMD portable release candidate for Windows 11 x64"
        }
        _ => return Err("Unknown package source state".to_owned()),
    };
    Ok(format!(
        "{title}\r\nVersion: {}\r\nSource commit: {}\r\n\r\n{}\r\n",
        inputs.version,
        inputs.commit,
        include_str!("package_readme.txt")
            .lines()
            .collect::<Vec<_>>()
            .join("\r\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readme_preserves_titles_identity_crlf_and_portable_warnings() {
        for (state, title) in [
            (
                "TAGGED_RELEASE",
                "StickyMD portable release for Windows 11 x64",
            ),
            (
                "EXACT_WORKFLOW_CANDIDATE",
                "StickyMD exact workflow candidate for Windows 11 x64",
            ),
            (
                "CLEAN_PREFLIGHT",
                "StickyMD portable release candidate for Windows 11 x64",
            ),
            (
                "DIRTY_VALIDATION",
                "StickyMD portable release candidate for Windows 11 x64",
            ),
        ] {
            let inputs = Inputs {
                version: "0.1.1".into(),
                commit: "a".repeat(40),
                archive: String::new(),
                state,
            };
            let text = readme(&inputs).unwrap();
            assert!(text.starts_with(&format!(
                "{title}\r\nVersion: 0.1.1\r\nSource commit: {}\r\n",
                "a".repeat(40)
            )));
            assert!(text.ends_with("\r\n"));
            assert!(!text.replace("\r\n", "").contains(['\r', '\n']));
            for warning in [
                "This build is unsigned.",
                "SHA-256",
                ".\\note\\note.md",
                "Remote images are never downloaded",
                "License: MIT",
            ] {
                assert!(text.contains(warning));
            }
        }
    }

    #[test]
    fn inventory_is_in_stable_order_and_excludes_user_data() {
        let names = ordered_members()
            .iter()
            .map(|member| member.name)
            .collect::<Vec<_>>();
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(names.len(), 6);
        assert!(names.iter().all(|name| !name.contains("/note/")));
        assert_eq!(
            MEMBERS.iter().filter(|member| member.sbom_required).count(),
            4
        );
    }
}
