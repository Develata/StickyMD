//! Portable member, path, size and native resource rules, independent of adapters.
//! plan_ref: docs/plan/11_testing_and_release.md#portable-windows-runtime

use super::json::Value;
use std::collections::BTreeSet;

const ALLOWED: &[&str] = &[
    "StickyMD/StickyMD.exe",
    "StickyMD/README.txt",
    "StickyMD/LICENSE.txt",
    "StickyMD/THIRD_PARTY_NOTICES.txt",
    "StickyMD/licenses/SIL-OFL-1.1.txt",
    "StickyMD/licenses/KaTeX-fonts-NOTICE.txt",
];
const MAX_ZIP_BYTES: u64 = 30 * 1024 * 1024;

pub(super) fn size(bytes: u64) -> Result<(), String> {
    if bytes > MAX_ZIP_BYTES {
        Err("Portable ZIP exceeds the 30 MiB hard gate".to_owned())
    } else {
        Ok(())
    }
}

pub(super) fn members(names: &[String]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err("Portable ZIP contains duplicate paths".to_owned());
        }
        if name.contains(['\\', ':'])
            || name.starts_with('/')
            || name.chars().any(char::is_control)
            || name.split('/').any(|part| {
                part.is_empty() || matches!(part, "." | "..") || part.ends_with(['.', ' '])
            })
        {
            return Err(format!("Unsafe ZIP path: {name}"));
        }
    }
    let unexpected = names
        .iter()
        .filter(|name| {
            !ALLOWED
                .iter()
                .any(|allowed| name.eq_ignore_ascii_case(allowed))
        })
        .map(String::as_str)
        .collect::<Vec<_>>();
    let missing = ALLOWED
        .iter()
        .filter(|name| !seen.contains(&name.to_ascii_lowercase()))
        .copied()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() || !missing.is_empty() {
        return Err(format!(
            "Portable allowlist mismatch; unexpected=[{}], missing=[{}]",
            unexpected.join(", "),
            missing.join(", ")
        ));
    }
    Ok(())
}

pub(super) fn resources(facts: &Value, version: &str) -> Result<(), String> {
    for (key, expected) in [
        ("ProductName", "StickyMD"),
        ("FileDescription", "StickyMD portable Markdown scratchpad"),
        ("OriginalFilename", "StickyMD.exe"),
        ("LegalCopyright", "Copyright (c) 2026 Develata"),
    ] {
        if !facts.field(key)?.string()?.eq_ignore_ascii_case(expected) {
            return Err("StickyMD.exe has an incomplete or incorrect version resource".to_owned());
        }
    }
    let numeric = version
        .split(['-', '+'])
        .next()
        .ok_or("missing workspace version")?;
    let parts = numeric
        .split('.')
        .map(|part| {
            part.parse::<u64>()
                .map_err(|_| "Cannot read the workspace semantic version".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 3 {
        return Err("Cannot read the workspace semantic version".to_owned());
    }
    for (key, expected) in ["Major", "Minor", "Build"].into_iter().zip(parts) {
        for prefix in ["File", "Product"] {
            if facts.field(&format!("{prefix}{key}Part"))?.unsigned()? != expected {
                return Err(format!(
                    "StickyMD.exe version does not match workspace version {numeric}"
                ));
            }
        }
    }
    if facts.field("IconCount")?.unsigned()? == 0 {
        return Err("StickyMD.exe lacks an application icon resource".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_resource_facts_reject_wrong_version_missing_fields_and_missing_icon() {
        let valid = r#"{"ProductName":"StickyMD","FileDescription":"StickyMD portable Markdown scratchpad","OriginalFilename":"StickyMD.exe","LegalCopyright":"Copyright (c) 2026 Develata","FileMajorPart":0,"FileMinorPart":1,"FileBuildPart":0,"ProductMajorPart":0,"ProductMinorPart":1,"ProductBuildPart":0,"IconCount":1}"#;
        resources(&super::super::json::parse(valid).unwrap(), "0.1.0").unwrap();
        for invalid in [
            valid.replace("\"IconCount\":1", "\"IconCount\":0"),
            valid.replace("\"FileMinorPart\":1", "\"FileMinorPart\":2"),
            valid.replace("\"ProductName\":\"StickyMD\"", "\"Other\":\"StickyMD\""),
        ] {
            assert!(resources(&super::super::json::parse(&invalid).unwrap(), "0.1.0").is_err());
        }
    }
    #[test]
    fn portable_members_reject_user_data_ambiguous_paths_and_wrong_inventory() {
        // Independent projection of plan 11's six release members, not the implementation list.
        let valid = [
            "StickyMD/StickyMD.exe",
            "StickyMD/README.txt",
            "StickyMD/LICENSE.txt",
            "StickyMD/THIRD_PARTY_NOTICES.txt",
            "StickyMD/licenses/SIL-OFL-1.1.txt",
            "StickyMD/licenses/KaTeX-fonts-NOTICE.txt",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        members(&valid).unwrap();
        for bad in [
            "StickyMD/note/note.md",
            "StickyMD/../evil",
            "/StickyMD/README.txt",
            "C:/evil",
            "StickyMD\\README.txt",
            "StickyMD//README.txt",
            "StickyMD/./README.txt",
            "StickyMD/README.txt.",
            "StickyMD/README.txt ",
        ] {
            let mut names = valid.clone();
            names.push(bad.to_owned());
            assert!(members(&names).is_err(), "{bad}");
        }
        let mut duplicate = valid.clone();
        duplicate.push(valid[0].to_uppercase());
        assert!(members(&duplicate).unwrap_err().contains("duplicate"));
        assert!(members(&valid[1..]).is_err());
        size(30 * 1024 * 1024).unwrap();
        assert!(size(30 * 1024 * 1024 + 1).is_err());
    }
}
