//! Ledger records: one JSON object per line, append-only. An automated record keeps the identity
//! its PASS can be reused under; a USER receipt keeps the product inputs it holds for. Both keep
//! the built app's hashes for audit. A revocation records the USER withdrawing one of them; the
//! withdrawn result stays as history and is never reused. Every line is validated on read: a malformed or out-of-range
//! record rejects the whole ledger. Records written before the input fingerprint (no
//! `app_inputs`) are read as history and never reused. An R03 record names the release ZIP it
//! opened (`zip_sha256`) when `mac-gui --zip` gave one; records without it read as before. A
//! finished `mac-resources` run is recorded too (`resources`).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::release::ReleaseRecord;
use crate::release::json::{self, Value};

/// What a reused PASS must match exactly (plan 11 #phase-16-macos-host).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Identity {
    /// The product input fingerprint `build-app.sh` wrote into the bundle; `None` for an app
    /// built before that, which nothing can match.
    pub inputs: Option<String>,
    pub driver: String,
    pub image: String,
    /// The VM's `sw_vers` output, whitespace collapsed.
    pub system: String,
}

impl Identity {
    /// A PASS under `self` stands for `other` only when both name the same product inputs.
    pub fn reusable_for(&self, other: &Self) -> bool {
        self.inputs.is_some() && self == other
    }
}

/// The built app's bytes, kept for audit only: the same inputs can build different bytes, and the
/// version label alone changes them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Built {
    pub executable: String,
    pub bundle: String,
}

/// What a revocation withdraws: an automated PASS, named by the digest of the evidence it was
/// recorded from; the receipts for a row, named by the product inputs they hold for; or the
/// release records for a row (receipts, waivers, download records), named by the candidate
/// receipt they are bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Revoked {
    Evidence(String),
    Inputs(String),
    Candidate(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Record {
    Automated {
        row: String,
        verdict: String,
        identity: Identity,
        built: Built,
        evidence: String,
        evidence_sha256: String,
        /// When the run started (the evidence's `environment.unix_seconds`); absent in records
        /// written before it was kept.
        run_unix: Option<u64>,
        /// The acceptance batch, for the never-reused rows (plan 11 #phase-16-macos-host).
        batch: Option<String>,
        /// The SHA-256 of the release ZIP R03 opened (`mac-gui --zip`); only R03 records carry it,
        /// and only when a ZIP was given (plan 11 #phase-16-macos-host).
        zip: Option<String>,
        recorded_unix: u64,
    },
    Manual {
        row: String,
        /// The product inputs the receipt holds for; `None` in receipts written before them.
        inputs: Option<String>,
        app_executable: String,
        environment: String,
        quote: String,
        batch: Option<String>,
        recorded_unix: u64,
    },
    /// A USER record bound to one promoted candidate (plan 11 #macos-readiness-check).
    Release(ReleaseRecord),
    /// The USER withdrew a result after reviewing it (plan 11 #phase-16-macos-host).
    Revocation {
        row: String,
        revoked: Revoked,
        quote: String,
        recorded_unix: u64,
    },
}

const VERDICTS: [&str; 3] = ["PASS", "FAIL", "NOT_TESTED"];

/// The key an image is grouped by: its content digest `sha256:<hex>` (an OCI digest names the
/// content, whatever repository name precedes it), lowercased; a value without one is kept
/// lowercased as written.
pub(crate) fn image_key(image: &str) -> String {
    let lower = image.to_ascii_lowercase();
    match lower.rsplit_once("sha256:") {
        Some((_, hex)) if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
            format!("sha256:{hex}")
        }
        _ => lower,
    }
}

/// `sw_vers` text with every run of whitespace made one space, so the VM's own report and the
/// copy an operator passes compare equal.
pub(super) fn system(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 64 hex digits in either case; records are read with digests lowercased (`Record::normalized`),
/// so string equality compares digests, and new records are written lowercase.
fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

use super::options::is_batch;

fn quote(s: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(s))
}

