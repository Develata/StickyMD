//! The USER's target environment receipt: what `current` and `previous` mean for this
//! candidate. The tool records and validates the declaration; it never infers a target.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use super::super::json;

pub(in crate::qualification) const TARGET_RECEIPT: &str = "dist/evidence/manual-targets.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(in crate::qualification) enum Role {
    Current,
    Previous,
}

impl Role {
    pub(in crate::qualification) const ALL: [Self; 2] = [Self::Current, Self::Previous];

    pub(in crate::qualification) const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Previous => "previous",
        }
    }

    pub(in crate::qualification) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "current" => Ok(Self::Current),
            "previous" => Ok(Self::Previous),
            _ => Err(format!("unknown target role `{value}`")),
        }
    }
}

/// The host's native machine as `IsWow64Process2` reports it, not the tool's own target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Machine {
    Amd64,
    Arm64,
}

impl Machine {
    pub(in crate::qualification) const fn as_str(self) -> &'static str {
        match self {
            Self::Amd64 => "AMD64",
            Self::Arm64 => "ARM64",
        }
    }

    pub(in crate::qualification) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "AMD64" => Ok(Self::Amd64),
            "ARM64" => Ok(Self::Arm64),
            _ => Err(format!("unknown native machine `{value}`")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct RoleTarget {
    pub(in crate::qualification) role: Role,
    /// Windows 11 feature version such as `25H2`.
    pub(in crate::qualification) feature_version: String,
    /// `CurrentBuild` of that feature version, such as `26200`.
    pub(in crate::qualification) build_branch: u32,
    /// Approved `UBR` values on `build_branch`: alternatives, not a set to cover one by one.
    pub(in crate::qualification) approved_ubrs: Vec<u32>,
    /// Exact `EditionID` values (SKUs), never policy labels such as Home or Pro.
    pub(in crate::qualification) edition_ids: Vec<String>,
    pub(in crate::qualification) native_machine: Machine,
    /// Where the USER took the approved builds from (Microsoft release history).
    pub(in crate::qualification) source: String,
    /// `YYYY-MM-DD` on which the roles were selected.
    pub(in crate::qualification) selected_on: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Target {
    pub(in crate::qualification) context_id: String,
    pub(in crate::qualification) candidate_receipt_sha256: String,
    /// `current` first, then `previous`.
    pub(in crate::qualification) roles: [RoleTarget; 2],
}

impl Target {
    pub(in crate::qualification) fn role(&self, role: Role) -> &RoleTarget {
        &self.roles[role as usize]
    }
}

/// A fresh 128-bit context identity (32 lowercase hex digits). std has no RNG, but every
/// `RandomState` is keyed from OS randomness, which is all a unique identity needs.
pub(in crate::qualification) fn new_context_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut halves = [0_u64; 2];
    for half in &mut halves {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(nanos);
        hasher.write_u32(std::process::id());
        *half = hasher.finish();
    }
    format!("{:016x}{:016x}", halves[0], halves[1])
}

pub(in crate::qualification) fn is_context_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(in crate::qualification) fn parse(text: &str) -> Result<Target, String> {
    let document = json::parse_object(text)?;
    json::exact_keys(
        &document,
        &[
            "schema_version",
            "context_id",
            "candidate_receipt_sha256",
            "roles",
        ],
    )?;
    if json::u64_field(&document, "schema_version")? != 1 {
        return Err("manual target receipt schema is not version 1".to_owned());
    }
    let context_id = json::string_field(&document, "context_id")?;
    if !is_context_id(&context_id) {
        return Err(format!(
            "manual target context `{context_id}` is not 32 hex digits"
        ));
    }
    let candidate_receipt_sha256 = json::string_field(&document, "candidate_receipt_sha256")?;
    crate::integrity::validate_sha256(&candidate_receipt_sha256, "candidate receipt digest")?;
    let objects = json::objects(&document, "roles")?;
    let [current, previous] = objects else {
        return Err("manual target receipt must declare exactly two roles".to_owned());
    };
    let roles = [
        parse_role(current, Role::Current)?,
        parse_role(previous, Role::Previous)?,
    ];
    if roles[0].feature_version == roles[1].feature_version
        || roles[0].build_branch == roles[1].build_branch
    {
        return Err("manual target roles must name two different Windows 11 versions".to_owned());
    }
    Ok(Target {
        context_id,
        candidate_receipt_sha256,
        roles,
    })
}

fn parse_role(object: &json::Value, expected: Role) -> Result<RoleTarget, String> {
    json::exact_keys(
        object,
        &[
            "role",
            "feature_version",
            "build_branch",
            "approved_builds",
            "edition_ids",
            "native_machine",
            "source",
            "selected_on",
        ],
    )?;
    let role = Role::parse(&json::string_field(object, "role")?)?;
    if role != expected {
        return Err(format!(
            "manual target roles must be `current` then `previous`; found `{}`",
            role.as_str()
        ));
    }
    let feature_version = json::string_field(object, "feature_version")?;
    if !is_feature_version(&feature_version) {
        return Err(format!(
            "feature version `{feature_version}` is not like 25H2"
        ));
    }
    let build_branch = parse_number(&json::string_field(object, "build_branch")?, "build branch")?;
    let mut approved_ubrs = Vec::new();
    for build in json::strings(object, "approved_builds")? {
        let (branch, ubr) = build
            .split_once('.')
            .ok_or_else(|| format!("approved build `{build}` is not CurrentBuild.UBR"))?;
        if parse_number(branch, "approved build branch")? != build_branch {
            return Err(format!(
                "approved build `{build}` is not on branch {build_branch}"
            ));
        }
        let ubr = parse_number(ubr, "approved build UBR")?;
        if approved_ubrs.contains(&ubr) {
            return Err(format!("approved build `{build}` is listed twice"));
        }
        approved_ubrs.push(ubr);
    }
    if approved_ubrs.is_empty() {
        return Err(format!("role {} approves no build", role.as_str()));
    }
    let edition_ids = json::strings(object, "edition_ids")?;
    if edition_ids.is_empty()
        || edition_ids.iter().any(|edition| {
            edition.is_empty()
                || edition.len() > 64
                || !edition.bytes().all(|byte| byte.is_ascii_alphanumeric())
                // Policy labels, not EditionID values (Home is `Core`, Pro is `Professional`).
                || matches!(edition.as_str(), "Home" | "Pro")
        })
    {
        return Err(format!(
            "role {} must list exact EditionID values",
            role.as_str()
        ));
    }
    let mut sorted = edition_ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() != edition_ids.len() {
        return Err(format!("role {} lists an EditionID twice", role.as_str()));
    }
    let native_machine = Machine::parse(&json::string_field(object, "native_machine")?)?;
    let source = json::string_field(object, "source")?;
    if source.trim().is_empty() || source.len() > 512 {
        return Err(format!(
            "role {} needs the Microsoft source of its builds (at most 512 bytes)",
            role.as_str()
        ));
    }
    let selected_on = json::string_field(object, "selected_on")?;
    if !is_date(&selected_on) {
        return Err(format!("selection date `{selected_on}` is not YYYY-MM-DD"));
    }
    Ok(RoleTarget {
        role,
        feature_version,
        build_branch,
        approved_ubrs,
        edition_ids,
        native_machine,
        source,
        selected_on,
    })
}

pub(in crate::qualification) fn render(target: &Target) -> String {
    let roles = target
        .roles
        .iter()
        .map(|role| {
            let builds = role
                .approved_ubrs
                .iter()
                .map(|ubr| format!("\"{}.{ubr}\"", role.build_branch))
                .collect::<Vec<_>>()
                .join(",");
            let editions = role
                .edition_ids
                .iter()
                .map(|edition| format!("\"{}\"", json::escape(edition)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"role\":\"{}\",\"feature_version\":\"{}\",\"build_branch\":\"{}\",\"approved_builds\":[{builds}],\"edition_ids\":[{editions}],\"native_machine\":\"{}\",\"source\":\"{}\",\"selected_on\":\"{}\"}}",
                role.role.as_str(),
                json::escape(&role.feature_version),
                role.build_branch,
                role.native_machine.as_str(),
                json::escape(&role.source),
                json::escape(&role.selected_on),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"context_id\":\"{}\",\"candidate_receipt_sha256\":\"{}\",\"roles\":[{roles}]}}\n",
        target.context_id, target.candidate_receipt_sha256
    )
}

fn is_feature_version(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 4
        && bytes[0].is_ascii_digit()
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'H'
        && matches!(bytes[3], b'1' | b'2')
}

/// A real `YYYY-MM-DD` calendar date (Gregorian leap years).
fn is_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| value[range].parse::<u32>().unwrap_or(0);
    let (year, month, day) = (number(0..4), number(5..7), number(8..10));
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 2021 && (1..=days).contains(&day)
}

fn parse_number(value: &str, label: &str) -> Result<u32, String> {
    if value.is_empty() || value.len() > 9 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{label} `{value}` is not a decimal number"));
    }
    value
        .parse()
        .map_err(|error| format!("{label} `{value}`: {error}"))
}

#[cfg(test)]
pub(in crate::qualification) mod fixtures {
    use super::{Machine, Role, RoleTarget, Target};

    pub(in crate::qualification) fn target() -> Target {
        Target {
            context_id: "0123456789abcdef0123456789abcdef".to_owned(),
            candidate_receipt_sha256: "c".repeat(64),
            roles: [
                RoleTarget {
                    role: Role::Current,
                    feature_version: "25H2".to_owned(),
                    build_branch: 26200,
                    approved_ubrs: vec![6899, 7019],
                    edition_ids: vec!["Core".to_owned(), "Professional".to_owned()],
                    native_machine: Machine::Amd64,
                    source: "https://learn.microsoft.com/windows/release-health".to_owned(),
                    selected_on: "2026-10-14".to_owned(),
                },
                RoleTarget {
                    role: Role::Previous,
                    feature_version: "24H2".to_owned(),
                    build_branch: 26100,
                    approved_ubrs: vec![6899],
                    edition_ids: vec!["Professional".to_owned()],
                    native_machine: Machine::Amd64,
                    source: "https://learn.microsoft.com/windows/release-health".to_owned(),
                    selected_on: "2026-10-14".to_owned(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fixtures::target, is_context_id, new_context_id, parse, render};

    #[test]
    fn a_rendered_target_parses_back_unchanged() {
        let target = target();
        assert_eq!(parse(&render(&target)).unwrap(), target);
    }

    #[test]
    fn malformed_or_ambiguous_targets_are_rejected() {
        let good = render(&target());
        for (from, to) in [
            ("\"schema_version\":1", "\"schema_version\":2"),
            ("\"role\":\"current\"", "\"role\":\"previous\""),
            ("\"25H2\"", "\"24H2\""),
            ("\"26200.7019\"", "\"26100.7019\""),
            ("\"26200.7019\"", "\"26200.6899\""),
            ("\"26200.7019\"", "\"26200\""),
            ("[\"Core\",\"Professional\"]", "[]"),
            ("[\"Core\",\"Professional\"]", "[\"Home\",\"Home\"]"),
            ("[\"Core\",\"Professional\"]", "[\"Pro N\"]"),
            ("[\"Core\",\"Professional\"]", "[\"Home\",\"Pro\"]"),
            (
                "\"selected_on\":\"2026-10-14\"",
                "\"selected_on\":\"2026-02-29\"",
            ),
            (
                "\"selected_on\":\"2026-10-14\"",
                "\"selected_on\":\"2026-99-99\"",
            ),
            ("\"native_machine\":\"AMD64\"", "\"native_machine\":\"x64\""),
            (
                "\"selected_on\":\"2026-10-14\"",
                "\"selected_on\":\"14/10/2026\"",
            ),
            ("\"context_id\":\"0123", "\"context_id\":\"X123"),
            ("\"roles\":[", "\"extra\":1,\"roles\":["),
        ] {
            let broken = good.replacen(from, to, 1);
            assert_ne!(broken, good, "{from}");
            assert!(parse(&broken).is_err(), "{to}");
        }
    }

    #[test]
    fn context_ids_are_fresh_32_digit_hex() {
        let first = new_context_id();
        assert!(is_context_id(&first), "{first}");
        assert_ne!(first, new_context_id());
    }
}
