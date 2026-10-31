//! Which `(case, role)` cells a manual waiver may dispose of. Recording a waiver and
//! applying it at readiness use the same check, so a hand-edited decision receipt cannot
//! widen a waiver past what its key allows.

use super::registry::{self, Tier};
use super::target::Role;

pub(in crate::qualification) type Cells = Vec<(String, Role)>;

pub(in crate::qualification) fn is_waiver_key(key: &str) -> bool {
    key.starts_with("WAIVER-")
}

/// Parses `P12-M05:previous,P12-M35:current`.
pub(in crate::qualification) fn parse_cells(text: &str) -> Result<Cells, String> {
    text.split(',')
        .map(|item| {
            let (case, role) = item
                .split_once(':')
                .ok_or_else(|| format!("waiver cell `{item}` is not CASE:ROLE"))?;
            Ok((case.trim().to_owned(), Role::parse(role.trim())?))
        })
        .collect()
}

/// A waiver key and its cells agree: `WAIVER-P12-Mxx` covers only that case and
/// `WAIVER-TIER-B-v{version}` only Tier B cases. Cells are registered, unique and non-empty,
/// and the versioned key names `version`. The retired `WAIVER-PROFILE-PREVIOUS-v{version}`
/// is no longer a waiver key: optional cells never block, so they need none (plan 11).
pub(in crate::qualification) fn validate(
    key: &str,
    cells: &[(String, Role)],
    version: &str,
) -> Result<(), String> {
    if cells.is_empty() {
        return Err(format!("waiver {key} lists no case and role"));
    }
    for (index, (case, role)) in cells.iter().enumerate() {
        if registry::find(case).is_none() {
            return Err(format!("waiver {key} names unregistered case `{case}`"));
        }
        if cells[..index].contains(&(case.clone(), *role)) {
            return Err(format!("waiver {key} lists {case}:{} twice", role.as_str()));
        }
    }
    if let Some(case) = key
        .strip_prefix("WAIVER-")
        .filter(|rest| rest.starts_with("P12-M"))
    {
        if registry::find(case).is_none() {
            return Err(format!("waiver {key} names no registered manual case"));
        }
        return match cells.iter().find(|(cell, _)| cell != case) {
            Some((other, _)) => Err(format!("waiver {key} cannot cover {other}")),
            None => Ok(()),
        };
    }
    if let Some(rest) = key.strip_prefix("WAIVER-TIER-B-v") {
        expect_version(key, rest, version)?;
        return match cells
            .iter()
            .find(|(case, _)| registry::find(case).is_some_and(|case| case.tier != Tier::B))
        {
            Some((other, _)) => Err(format!("waiver {key} cannot cover non-Tier-B {other}")),
            None => Ok(()),
        };
    }
    Err(format!("`{key}` is not a manual waiver key"))
}

fn expect_version(key: &str, actual: &str, version: &str) -> Result<(), String> {
    if actual == version {
        Ok(())
    } else {
        Err(format!(
            "waiver {key} is for version {actual}, not {version}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_cells, validate};

    fn cells(text: &str) -> Vec<(String, super::Role)> {
        parse_cells(text).unwrap()
    }

    #[test]
    fn each_key_covers_only_what_it_names() {
        let version = "0.1.4";
        validate(
            "WAIVER-P12-M05",
            &cells("P12-M05:previous,P12-M05:current"),
            version,
        )
        .unwrap();
        assert!(validate("WAIVER-P12-M05", &cells("P12-M11:current"), version).is_err());
        assert!(validate("WAIVER-P12-M03", &cells("P12-M03:current"), version).is_err());
        validate(
            "WAIVER-TIER-B-v0.1.4",
            &cells("P12-M38:current,P12-M39:previous"),
            version,
        )
        .unwrap();
        assert!(validate("WAIVER-TIER-B-v0.1.4", &cells("P12-M05:current"), version).is_err());
        assert!(validate("WAIVER-TIER-B-v0.1.3", &cells("P12-M38:current"), version).is_err());
        // Optional cases are not Tier B, and the retired profile key waives nothing.
        assert!(validate("WAIVER-TIER-B-v0.1.4", &cells("P12-M35:current"), version).is_err());
        assert!(
            validate(
                "WAIVER-PROFILE-PREVIOUS-v0.1.4",
                &cells("P12-M01:previous"),
                version
            )
            .is_err()
        );
    }

    #[test]
    fn empty_duplicate_unknown_or_malformed_cells_are_rejected() {
        assert!(validate("WAIVER-P12-M05", &[], "0.1.4").is_err());
        assert!(
            validate(
                "WAIVER-P12-M05",
                &cells("P12-M05:current,P12-M05:current"),
                "0.1.4"
            )
            .is_err()
        );
        assert!(validate("WAIVER-TIER-B-v0.1.4", &cells("P12-M99:current"), "0.1.4").is_err());
        assert!(validate("WAIVER-EVERYTHING", &cells("P12-M05:current"), "0.1.4").is_err());
        for text in ["P12-M05", "P12-M05:both", ""] {
            assert!(parse_cells(text).is_err(), "{text}");
        }
    }
}
