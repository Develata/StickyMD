//! Ledger lines: writing a record as one JSON line and reading it back, with the whole-ledger
//! read that rejects every ledger holding one bad line.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::{Built, Identity, OpenedZip, Record, Revoked, consistent_with, label, quote};
use crate::mac_ledger::release::ReleaseRecord;
use crate::release::json::{self, Value};

/// Optional trailing members, each with its leading comma; absent values write nothing, so a
/// record without them is written as before.
fn optional_members(
    run_unix: Option<u64>,
    batch: Option<&String>,
    zip: Option<&OpenedZip>,
) -> String {
    let mut out = String::new();
    if let Some(run_unix) = run_unix {
        out.push_str(&format!(",\"run_unix\":{run_unix}"));
    }
    if let Some(batch) = batch {
        out.push_str(&format!(",\"batch\":{}", quote(batch)));
    }
    if let Some(zip) = zip {
        out.push_str(&format!(",\"zip_sha256\":{}", quote(&zip.sha256)));
        if zip.write_watch {
            out.push_str(",\"write_watch\":true");
        }
    }
    out
}

/// The `app_inputs` member with its leading comma, or nothing for a record without inputs.
fn inputs_member(inputs: Option<&String>) -> String {
    inputs.map_or_else(String::new, |inputs| {
        format!(",\"app_inputs\":{}", quote(inputs))
    })
}

impl Record {
    pub fn line(&self) -> String {
        match self {
            Self::Automated {
                row,
                verdict,
                identity,
                built,
                evidence,
                evidence_sha256,
                run_unix,
                batch,
                zip,
                recorded_unix,
            } => format!(
                "{{\"kind\":{},\"row\":{},\"verdict\":{}{},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"image\":{},\"system\":{},\"evidence\":{},\"evidence_sha256\":{}{},\"recorded_unix\":{recorded_unix}}}",
                quote(crate::mac_ledger::producers::kind_of(row)),
                quote(row),
                quote(verdict),
                inputs_member(identity.inputs.as_ref()),
                quote(&built.executable),
                quote(&built.bundle),
                quote(&identity.driver),
                quote(&identity.image),
                quote(&identity.system),
                quote(evidence),
                quote(evidence_sha256),
                optional_members(*run_unix, batch.as_ref(), zip.as_ref()),
            ),
            Self::Manual {
                row,
                inputs,
                app_executable,
                environment,
                quote: words,
                batch,
                recorded_unix,
            } => format!(
                "{{\"kind\":\"manual\",\"row\":{},\"verdict\":\"MANUAL PASS\"{},\"app_executable_sha256\":{},\"environment\":{},\"quote\":{}{},\"recorded_unix\":{recorded_unix}}}",
                quote(row),
                inputs_member(inputs.as_ref()),
                quote(app_executable),
                quote(environment),
                quote(words),
                optional_members(None, batch.as_ref(), None),
            ),
            Self::Revocation {
                row,
                revoked,
                quote: words,
                recorded_unix,
            } => {
                let target = match revoked {
                    Revoked::Evidence(digest) => format!("\"evidence_sha256\":{}", quote(digest)),
                    Revoked::Inputs(inputs) => format!("\"app_inputs\":{}", quote(inputs)),
                    Revoked::Candidate(digest) => {
                        format!("\"candidate_receipt_sha256\":{}", quote(digest))
                    }
                };
                format!(
                    "{{\"kind\":\"revocation\",\"row\":{},{target},\"quote\":{},\"recorded_unix\":{recorded_unix}}}",
                    quote(row),
                    quote(words),
                )
            }
            Self::Release(release) => release.line(),
        }
    }

