//! What the macOS ZIP holds, exactly, and the README it carries. Pure, so the promotion workflow
//! on a Windows runner checks the same list the macOS packager writes.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use crate::integrity;
use crate::release::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

/// The packaged files an SBOM must describe: every regular file of the package.
pub(crate) fn sbom_members() -> Vec<&'static str> {
    FILES
        .iter()
        .map(|(name, _)| *name)
        .chain(APP_FILES.iter().copied())
        .collect()
}

/// The SHA-256 of each SBOM member under `extracted` (an extracted package root).
pub(crate) fn member_hashes(extracted: &Path) -> Result<BTreeMap<&'static str, String>, String> {
    sbom_members()
        .into_iter()
        .map(|member| Ok((member, integrity::sha256(&extracted.join(member))?)))
        .collect()
}

/// The SPDX version Syft 1.50.0 writes (observed 2026-11-19); another version is another
/// generator and is reviewed before it is accepted.
const SPDX_VERSION: &str = "SPDX-2.3";

/// The SPDX document must be the expected SPDX version, list packages, and describe every
/// packaged file exactly once, as SPDX 2.3 requires of a file (an `SPDXRef-` identifier and a
/// SHA-1), with the SHA-256 of the bytes actually packaged (`hashes`). Syft 1.50.0 on macOS
/// names a directory source's files relative to it (`package/...`, observed 2026-11-19); a
/// leading `/` is accepted as the same name.
pub(crate) fn validate_sbom(
    text: &str,
    hashes: &BTreeMap<&'static str, String>,
) -> Result<(), String> {
    let document = json::parse(text)?;
    let version = document.field("spdxVersion")?.string()?;
    if version != SPDX_VERSION {
        return Err(format!(
            "Generated document is {version}, expected an {SPDX_VERSION} JSON SBOM"
        ));
    }
    if document.field("packages")?.array()?.is_empty() {
        return Err("Generated SBOM contains no packages".to_owned());
    }
    let files = document.field("files")?.array()?;
    for member in sbom_members() {
        let required = format!("package/{member}");
        let entries = files
            .iter()
            .filter(|file| {
                file.field("fileName")
                    .and_then(|name| name.string())
                    .is_ok_and(|name| name.strip_prefix('/').unwrap_or(name) == required)
            })
            .collect::<Vec<_>>();
        let [entry] = entries.as_slice() else {
            return Err(format!(
                "Generated SBOM describes packaged file {required} {} times, expected once",
                entries.len()
            ));
        };
        if !entry
            .field("SPDXID")
            .and_then(|id| id.string())
            .is_ok_and(|id| id.len() > "SPDXRef-".len() && id.starts_with("SPDXRef-"))
        {
            return Err(format!("{required} has no SPDX file identifier"));
        }
        let checksums = entry.field("checksums")?.array()?;
        let values = |algorithm: &str| {
            checksums
                .iter()
                .filter(|checksum| {
                    checksum
                        .field("algorithm")
                        .and_then(|value| value.string())
                        .is_ok_and(|value| value == algorithm)
                })
                .map(|checksum| checksum.field("checksumValue")?.string())
                .collect::<Result<Vec<_>, _>>()
        };
        match values("SHA1")?.as_slice() {
            [sha1] if sha1.len() == 40 && sha1.bytes().all(|byte| byte.is_ascii_hexdigit()) => {}
            other => {
                return Err(format!(
                    "{required} must carry one SHA-1 as SPDX requires, found {other:?}"
                ));
            }
        }
        let expected = hashes
            .get(member)
            .ok_or_else(|| format!("no packaged hash for {member}"))?;
        let described = values("SHA256")?;
        if described.as_slice() != [expected.as_str()] {
            return Err(format!(
                "the SBOM describes {required} with SHA-256 {described:?}, the package holds {expected}"
            ));
        }
    }
    Ok(())
}

