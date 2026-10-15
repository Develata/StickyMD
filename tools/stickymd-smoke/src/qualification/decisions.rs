//! Source-freeze projection of USER release decisions.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::path::Path;

use super::file_lock::{SidecarGuard, sidecar};
use super::json;
use super::manual_observation::{log::is_reset_decision, store, target::Role, waiver};
use super::receipt;
use super::source_freeze::{self, SourceFreeze};

const SOURCE_LEDGER: &str = "docs/report/phase-12-release-decisions.md";
pub(super) const DECISION_RECEIPT: &str = "dist/evidence/release-decisions.json";
const DECISION_LOCK: &str = "dist/evidence/release-decisions.lock";

/// Every change to the decision receipt is a read-modify-write under this exclusive lock;
/// readers that must agree with a manual observation context take it shared, after the
/// manual store lock (see `file_lock`).
fn lock(root: &Path, exclusive: bool) -> Result<SidecarGuard, String> {
    sidecar(
        &root.join(DECISION_LOCK),
        exclusive,
        "release decision receipt",
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Decision {
    pub(super) key: String,
    pub(super) status: String,
    pub(super) evidence: String,
    /// For a manual waiver: the observation context and the `(case, role)` cells it
    /// disposes of. `None` on a waiver means the superseded format, which no longer
    /// waives anything.
    pub(super) scope: Option<WaiverScope>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WaiverScope {
    pub(super) context_id: String,
    pub(super) cells: waiver::Cells,
}

pub(super) fn project(root: &Path, source: &SourceFreeze) -> Result<(), String> {
    let _guard = lock(root, true)?;
    let source_document = fs::read_to_string(root.join(SOURCE_LEDGER))
        .map_err(|error| format!("cannot read release decision ledger: {error}"))?;
    let mut decisions = parse_markdown(&source_document)?;
    // The template records historical policy; each freeze needs its own version authority.
    if let Some(version) = decisions
        .iter_mut()
        .find(|decision| decision.key == "RELEASE-VERSION")
    {
        version.status = "PENDING".to_owned();
        version.evidence = format!(
            "USER approval required for version {} at Source Freeze {}",
            source.version, source.source_commit
        );
    }
    write(root, source, &decisions)
}

/// Records a USER decision. A manual waiver also needs `manual_scope`
/// (`CASE:ROLE,...`); it is bound to the current manual observation context, so a new
/// context never inherits it.
pub(super) fn update(
    root: &Path,
    key: &str,
    status: &str,
    evidence: &str,
    manual_scope: Option<&str>,
) -> Result<(), String> {
    let source = source_freeze::read(root)?;
    source_freeze::validate_against_repository(root, &source)?;
    let status = normalize_status(status)?;
    if evidence.trim().is_empty() {
        return Err("decision evidence must not be empty".to_owned());
    }
    let waiver_key = waiver::is_waiver_key(key);
    // Lock order: the manual store (to pin the context a waiver binds to), then decisions.
    let _manual = waiver_key.then(|| store::lock(root, false)).transpose()?;
    let _decisions = lock(root, true)?;
    let mut decisions = read(root, &source)?;
    let scope = match (waiver_key, manual_scope) {
        (true, Some(text)) => {
            let cells = waiver::parse_cells(text)?;
            waiver::validate(key, &cells, &source.version)?;
            let (_, candidate) = receipt::read_candidate_snapshot(root)?;
            let context = store::load(root, &candidate).map_err(|error| error.message())?;
            Some(WaiverScope {
                context_id: context.target.context_id,
                cells,
            })
        }
        (true, None) => {
            return Err(format!(
                "manual waiver {key} needs --manual-scope=CASE:ROLE,..."
            ));
        }
        (false, Some(_)) => {
            return Err("--manual-scope applies only to manual waivers".to_owned());
        }
        (false, None) => None,
    };
    if let Some(decision) = decisions.iter_mut().find(|decision| decision.key == key) {
        decision.status = status.to_owned();
        decision.evidence = evidence.to_owned();
        decision.scope = scope;
    } else if scope.is_some() || is_reset_decision(key) {
        decisions.push(Decision {
            key: key.to_owned(),
            status: status.to_owned(),
            evidence: evidence.to_owned(),
            scope,
        });
    } else {
        return Err(format!("unknown release decision `{key}`"));
    }
    write(root, &source, &decisions)
}

/// The decisions as one consistent snapshot (shared lock). A caller that also needs the
/// manual observation context takes the manual store lock first.
pub(super) fn read_consistent(root: &Path, source: &SourceFreeze) -> Result<Vec<Decision>, String> {
    let _guard = lock(root, false)?;
    read(root, source)
}

/// Whether the USER approved `key` for the current Source Freeze, read at call time. A
/// receipt that cannot be read or locked is an error, not a missing approval.
pub(super) fn is_approved(root: &Path, key: &str) -> Result<bool, String> {
    let source = source_freeze::read(root)?;
    let decisions = read_consistent(root, &source)?;
    Ok(status(&decisions, key) == Some("USER APPROVED"))
}

/// Whether `path` names the decision receipt or its lock, or anything below them; only
/// this module writes them.
pub(super) fn is_internal_path(root: &Path, path: &Path) -> bool {
    [DECISION_RECEIPT, DECISION_LOCK]
        .iter()
        .any(|reserved| super::path_identity::reaches(root, path, reserved))
}

pub(super) fn read(root: &Path, source: &SourceFreeze) -> Result<Vec<Decision>, String> {
    let document = json::parse_object(&receipt::read_receipt(&root.join(DECISION_RECEIPT))?)?;
    if json::u64_field(&document, "schema_version")? != 2 {
        return Err("release decision receipt schema is not version 2".to_owned());
    }
    for (field, expected) in [
        ("source_commit", source.source_commit.as_str()),
        ("cargo_lock_sha256", source.cargo_lock_sha256.as_str()),
        ("version", source.version.as_str()),
    ] {
        let actual = json::string_field(&document, field)?;
        if actual != expected {
            return Err(format!(
                "STALE RECEIPT: decision {field} is {actual}, expected {expected}"
            ));
        }
    }
    parse_json_decisions(&document)
}

pub(super) fn status<'a>(decisions: &'a [Decision], key: &str) -> Option<&'a str> {
    decisions
        .iter()
        .find(|decision| decision.key == key)
        .map(|decision| decision.status.as_str())
}

fn parse_markdown(content: &str) -> Result<Vec<Decision>, String> {
    let mut decisions = Vec::new();
    for line in content.lines() {
        if !line.trim_start().starts_with("| DEC-") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 4 {
            return Err("decision ledger rows must contain four columns".to_owned());
        }
        let status = normalize_status(cells[2])?;
        if decisions
            .iter()
            .any(|decision: &Decision| decision.key == cells[1])
        {
            return Err(format!("duplicate decision key `{}`", cells[1]));
        }
        decisions.push(Decision {
            key: cells[1].to_owned(),
            status: status.to_owned(),
            evidence: cells[3].to_owned(),
            scope: None,
        });
    }
    if decisions.is_empty() {
        return Err("decision ledger contains no DEC rows".to_owned());
    }
    Ok(decisions)
}

/// Each decision is read from its own object in the `decisions` array.
fn parse_json_decisions(document: &json::Value) -> Result<Vec<Decision>, String> {
    let mut decisions = Vec::new();
    for object in json::objects(document, "decisions")? {
        let key = json::string_field(object, "decision")?;
        let scope = if json::has_field(object, "manual_context") {
            if !waiver::is_waiver_key(&key) {
                return Err(format!(
                    "decision `{key}` is not a waiver but has a manual scope"
                ));
            }
            let cells = json::objects(object, "scope")?
                .iter()
                .map(|cell| {
                    Ok((
                        json::string_field(cell, "case")?,
                        Role::parse(&json::string_field(cell, "role")?)?,
                    ))
                })
                .collect::<Result<waiver::Cells, String>>()?;
            Some(WaiverScope {
                context_id: json::string_field(object, "manual_context")?,
                cells,
            })
        } else {
            None
        };
        let decision = Decision {
            key,
            status: json::string_field(object, "status")?,
            evidence: json::string_field(object, "evidence")?,
            scope,
        };
        normalize_status(&decision.status)?;
        if decisions
            .iter()
            .any(|existing: &Decision| existing.key == decision.key)
        {
            return Err(format!("duplicate decision key `{}`", decision.key));
        }
        decisions.push(decision);
    }
    if decisions.is_empty() {
        return Err("release decision receipt contains no decisions".to_owned());
    }
    Ok(decisions)
}

fn write(root: &Path, source: &SourceFreeze, decisions: &[Decision]) -> Result<(), String> {
    let mut document = format!(
        "{{\"schema_version\":2,\"source_commit\":\"{}\",\"cargo_lock_sha256\":\"{}\",\"version\":\"{}\",\"decisions\":[",
        json::escape(&source.source_commit),
        json::escape(&source.cargo_lock_sha256),
        json::escape(&source.version),
    );
    for (index, decision) in decisions.iter().enumerate() {
        if index > 0 {
            document.push(',');
        }
        let scope = decision.scope.as_ref().map_or_else(String::new, |scope| {
            let cells = scope
                .cells
                .iter()
                .map(|(case, role)| {
                    format!(
                        "{{\"case\":\"{}\",\"role\":\"{}\"}}",
                        json::escape(case),
                        role.as_str()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                ",\"manual_context\":\"{}\",\"scope\":[{cells}]",
                json::escape(&scope.context_id)
            )
        });
        document.push_str(&format!(
            "{{\"decision\":\"{}\",\"status\":\"{}\",\"evidence\":\"{}\"{scope}}}",
            json::escape(&decision.key),
            json::escape(&decision.status),
            json::escape(&decision.evidence),
        ));
    }
    document.push_str("]}\n");
    receipt::write_receipt(root, DECISION_RECEIPT, &document)
}

fn normalize_status(value: &str) -> Result<&'static str, String> {
    match value.to_ascii_uppercase().replace(['-', '_'], " ").as_str() {
        "PENDING" => Ok("PENDING"),
        "USER APPROVED" => Ok("USER APPROVED"),
        "USER REJECTED" => Ok("USER REJECTED"),
        "NOT APPLICABLE" => Ok("NOT APPLICABLE"),
        _ => Err(format!("invalid USER decision status `{value}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_status, parse_json_decisions, parse_markdown, status};

    #[test]
    fn source_freeze_requires_fresh_version_approval() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-decision-version-{nonce}"));
        std::fs::create_dir_all(root.join("docs/report")).expect("fixture directory");
        std::fs::write(
            root.join(super::SOURCE_LEDGER),
            "| DEC-02 | RELEASE-VERSION | USER APPROVED | USER approved 0.1.0 |\n",
        )
        .expect("historical template");
        let source = super::SourceFreeze {
            source_commit: "a".repeat(40),
            version: "0.1.1".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            rustc: "rustc test".to_owned(),
            target: crate::qualification::receipt::RELEASE_TARGET.to_owned(),
            remote_synced: false,
        };
        super::project(&root, &source).expect("project current freeze");
        let decisions = super::read(&root, &source).expect("read projected decisions");
        std::fs::remove_dir_all(&root).expect("cleanup fixture");
        assert_eq!(
            super::status(&decisions, "RELEASE-VERSION"),
            Some("PENDING")
        );
    }

    #[test]
    fn source_ledger_rejects_duplicate_or_inferred_decisions() {
        let source = "| DEC-01 | STARTUP-RELEASE-BOUNDARY | USER APPROVED | USER message |\n";
        let decisions = parse_markdown(source).expect("valid ledger");
        assert_eq!(decisions[0].key, "STARTUP-RELEASE-BOUNDARY");
        assert!(normalize_status("approved").is_err());
    }

    #[test]
    fn a_decision_is_read_from_its_own_object() {
        let receipt = |decision: &str| {
            crate::qualification::json::parse_object(&format!(
                "{{\"schema_version\":2,\"decisions\":[{decision}]}}"
            ))
            .unwrap()
        };
        let rejected = receipt(
            "{\"decision\":\"RELEASE-VERSION\",\"status\" : \"USER REJECTED\",\"extra\":{\"status\":\"USER APPROVED\"},\"evidence\":\"rejected\"}",
        );
        let decisions = parse_json_decisions(&rejected).unwrap();
        assert_eq!(status(&decisions, "RELEASE-VERSION"), Some("USER REJECTED"));
        for broken in [
            "{\"decision\":\"RELEASE-VERSION\",\"status\":\"USER APPROVED\"}",
            "{\"decision\":\"A\",\"status\":\"PENDING\",\"evidence\":\"x\"},{\"decision\":\"A\",\"status\":\"PENDING\",\"evidence\":\"y\"}",
            "\"RELEASE-VERSION\"",
        ] {
            assert!(parse_json_decisions(&receipt(broken)).is_err(), "{broken}");
        }
    }

    #[test]
    fn a_waiver_scope_round_trips_and_only_waivers_carry_one() {
        let receipt = |decision: &str| {
            crate::qualification::json::parse_object(&format!(
                "{{\"schema_version\":2,\"decisions\":[{decision}]}}"
            ))
            .unwrap()
        };
        let scoped = receipt(
            r#"{"decision":"WAIVER-P12-M05","status":"USER APPROVED","evidence":"x","manual_context":"0123456789abcdef0123456789abcdef","scope":[{"case":"P12-M05","role":"previous"}]}"#,
        );
        let decisions = parse_json_decisions(&scoped).unwrap();
        let scope = decisions[0].scope.as_ref().unwrap();
        assert_eq!(scope.context_id, "0123456789abcdef0123456789abcdef");
        assert_eq!(scope.cells.len(), 1);
        // The superseded format still parses but carries no scope, so it waives nothing.
        let old =
            receipt(r#"{"decision":"WAIVER-P12-M05","status":"USER APPROVED","evidence":"x"}"#);
        assert_eq!(parse_json_decisions(&old).unwrap()[0].scope, None);
        let misplaced = receipt(
            r#"{"decision":"RELEASE-VERSION","status":"USER APPROVED","evidence":"x","manual_context":"0123456789abcdef0123456789abcdef","scope":[]}"#,
        );
        assert!(parse_json_decisions(&misplaced).is_err());
    }
}