    fn parse(line: &str) -> Result<Self, String> {
        let value = json::parse(line)?;
        let text =
            |key: &str| -> Result<String, String> { Ok(value.field(key)?.string()?.to_owned()) };
        let recorded_unix = value.field("recorded_unix")?.unsigned()?;
        let inputs = optional_text(&value, "app_inputs")?;
        let kind = text("kind")?;
        let record = match kind.as_str() {
            "automated" | "resources" => Self::Automated {
                row: text("row")?,
                verdict: text("verdict")?,
                identity: Identity {
                    inputs,
                    driver: text("driver_sha256")?,
                    image: text("image")?,
                    system: text("system")?,
                },
                built: Built {
                    executable: text("app_executable_sha256")?,
                    bundle: text("app_bundle_manifest_sha256")?,
                },
                evidence: text("evidence")?,
                evidence_sha256: text("evidence_sha256")?,
                run_unix: optional_unsigned(&value, "run_unix")?,
                batch: optional_text(&value, "batch")?,
                zip: match (
                    optional_text(&value, "zip_sha256")?,
                    value.field("write_watch").ok(),
                ) {
                    (sha256, None) => sha256.map(|sha256| OpenedZip {
                        sha256,
                        write_watch: false,
                    }),
                    (Some(sha256), Some(Value::Bool(true))) => Some(OpenedZip {
                        sha256,
                        write_watch: true,
                    }),
                    _ => {
                        return Err(
                            "write_watch is written only as true, beside the ZIP R03 opened".into(),
                        );
                    }
                },
                recorded_unix,
            },
            "manual" if text("verdict")? == "MANUAL PASS" => Self::Manual {
                row: text("row")?,
                inputs,
                app_executable: text("app_executable_sha256")?,
                environment: text("environment")?,
                quote: text("quote")?,
                batch: optional_text(&value, "batch")?,
                recorded_unix,
            },
            "revocation" => {
                // Exactly its fields: an unknown one (a batch, a schema) would be dropped by every
                // reader and the withdrawal applied wider than it says.
                if let Value::Object(fields) = &value
                    && let Some(extra) = fields.keys().find(|key| {
                        !matches!(
                            key.as_str(),
                            "kind"
                                | "row"
                                | "evidence_sha256"
                                | "app_inputs"
                                | "candidate_receipt_sha256"
                                | "quote"
                                | "recorded_unix"
                        )
                    })
                {
                    return Err(format!("a revocation has no field {extra}"));
                }
                let revoked = match (
                    optional_text(&value, "evidence_sha256")?,
                    inputs,
                    optional_text(&value, "candidate_receipt_sha256")?,
                ) {
                    (Some(digest), None, None) => Revoked::Evidence(digest),
                    (None, Some(inputs), None) => Revoked::Inputs(inputs),
                    (None, None, Some(digest)) => Revoked::Candidate(digest),
                    _ => {
                        return Err(
                            "a revocation names exactly one of evidence_sha256, app_inputs and candidate_receipt_sha256"
                                .into(),
                        );
                    }
                };
                Self::Revocation {
                    row: text("row")?,
                    revoked,
                    quote: text("quote")?,
                    recorded_unix,
                }
            }
            other => match ReleaseRecord::parse(other, &value, recorded_unix)? {
                Some(release) => Self::Release(release),
                None => return Err(format!("unknown ledger record kind {other}")),
            },
        };
        // Review B5: a resource row is kept only by `mac-resources`'s importer, under its own kind;
        // a line of the other kind naming one (an older GUI-shaped import) has no such producer.
        if let Self::Automated { row, .. } = &record
            && crate::mac_ledger::producers::kind_of(row) != kind
        {
            return Err(format!(
                "a {kind:?} record of {} is not one any producer writes",
                label(row)
            ));
        }
        record.validate()?;
        Ok(record.normalized())
    }
}

/// A number member that may be absent; present with any other type, it is an error.
fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, String> {
    match value.field(key) {
        Ok(member) => Ok(Some(member.unsigned()?)),
        Err(_) => Ok(None),
    }
}

/// A string member that may be absent; present with any other type, it is an error.
pub(in crate::mac_ledger) fn optional_text(
    value: &Value,
    key: &str,
) -> Result<Option<String>, String> {
    match value.field(key) {
        Ok(member) => Ok(Some(member.string()?.to_owned())),
        Err(_) => Ok(None),
    }
}

/// Every record of a ledger; one bad line rejects the whole ledger rather than skipping it. Each
/// line is one JSON value (JSON Lines): a byte-order mark or a blank line is an error. Across
/// lines, an evidence file is recorded once per row, and a revocation names a result recorded
/// before it.
pub(crate) fn read_all(text: &str) -> Result<Vec<Record>, String> {
    if text.starts_with('\u{feff}') {
        return Err("ledger line 1: a byte-order mark is not allowed".into());
    }
    let mut records: Vec<Record> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let at = |e: String| format!("ledger line {}: {e}", i + 1);
        if line.trim().is_empty() {
            return Err(at("a blank line is not allowed".into()));
        }
        if line.starts_with('\u{feff}') {
            return Err(at("a byte-order mark is not allowed".into()));
        }
        let record = Record::parse(line).map_err(at)?;
        consistent_with(&records, &record).map_err(at)?;
        records.push(record);
    }
    Ok(records)
}