/// The signing kind `codesign -dv` reports, checked separately from the signature's validity:
/// ad-hoc means `Signature=adhoc`, no Team ID and no certificate authority (plan 11 macOS gate).
pub(crate) fn ad_hoc_signature(display: &str) -> Result<&'static str, String> {
    let lines = display.lines().map(str::trim).collect::<Vec<_>>();
    let adhoc = lines.contains(&"Signature=adhoc");
    let no_team = lines.contains(&"TeamIdentifier=not set");
    let authority = lines.iter().find(|line| line.starts_with("Authority="));
    match (adhoc, no_team, authority) {
        (true, true, None) => Ok("AD_HOC"),
        _ => Err(format!(
            "the bundle is not ad-hoc signed (Signature=adhoc {adhoc}, no Team ID {no_team}, authority {authority:?})"
        )),
    }
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
    fn only_an_ad_hoc_signature_passes_the_signing_check() {
        let ad_hoc = "Format=app bundle with Mach-O thin (arm64)\nCodeDirectory v=20400 size=49045 flags=0x2(adhoc) hashes=1526+3 location=embedded\nSignature=adhoc\nTeamIdentifier=not set\n";
        assert_eq!(ad_hoc_signature(ad_hoc), Ok("AD_HOC"));
        // A valid Developer ID signature is a different kind, and so is an unsigned bundle.
        let developer_id = "Signature size=9054\nAuthority=Developer ID Application: Someone (ABCDE12345)\nAuthority=Developer ID Certification Authority\nAuthority=Apple Root CA\nTeamIdentifier=ABCDE12345\n";
        assert!(ad_hoc_signature(developer_id).is_err());
        assert!(ad_hoc_signature("code object is not signed at all\n").is_err());
        let mixed = format!("{ad_hoc}Authority=Apple Root CA\n");
        assert!(ad_hoc_signature(&mixed).is_err());
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

    fn document(files: &[(&str, &str)]) -> String {
        let files = files
            .iter()
            .map(|(name, hash)| {
                format!(
                    "{{\"SPDXID\":\"SPDXRef-File-{}\",\"fileName\":\"{name}\",\"checksums\":[{{\"algorithm\":\"SHA1\",\"checksumValue\":\"{}\"}},{{\"algorithm\":\"SHA256\",\"checksumValue\":\"{hash}\"}}]}}",
                    name.len(),
                    "0".repeat(40)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"spdxVersion\":\"SPDX-2.3\",\"packages\":[{{\"name\":\"x\"}}],\"files\":[{files}]}}"
        )
    }

    #[test]
    fn the_sbom_must_describe_each_member_with_the_packaged_bytes() {
        let hashes = sbom_members()
            .into_iter()
            .enumerate()
            .map(|(index, member)| (member, format!("{index:064x}")))
            .collect::<BTreeMap<_, _>>();
        let names = sbom_members()
            .into_iter()
            .enumerate()
            .map(|(index, member)| {
                // Syft's relative names, with one leading slash accepted as the same.
                let name = if index == 1 {
                    format!("/package/{member}")
                } else {
                    format!("package/{member}")
                };
                (name, hashes[member].clone())
            })
            .collect::<Vec<_>>();
        let all = names
            .iter()
            .map(|(name, hash)| (name.as_str(), hash.as_str()))
            .collect::<Vec<_>>();
        validate_sbom(&document(&all), &hashes).unwrap();
        for skip in 0..all.len() {
            let rest = all
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != skip)
                .map(|(_, entry)| *entry)
                .collect::<Vec<_>>();
            assert!(validate_sbom(&document(&rest), &hashes).is_err());
        }
        // The same path with other bytes: an SBOM of another build.
        let mut other = all.clone();
        other[0].1 = "f".repeat(64).leak();
        let error = validate_sbom(&document(&other), &hashes).unwrap_err();
        assert!(error.contains("SHA-256"), "{error}");
        assert!(validate_sbom(&document(&all).replace("SPDX-2.3", "SPDX-3.0"), &hashes).is_err());
        // A member described twice, without an identifier, or without a full SHA-1.
        let mut twice = all.clone();
        twice.push(all[0]);
        assert!(validate_sbom(&document(&twice), &hashes).is_err());
        let text = document(&all);
        let first_id = text.find("\"SPDXID\"").unwrap();
        let without_id = format!("{}\"X\"{}", &text[..first_id], &text[first_id + 8..]);
        assert!(validate_sbom(&without_id, &hashes).is_err());
        let short_sha1 = text.replacen(&"0".repeat(40), "00", 1);
        assert!(validate_sbom(&short_sha1, &hashes).is_err());
        // LICENSE.txt and README.txt are members like the rest.
        let members = sbom_members();
        assert!(
            members.contains(&"StickyMD/LICENSE.txt") && members.contains(&"StickyMD/README.txt")
        );
    }
}
