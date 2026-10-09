//! Cache lookup explanations; advisory latest pointers never authorize historical identities.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{DIRECTORY, Identity, Store, now, read_bounded, record};
use crate::{
    evidence::EvidenceResult,
    qualification::{path_identity, receipt},
    release::json,
    resource_plan::diagnostic::Unit,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) struct Lookup {
    pub(super) result: Option<EvidenceResult>,
    pub(super) reason: String,
}

impl Store {
    pub(super) fn inspect(&self, root: &Path, unit: Unit) -> Result<Lookup, String> {
        let mut requested = self.inspect_exact(root, unit)?;
        if requested.result.is_some() {
            return Ok(requested);
        }
        for origin in unit.equivalents() {
            let cached = self.inspect_exact(root, origin)?;
            if let Some(mut result) = cached.result {
                unit.project(origin, &mut result)?;
                return Ok(Lookup {
                    result: Some(result),
                    reason: format!("EQUIVALENT_COMPLETE_RECORD: {}", origin.key()),
                });
            }
            if cached.reason != "MISSING" {
                requested.reason.push_str(&format!(
                    "; equivalent {}: {}",
                    origin.key(),
                    cached.reason
                ));
            }
        }
        Ok(requested)
    }

    fn inspect_exact(&self, root: &Path, unit: Unit) -> Result<Lookup, String> {
        let path = self.path(root, unit)?;
        let exists = path.try_exists().map_err(|e| e.to_string())?;
        if !exists {
            return Ok(Lookup {
                result: None,
                reason: self.missing_reason(root, unit)?,
            });
        }
        match read_bounded(&path)
            .and_then(|text| record::decode(&text, &self.identity, unit, now()?))
        {
            Ok(result) => Ok(Lookup {
                result: Some(result),
                reason: "VALID_COMPLETE_RECORD".into(),
            }),
            Err(error) => Ok(Lookup {
                result: None,
                reason: format!("INVALID_RECORD: {}", bounded(&error)),
            }),
        }
    }

    fn missing_reason(&self, root: &Path, unit: Unit) -> Result<String, String> {
        let hint = latest_path(root, unit)?;
        if !hint.try_exists().map_err(|e| e.to_string())? {
            return Ok("MISSING".into());
        }
        let previous = (|| {
            let key = read_bounded(&hint)?;
            receipt::validate_sha256(&key, "latest diagnostic key")?;
            let path = cache_path(root, &key, unit)?;
            let document = read_bounded(&path)?;
            changes(&document, &self.identity, unit)
        })();
        Ok(match previous {
            Ok(reason) if !reason.is_empty() => format!("IDENTITY_CHANGED: {}", reason.join(",")),
            Ok(_) => "MISSING_CURRENT_RECORD".into(),
            Err(error) => format!("MISSING; LATEST_UNAVAILABLE: {}", bounded(&error)),
        })
    }

    pub(super) fn save_hint(&self, root: &Path, unit: Unit) -> Result<(), String> {
        let path = latest_path(root, unit)?;
        fs::create_dir_all(path.parent().ok_or("missing hint parent")?)
            .map_err(|e| e.to_string())?;
        crate::atomic_evidence::write(
            &latest_path(root, unit)?,
            self.identity.fingerprint.as_bytes(),
        )
    }
}

pub(super) fn cache_path(root: &Path, key: &str, unit: Unit) -> Result<PathBuf, String> {
    receipt::validate_sha256(key, "diagnostic key")?;
    unit.registered()?;
    checked(
        root,
        root.join(DIRECTORY)
            .join(key)
            .join(format!("{}.json", unit.key())),
    )
}

fn latest_path(root: &Path, unit: Unit) -> Result<PathBuf, String> {
    unit.registered()?;
    checked(
        root,
        root.join(DIRECTORY)
            .join("latest")
            .join(format!("{}.key", unit.key())),
    )
}

pub(super) fn checked(root: &Path, path: PathBuf) -> Result<PathBuf, String> {
    if !path_identity::is_within(root, &path, "target")
        || !path_identity::is_within(root, &path, DIRECTORY)
        || path_identity::is_within(root, &path, "dist")
    {
        return Err("diagnostic cache path escaped its target directory".into());
    }
    Ok(path)
}

fn changes(document: &str, current: &Identity, unit: Unit) -> Result<Vec<&'static str>, String> {
    let outer = json::parse(document)?;
    let body = outer.field("body")?.string()?;
    if super::digest::bytes(body.as_bytes())? != outer.field("sha256")?.string()? {
        return Err("CHECKSUM_MISMATCH".into());
    }
    let metadata = json::parse(body)?;
    if metadata.field("schema_version")?.unsigned()? != 2
        || metadata.field("unit")?.string()? != unit.key()
    {
        return Err("INCOMPATIBLE_LATEST".into());
    }
    let mut changed = Vec::new();
    for (field, value) in [
        ("source", &current.source),
        ("executable", &current.executable),
        ("harness", &current.harness),
        ("inputs", &current.inputs),
        ("environment", &current.environment),
    ] {
        if metadata.field(field)?.string()? != value {
            changed.push(field);
        }
    }
    if changed.is_empty() && metadata.field("identity")?.string()? != current.fingerprint {
        changed.push("protocol_or_identity");
    }
    Ok(changed)
}

fn bounded(reason: &str) -> String {
    reason
        .chars()
        .filter(|c| !c.is_control())
        .take(240)
        .collect()
}