/// Optional trailing members, each with its leading comma; absent values write nothing, so a
/// record without them is written as before.
fn optional_members(run_unix: Option<u64>, batch: Option<&String>, zip: Option<&String>) -> String {
    let mut out = String::new();
    if let Some(run_unix) = run_unix {
        out.push_str(&format!(",\"run_unix\":{run_unix}"));
    }
    if let Some(batch) = batch {
        out.push_str(&format!(",\"batch\":{}", quote(batch)));
    }
    if let Some(zip) = zip {
        out.push_str(&format!(",\"zip_sha256\":{}", quote(zip)));
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
                quote(super::producers::kind_of(row)),
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

    /// The invariants every record keeps, checked on read and before append.
    pub fn validate(&self) -> Result<(), String> {
        let nonempty = |name: &str, value: &str| {
            if value.trim().is_empty() {
                Err(format!("{name} is empty"))
            } else {
                Ok(())
            }
        };
        let hash = |name: &str, value: &str| {
            if is_sha256(value) {
                Ok(())
            } else {
                Err(format!("{name} is not a SHA-256: {value:?}"))
            }
        };
        let inputs = |value: Option<&String>| value.map_or(Ok(()), |v| hash("app_inputs", v));
        let batch = |value: Option<&String>| match value {
            Some(b) if !is_batch(b) => Err(format!("batch {b:?} is not 1-64 of [A-Za-z0-9._-]")),
            _ => Ok(()),
        };
        match self {
            Self::Automated {
                row,
                verdict,
                identity,
                built,
                evidence,
                evidence_sha256,
                batch: run_batch,
                zip,
                ..
            } => {
                nonempty("row", row)?;
                batch(run_batch.as_ref())?;
                if let Some(zip) = zip {
                    if row != "R03" {
                        return Err(format!("only R03 opens a ZIP; P16-{row} names one"));
                    }
                    hash("zip_sha256", zip)?;
                }
                if !VERDICTS.contains(&verdict.as_str()) {
                    return Err(format!("unknown verdict {verdict:?}"));
                }
                inputs(identity.inputs.as_ref())?;
                hash("app_executable_sha256", &built.executable)?;
                hash("app_bundle_manifest_sha256", &built.bundle)?;
                hash("driver_sha256", &identity.driver)?;
                hash("evidence_sha256", evidence_sha256)?;
                nonempty("image", &identity.image)?;
                nonempty("system", &identity.system)?;
                nonempty("evidence", evidence)
            }
            Self::Manual {
                row,
                inputs: receipt_inputs,
                app_executable,
                environment,
                quote,
                batch: receipt_batch,
                ..
            } => {
                nonempty("row", row)?;
                batch(receipt_batch.as_ref())?;
                inputs(receipt_inputs.as_ref())?;
                hash("app_executable_sha256", app_executable)?;
                nonempty("environment", environment)?;
                nonempty("quote", quote)
            }
            Self::Revocation {
                row,
                revoked,
                quote,
                ..
            } => {
                nonempty("row", row)?;
                match revoked {
                    Revoked::Evidence(digest) => hash("evidence_sha256", digest)?,
                    Revoked::Inputs(inputs) => hash("app_inputs", inputs)?,
                    Revoked::Candidate(digest) => hash("candidate_receipt_sha256", digest)?,
                }
                nonempty("quote", quote)
            }
            Self::Release(release) => release.validate(),
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
                zip: optional_text(&value, "zip_sha256")?,
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
            && super::producers::kind_of(row) != kind
        {
            return Err(format!(
                "a {kind:?} record of {} is not one any producer writes",
                label(row)
            ));
        }
        record.validate()?;
        Ok(record.normalized())
    }

    /// The record with every digest lowercased; the ledger's bytes stay as written.
    pub(super) fn normalized(mut self) -> Self {
        let lower = |s: &mut String| s.make_ascii_lowercase();
        match &mut self {
            Self::Automated {
                identity,
                built,
                evidence_sha256,
                zip,
                ..
            } => {
                identity.inputs.as_mut().map(lower);
                // One image, one key (review B4): its content digest, whatever name or case a
                // record spelled it with; the ledger's bytes keep the spelling.
                identity.image = image_key(&identity.image);
                zip.as_mut().map(lower);
                lower(&mut identity.driver);
                lower(&mut built.executable);
                lower(&mut built.bundle);
                lower(evidence_sha256);
            }
            Self::Manual {
                inputs,
                app_executable,
                ..
            } => {
                inputs.as_mut().map(lower);
                lower(app_executable);
            }
            Self::Revocation { revoked, .. } => match revoked {
                Revoked::Evidence(digest)
                | Revoked::Inputs(digest)
                | Revoked::Candidate(digest) => lower(digest),
            },
            Self::Release(release) => lower(&mut release.candidate),
        }
        self
    }
}

/// A number member that may be absent; present with any other type, it is an error.
pub(super) fn optional_unsigned(value: &Value, key: &str) -> Result<Option<u64>, String> {
    match value.field(key) {
        Ok(member) => Ok(Some(member.unsigned()?)),
        Err(_) => Ok(None),
    }
}

/// A string member that may be absent; present with any other type, it is an error.
pub(super) fn optional_text(value: &Value, key: &str) -> Result<Option<String>, String> {
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

/// Whether `record` may follow `earlier`: no second record of the same evidence for a row, and a
/// revocation only of a result already in the ledger. Run order is not checked: wall-clock starts
/// cannot prove it, so the reuse rule does not depend on it (`decide::plan`).
/// A row as the matrices name it: Phase 16 rows are stored without their prefix.
pub(crate) fn label(row: &str) -> String {
    if row.starts_with("P18-") {
        row.to_owned()
    } else {
        format!("P16-{row}")
    }
}

pub(super) fn consistent_with(earlier: &[Record], record: &Record) -> Result<(), String> {
    match record {
        Record::Automated {
            row,
            evidence_sha256,
            ..
        } => {
            let replayed = earlier.iter().any(|r| {
                matches!(r, Record::Automated { row: r_row, evidence_sha256: digest, .. }
                    if r_row == row && digest == evidence_sha256)
            });
            if replayed {
                return Err(format!(
                    "P16-{row} from evidence {evidence_sha256} is already recorded; a replay cannot stand for a new run"
                ));
            }
        }
        Record::Revocation { row, revoked, .. } => {
            let held = earlier.iter().any(|r| match (r, revoked) {
                (
                    Record::Automated {
                        row: r_row,
                        verdict,
                        evidence_sha256,
                        ..
                    },
                    Revoked::Evidence(digest),
                ) => r_row == row && verdict == "PASS" && evidence_sha256 == digest,
                (
                    Record::Manual {
                        row: r_row,
                        inputs: Some(held),
                        ..
                    },
                    Revoked::Inputs(inputs),
                ) => r_row == row && held == inputs,
                (Record::Release(release), Revoked::Candidate(digest)) => {
                    &release.row == row && &release.candidate == digest
                }
                _ => false,
            });
            if !held {
                return Err(format!(
                    "the revocation of {} names no result recorded before it",
                    label(row)
                ));
            }
        }
        Record::Manual { .. } | Record::Release(_) => {}
    }
    Ok(())
}

/// The records of a finished `mac-gui` run, one per row, from one read of its bytes (the hash
/// and the parsed verdicts describe the same file). Evidence written before the driver hash was
/// recorded cannot say which driver produced it, so it is refused.
pub(super) fn from_evidence(
    bytes: &[u8],
    evidence: &str,
    image: &str,
    recorded_unix: u64,
) -> Result<Vec<Record>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("evidence is not UTF-8: {e}"))?;
    let value = json::parse(text)?;
    // Each producer has its own schema and rows: mac-resources writes R01 and R02, mac-gui and
    // mac-logout (no `tool` member) only the rows they run.
    match optional_text(&value, "tool")?.as_deref() {
        Some(super::resources::TOOL) => {
            return super::resources::from_evidence(&value, bytes, evidence, image, recorded_unix);
        }
        Some(other) => return Err(format!("evidence of tool {other:?} cannot be recorded")),
        None => {}
    }
    // Schema 2 is the first whose `finished` only the current rules set and that names the batch
    // the run was started for; an older file cannot say either, so it is rerun, not recorded.
    if !matches!(
        value.field("schema_version").and_then(Value::unsigned),
        Ok(2)
    ) {
        return Err("the evidence is not schema 2 of mac-gui (an older driver) nor schema 5 of mac-resources; it cannot be recorded".into());
    }
    // The release ZIP R03 opened, kept with R03 alone.
    let zip = optional_text(&value, "zip_sha256")?;
    if let Some(digest) = zip.as_deref().filter(|d| !is_sha256(d)) {
        return Err(format!(
            "the evidence names a ZIP digest that is not a SHA-256: {digest:?}"
        ));
    }
    let batch = optional_text(&value, "batch")?;
    if let Some(name) = batch.as_deref().filter(|b| !super::options::is_batch(b)) {
        return Err(format!("the evidence names an invalid batch {name:?}"));
    }
    let field =
        |v: &Value, key: &str| -> Result<String, String> { Ok(v.field(key)?.string()?.to_owned()) };
    let driver = field(&value, "driver_sha256")
        .map_err(|e| format!("evidence names no driver ({e}); it cannot be recorded"))?;
    // Only a run in the dedicated VM is acceptance evidence (plan 11 #phase-16-macos-host).
    let environment = value.field("environment")?;
    if field(environment, "hv_vmm_present")? != "1" {
        return Err("the run was not in a virtual machine; it cannot be recorded".into());
    }
    if !matches!(environment.field("allow_host")?, Value::Bool(false)) {
        return Err("the run allowed a host; it cannot be recorded".into());
    }
    let run_unix = environment.field("unix_seconds")?.unsigned()?;
    // The driver rewrites the evidence after every row and marks it finished only after the last.
    if !matches!(value.field("finished"), Ok(Value::Bool(true))) {
        return Err(
            "the run is not finished (a checkpoint or an older driver); it cannot be recorded"
                .into(),
        );
    }
    // mac-logout's evidence names its case (and boot); mac-gui's names none (review S4).
    let logout_case = optional_text(environment, "case")?;
    let identity = Identity {
        inputs: optional_text(&value, "app_inputs")?,
        driver,
        image: image.to_owned(),
        system: system(&field(value.field("environment")?, "os")?),
    };
    let built = Built {
        executable: field(&value, "app_executable_sha256")?,
        bundle: field(&value, "app_bundle_manifest_sha256")?,
    };
    let evidence_sha256 = crate::integrity::sha256_bytes(bytes)?;
    let mut records: Vec<Record> = Vec::new();
    for scenario in value.field("scenarios")?.array()? {
        let id = field(scenario, "id")?;
        let row = id.strip_prefix("P16-").unwrap_or(&id).to_owned();
        if records
            .iter()
            .any(|r| matches!(r, Record::Automated { row: seen, .. } if *seen == row))
        {
            return Err(format!("P16-{row} appears twice in the evidence"));
        }
        if !super::producers::gui_row(&row, logout_case.as_deref()) {
            return Err(format!(
                "P16-{row} is not a row mac-gui or mac-logout runs; it cannot be recorded from this evidence"
            ));
        }
        let verdict = field(scenario, "verdict")?;
        let checks = scenario.field("checks")?.array()?;
        if row == "R03" && verdict == "PASS" {
            super::producers::r03_pass(checks, zip.as_deref())?;
        }
        // A row stopped by the run's shared stop, or never started, means the run was cut short;
        // an older driver could still mark such a run finished, so the rows themselves decide.
        for check in checks {
            let name = field(check, "name")?;
            let unstarted = name == "execution"
                && check
                    .field("observation")
                    .and_then(|o| Ok(o.string()? == "not started"))
                    .unwrap_or(false);
            if name == "shared stop" || unstarted {
                return Err(format!(
                    "P16-{row} was not run (check {name:?}); the run was cut short and cannot be recorded"
                ));
            }
        }
        let checks = checks
            .iter()
            .map(|check| field(check, "verdict"))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(unknown) = checks.iter().find(|c| !VERDICTS.contains(&c.as_str())) {
            return Err(format!(
                "P16-{row} has a check with unknown verdict {unknown:?}"
            ));
        }
        // The driver's aggregation: FAIL first, then NOT_TESTED, then PASS; no check is not a PASS.
        let expected = if checks.iter().any(|c| c == "FAIL") {
            "FAIL"
        } else if checks.is_empty() || checks.iter().any(|c| c != "PASS") {
            "NOT_TESTED"
        } else {
            "PASS"
        };
        if verdict != expected {
            return Err(format!(
                "P16-{row} says {verdict} but its checks give {expected}"
            ));
        }
        let record = Record::Automated {
            zip: zip.clone().filter(|_| row == "R03"),
            row,
            verdict,
            identity: identity.clone(),
            built: built.clone(),
            evidence: evidence.to_owned(),
            evidence_sha256: evidence_sha256.clone(),
            run_unix: Some(run_unix),
            batch: batch.clone(),
            recorded_unix,
        };
        record.validate()?;
        // The ledger stores digests in lower case whatever case the evidence used.
        records.push(record.normalized());
    }
    if records.is_empty() {
        return Err("the evidence has no rows".into());
    }
    if zip.is_some()
        && !records
            .iter()
            .any(|r| matches!(r, Record::Automated { row, .. } if row == "R03"))
    {
        return Err(
            "the evidence names a ZIP but has no R03 row, the only row that opens one".into(),
        );
    }
    Ok(records)
}
