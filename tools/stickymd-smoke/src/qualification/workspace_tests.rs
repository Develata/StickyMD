//! Shared, source-bound full workspace test prerequisite for formal local channels.
//! plan_ref: docs/plan/11_testing_and_release.md#shared-headless-prerequisite

mod identity;
#[cfg(test)]
mod tests;

use crate::cli::{Options, Phase, Selection};
use crate::evidence::{EvidenceMeasurement, EvidenceResult, EvidenceStatus};
use crate::release::json;
use std::{path::Path, time::Instant};

pub(crate) const ARGS: [&str; 3] = ["test", "--workspace", "--locked"];
pub(crate) const RECEIPT: &str = "dist/evidence/source-success/workspace-tests.json";
const COMMAND: &str = "cargo test --workspace --locked";

#[derive(Clone, Debug, PartialEq)]
struct Identity {
    source: String,
    fingerprint: String,
    reusable: bool,
}

pub(crate) fn eligible(root: &Path, options: &Options) -> bool {
    use super::module_ledger::{ModuleId, module_for_receipt};
    options.selection == Selection::Phase(Phase::P14)
        && options.json
        && !options.ci
        && !options.release
        && !options.package
        && !options.resources
        && options.resource_module.is_none()
        && options.ci_shard.is_none()
        && options.evidence_file.as_deref().is_some_and(|path| {
            matches!(
                (
                    module_for_receipt(root, path),
                    options.runtime,
                    options.performance
                ),
                (Some(ModuleId::Runtime), true, false) | (Some(ModuleId::Performance), false, true)
            )
        })
}

pub(crate) fn execute(root: &Path, run: impl FnOnce() -> Result<(), String>) -> EvidenceResult {
    execute_with(root, || identity::capture(root), run)
}

fn execute_with(
    root: &Path,
    mut capture: impl FnMut() -> Result<Identity, String>,
    run: impl FnOnce() -> Result<(), String>,
) -> EvidenceResult {
    let mut result = EvidenceResult {
        id: "workspace tests".into(),
        status: EvidenceStatus::Failed,
        detail: None,
        measurements: Vec::new(),
        gates: Vec::new(),
        samples: Vec::new(),
    };
    let outcome = (|| {
        let started = Instant::now();
        let before = capture()?;
        result
            .measurements
            .push(seconds("workspace.identity_before_seconds", started));
        if before.reusable {
            if let Some(original_seconds) = compatible(root, &before) {
                // Revalidate bytes after reading the receipt; never reuse an identity snapshot indefinitely.
                let started = Instant::now();
                if capture()? != before {
                    return Err("workspace inputs changed during prerequisite lookup".into());
                }
                result
                    .measurements
                    .push(seconds("workspace.identity_after_seconds", started));
                result.measurements.push(EvidenceMeasurement {
                    name: "workspace.origin_run_seconds".into(),
                    unit: "seconds".into(),
                    value: original_seconds,
                });
                return Ok(format!(
                    "REUSED_PASS SOURCE_BOUND origin_source={} input_fingerprint={} receipt={RECEIPT}",
                    before.source, before.fingerprint
                ));
            }
        } else {
            eprintln!("WORKSPACE_TESTS cache=BYPASS reason=unrecognized_execution_configuration");
        }
        let started = Instant::now();
        let outcome = run();
        let elapsed = seconds("workspace.run_seconds", started);
        let run_seconds = elapsed.value;
        result.measurements.push(elapsed);
        outcome?;
        let started = Instant::now();
        if capture()? != before {
            return Err("workspace inputs changed during tests; refusing source success".into());
        }
        result
            .measurements
            .push(seconds("workspace.identity_after_seconds", started));
        if before.reusable {
            let document = format!(
                concat!(
                    "{{\"schema_version\":1,\"status\":\"PASSED\",\"evidence_class\":\"SOURCE_BOUND\",",
                    "\"scope\":\"FULL_WORKSPACE\",\"command\":\"{}\",\"source_commit\":\"{}\",",
                    "\"input_fingerprint\":\"{}\",\"run_seconds\":{:.6}}}\n"
                ),
                COMMAND,
                super::json::escape(&before.source),
                super::json::escape(&before.fingerprint),
                run_seconds
            );
            crate::atomic_evidence::write(&root.join(RECEIPT), document.as_bytes())?;
        }
        Ok(format!(
            "RAN_PASS SOURCE_BOUND origin_source={} input_fingerprint={} shared_receipt={}",
            before.source,
            before.fingerprint,
            if before.reusable {
                "RECORDED"
            } else {
                "BYPASS"
            }
        ))
    })();
    match outcome {
        Ok(detail) => {
            result.status = EvidenceStatus::Passed;
            result.detail = Some(detail);
        }
        Err(error) => result.detail = Some(error),
    }
    result
}

fn seconds(name: &str, started: Instant) -> EvidenceMeasurement {
    EvidenceMeasurement {
        name: name.into(),
        unit: "seconds".into(),
        value: started.elapsed().as_secs_f64(),
    }
}

fn compatible(root: &Path, identity: &Identity) -> Option<f64> {
    let text = std::fs::read_to_string(root.join(RECEIPT)).ok()?;
    read_success(&text, identity).ok()
}

fn read_success(text: &str, identity: &Identity) -> Result<f64, String> {
    let document = json::parse(text)?;
    if document.field("schema_version")?.unsigned()? != 1 {
        return Err("unsupported shared workspace receipt".into());
    }
    for (field, expected) in [
        ("status", "PASSED"),
        ("evidence_class", "SOURCE_BOUND"),
        ("scope", "FULL_WORKSPACE"),
        ("command", COMMAND),
        ("source_commit", identity.source.as_str()),
        ("input_fingerprint", identity.fingerprint.as_str()),
    ] {
        if document.field(field)?.string()? != expected {
            return Err(format!("shared workspace receipt differs at {field}"));
        }
    }
    let json::Value::Number(value) = document.field("run_seconds")? else {
        return Err("missing workspace elapsed time".into());
    };
    let elapsed = value
        .parse::<f64>()
        .map_err(|_| "invalid workspace elapsed time")?;
    if !elapsed.is_finite() || elapsed < 0.0 {
        return Err("invalid workspace elapsed time".into());
    }
    Ok(elapsed)
}
