//! Versioned, checksummed diagnostic records and strict five-observation decoding.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Identity, MAX_AGE, MAX_BYTES, digest};
use crate::{
    evidence::{
        self, EvidenceGate, EvidenceMeasurement, EvidenceResult, EvidenceSample, EvidenceStatus,
    },
    release::json::{self, Value},
    resource_plan::{self, ResourceCase},
};

pub(super) fn encode(
    identity: &Identity,
    case: ResourceCase,
    result: &EvidenceResult,
    elapsed: f64,
    created: u64,
) -> Result<String, String> {
    if !elapsed.is_finite() || elapsed < 0.0 {
        return Err("invalid diagnostic duration".into());
    }
    let payload = evidence::render_json(
        &identity.source,
        identity.dirty,
        None,
        Some(&identity.executable),
        "resource-diagnostic-cohort",
        std::slice::from_ref(result),
        None,
    );
    let parsed = json::parse(&payload)?;
    resource_plan::validate_case(&parsed.field("results")?.array()?[0], case)?;
    let body = format!(
        "{{\"schema_version\":1,\"kind\":\"DIAGNOSTIC_ONLY\",\"identity\":\"{}\",\"source\":\"{}\",\"executable\":\"{}\",\"harness\":\"{}\",\"case\":\"{}\",\"created\":{created},\"origin_execution_seconds\":{elapsed},\"payload\":\"{}\"}}",
        evidence::escape_json(&identity.fingerprint),
        evidence::escape_json(&identity.source),
        evidence::escape_json(&identity.executable),
        evidence::escape_json(&identity.harness),
        case.label,
        evidence::escape_json(&payload)
    );
    let document = envelope(&body)?;
    if document.len() as u64 > MAX_BYTES {
        return Err("diagnostic record too large".into());
    }
    Ok(document)
}

fn envelope(body: &str) -> Result<String, String> {
    Ok(format!(
        "{{\"sha256\":\"{}\",\"body\":\"{}\"}}",
        digest::bytes(body.as_bytes())?,
        evidence::escape_json(body)
    ))
}

pub(super) fn decode(
    document: &str,
    identity: &Identity,
    case: ResourceCase,
    now: u64,
) -> Result<EvidenceResult, String> {
    let outer = json::parse(document)?;
    let body = outer.field("body")?.string()?;
    if digest::bytes(body.as_bytes())? != outer.field("sha256")?.string()? {
        return Err("diagnostic cache checksum mismatch".into());
    }
    let metadata = json::parse(body)?;
    if metadata.field("schema_version")?.unsigned()? != 1
        || metadata.field("kind")?.string()? != "DIAGNOSTIC_ONLY"
        || metadata.field("identity")?.string()? != identity.fingerprint
        || metadata.field("source")?.string()? != identity.source
        || metadata.field("executable")?.string()? != identity.executable
        || metadata.field("harness")?.string()? != identity.harness
        || metadata.field("case")?.string()? != case.label
    {
        return Err("incompatible diagnostic cache identity".into());
    }
    let created = metadata.field("created")?.unsigned()?;
    if now.checked_sub(created).is_none_or(|age| age > MAX_AGE) {
        return Err("expired or future diagnostic cache".into());
    }
    let elapsed = number(metadata.field("origin_execution_seconds")?)?;
    if elapsed < 0.0 {
        return Err("invalid diagnostic duration".into());
    }
    let payload = json::parse(metadata.field("payload")?.string()?)?;
    if payload.field("suite")?.string()? != "resource-diagnostic-cohort"
        || payload.field("schema_version")?.unsigned()? != 2
        || payload.field("commit")?.string()? != identity.source
        || payload.field("executable_sha256")?.string()? != identity.executable
        || payload.field("worktree_dirty")? != &Value::Bool(identity.dirty)
    {
        return Err("incompatible diagnostic payload".into());
    }
    let results = payload.field("results")?.array()?;
    if results.len() != 1 {
        return Err("cache must contain exactly one case".into());
    }
    resource_plan::validate_case(&results[0], case)?;
    let value = &results[0];
    let origin = format!("diagnostic-cache:{}:{created}", identity.fingerprint);
    let mut summary = measurements(value.field("measurements")?)?;
    summary.push(EvidenceMeasurement {
        name: format!("{}.origin_execution_seconds", case.label),
        unit: "seconds".into(),
        value: elapsed,
    });
    Ok(EvidenceResult {
        id: case.label.into(),
        status: EvidenceStatus::Passed,
        detail: Some(format!(
            "DIAGNOSTIC_REUSED {origin}; origin_source={}; origin_exe_sha256={}; origin_harness_sha256={}; origin_execution_seconds={elapsed}",
            identity.source, identity.executable, identity.harness
        )),
        measurements: summary,
        gates: value
            .field("gates")?
            .array()?
            .iter()
            .map(|g| {
                Ok(EvidenceGate {
                    metric: g.field("metric")?.string()?.into(),
                    comparator: g.field("comparator")?.string()?.into(),
                    value: number(g.field("value")?)?,
                    unit: g.field("unit")?.string()?.into(),
                    source: g.field("source")?.string()?.into(),
                })
            })
            .collect::<Result<_, String>>()?,
        samples: value
            .field("samples")?
            .array()?
            .iter()
            .map(|s| {
                Ok(EvidenceSample {
                    cohort: s.field("cohort")?.string()?.into(),
                    run: s.field("run")?.unsigned()? as usize,
                    shared_from: Some(origin.clone()),
                    measurements: measurements(s.field("measurements")?)?,
                })
            })
            .collect::<Result<_, String>>()?,
    })
}

fn number(value: &Value) -> Result<f64, String> {
    let Value::Number(text) = value else {
        return Err("not a number".into());
    };
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| "invalid diagnostic number".into())
}

fn measurements(value: &Value) -> Result<Vec<EvidenceMeasurement>, String> {
    value
        .array()?
        .iter()
        .map(|m| {
            Ok(EvidenceMeasurement {
                name: m.field("name")?.string()?.into(),
                unit: m.field("unit")?.string()?.into(),
                value: number(m.field("value")?)?,
            })
        })
        .collect()
}
