//! Advisory last-failure hints: ordering only, never reusable success evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{DIRECTORY, MAX_AGE, digest, lookup, now, read_bounded};
use crate::{
    evidence::{self, EvidenceResult},
    release::json,
    resource_plan::diagnostic::Unit,
};
use std::{fs, path::Path, time::SystemTime};

#[cfg(test)]
#[path = "priority_tests.rs"]
mod tests;

#[derive(Default)]
pub(crate) struct Selection {
    pub(crate) first: Option<Unit>,
    hint: Option<Hint>,
}

struct Hint {
    unit: Unit,
    document: String,
}

/// Read once per command, so planning, task order and forced sampling agree.
pub(crate) fn select(root: &Path, units: &[Unit], enabled: bool) -> Selection {
    let (hint, reason) = match read(root) {
        Ok(Some(hint)) if units.contains(&hint.unit) => (Some(hint), "IN_SCOPE"),
        Ok(Some(_)) => (None, "OUT_OF_SCOPE"),
        Ok(None) => (None, "MISSING_OR_CLEARED"),
        Err(_) => (None, "INVALID_OR_EXPIRED"),
    };
    let first = enabled.then(|| hint.as_ref().map(|h| h.unit)).flatten();
    if enabled {
        eprintln!(
            "RESOURCE_FAILURE_FIRST unit={} status={} reason={reason} (advisory; selected failure runs fresh)",
            first.map_or("none", Unit::key),
            if first.is_some() {
                "SELECTED"
            } else {
                "FIXED_ORDER"
            }
        );
    }
    Selection { first, hint }
}

impl Selection {
    /// Historical/partial success cannot clear a hint. Do not overwrite a newer revision.
    pub(crate) fn complete(
        &self,
        root: &Path,
        unit: Unit,
        result: &EvidenceResult,
    ) -> Result<(), String> {
        let Some(hint) = self.hint.as_ref().filter(|hint| hint.unit == unit) else {
            return Ok(());
        };
        let payload = evidence::render_json(
            "diagnostic-only",
            true,
            None,
            None,
            "failure-hint-check",
            std::slice::from_ref(result),
            None,
        );
        unit.validate(&json::parse(&payload)?.field("results")?.array()?[0])?;
        let current = match read(root) {
            Ok(current) => current,
            Err(_) => {
                // Hints only guide ordering. Expiry or damage while a long measurement runs
                // cannot turn complete fresh observations into a failed resource result.
                eprintln!(
                    "RESOURCE_FAILURE_HINT unit={} status=NOT_CLEARED reason=UNAVAILABLE_OR_EXPIRED",
                    unit.key()
                );
                return Ok(());
            }
        };
        if current.is_some_and(|current| current.document == hint.document) {
            // This is an advisory compare-before-write, not a cross-process transaction.
            write(root, &encode(unit, now()?, "CLEARED")?)?;
            eprintln!(
                "RESOURCE_FAILURE_HINT unit={} status=CLEARED_FRESH_SUCCESS",
                unit.key()
            );
        }
        Ok(())
    }
}

pub(crate) fn failed(root: &Path, unit: Unit) -> Result<(), String> {
    write(root, &encode(unit, now()?, "FAILED")?)?;
    eprintln!("RESOURCE_FAILURE_HINT unit={} status=RECORDED", unit.key());
    Ok(())
}

fn path(root: &Path) -> Result<std::path::PathBuf, String> {
    lookup::checked(root, root.join(DIRECTORY).join("last-failure.json"))
}

fn read(root: &Path) -> Result<Option<Hint>, String> {
    let path = path(root)?;
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(None);
    }
    decode(&read_bounded(&path)?, now()?)
}

fn write(root: &Path, document: &str) -> Result<(), String> {
    let destination = path(root)?;
    fs::create_dir_all(destination.parent().ok_or("missing failure hint parent")?)
        .map_err(|e| e.to_string())?;
    crate::atomic_evidence::write(&path(root)?, document.as_bytes())
}

fn encode(unit: Unit, created: u64, state: &str) -> Result<String, String> {
    if Unit::from_key(unit.key()) != Some(unit) || !matches!(state, "FAILED" | "CLEARED") {
        return Err("unregistered diagnostic failure hint".into());
    }
    // A new revision distinguishes a later failure of the same unit in the same second.
    let revision = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let body = format!(
        "{{\"schema_version\":1,\"kind\":\"DIAGNOSTIC_FAILURE_HINT\",\"unit\":\"{}\",\"created\":{created},\"revision\":\"{revision}-{}\",\"state\":\"{state}\"}}",
        unit.key(),
        std::process::id()
    );
    Ok(format!(
        "{{\"sha256\":\"{}\",\"body\":\"{}\"}}",
        digest::bytes(body.as_bytes())?,
        evidence::escape_json(&body)
    ))
}

fn decode(document: &str, current: u64) -> Result<Option<Hint>, String> {
    if document.len() > 4096 {
        return Err("failure hint too large".into());
    }
    let outer = json::parse(document)?;
    let body = outer.field("body")?.string()?;
    if digest::bytes(body.as_bytes())? != outer.field("sha256")?.string()? {
        return Err("failure hint checksum mismatch".into());
    }
    let value = json::parse(body)?;
    let created = value.field("created")?.unsigned()?;
    if value.field("schema_version")?.unsigned()? != 1
        || value.field("kind")?.string()? != "DIAGNOSTIC_FAILURE_HINT"
        || created > current
        || current - created > MAX_AGE
        || value.field("revision")?.string()?.is_empty()
    {
        return Err("invalid or expired failure hint".into());
    }
    let unit = Unit::from_key(value.field("unit")?.string()?).ok_or("unknown failure hint unit")?;
    match value.field("state")?.string()? {
        "FAILED" => Ok(Some(Hint {
            unit,
            document: document.into(),
        })),
        "CLEARED" => Ok(None),
        _ => Err("unknown failure hint state".into()),
    }
}
