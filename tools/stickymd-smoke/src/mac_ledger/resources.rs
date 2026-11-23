//! A finished `mac-resources` run in the ledger (plan 11 #phase-16-macos-host, "资源运行进账本"):
//! P16-R01 and P16-R02, recomputed from the runs in the evidence's bytes, under the identity a
//! `mac-gui` run is kept under (product inputs, driver, VM image, system from `sw_vers`) with both
//! byte identities of the bundle, so the release gate can judge them on a candidate's bytes. As
//! for `mac-gui` evidence, only a finished run in the VM is recorded; schema 4 and older cannot
//! say which bundle or driver they measured, so they are rerun, not recorded.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::record::{self, Built, Identity, Record, optional_text};
use crate::release::json::Value;

/// The `tool` member that marks `mac-resources` evidence.
pub(super) const TOOL: &str = "mac-resources";

pub(super) fn from_evidence(
    value: &Value,
    bytes: &[u8],
    evidence: &str,
    image: &str,
    recorded_unix: u64,
) -> Result<Vec<Record>, String> {
    if !matches!(
        value.field("schema_version").and_then(Value::unsigned),
        Ok(5)
    ) {
        return Err(
            "mac-resources evidence before schema 5 names neither the bundle nor the driver; it cannot be recorded"
                .into(),
        );
    }
    let field =
        |v: &Value, key: &str| -> Result<String, String> { Ok(v.field(key)?.string()?.to_owned()) };
    let environment = value.field("environment")?;
    // Only a run in the dedicated VM is acceptance evidence (plan 11 #phase-16-macos-host).
    if field(environment, "hv_vmm_present")? != "1" {
        return Err("the run was not in a virtual machine; it cannot be recorded".into());
    }
    // The tool rewrites the evidence after every run and marks it finished only after the last.
    if !matches!(value.field("finished"), Ok(Value::Bool(true))) {
        return Err(
            "the run is not finished (a checkpoint or a stopped campaign); it cannot be recorded"
                .into(),
        );
    }
    let identity = Identity {
        inputs: optional_text(value, "app_inputs")?,
        driver: field(value, "driver_sha256")?,
        image: image.to_owned(),
        system: record::system(&field(environment, "os")?),
    };
    let built = Built {
        executable: field(value, "app_executable_sha256")?,
        bundle: field(value, "app_bundle_manifest_sha256")?,
    };
    let run_unix = environment.field("unix_seconds")?.unsigned()?;
    let evidence_sha256 = crate::integrity::sha256_bytes(bytes)?;
    crate::mac_resources::rows::from_evidence(value)?
        .into_iter()
        .map(|row| {
            let record = Record::Automated {
                row: row.row.to_owned(),
                verdict: row.verdict.to_owned(),
                identity: identity.clone(),
                built: built.clone(),
                evidence: evidence.to_owned(),
                evidence_sha256: evidence_sha256.clone(),
                run_unix: Some(run_unix),
                batch: None,
                zip: None,
                recorded_unix,
            };
            record.validate()?;
            Ok(record.normalized())
        })
        .collect()
}
