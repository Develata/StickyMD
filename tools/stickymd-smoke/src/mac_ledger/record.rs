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

mod codec;
mod consistency;
mod evidence;

pub(super) use codec::optional_text;
pub(crate) use codec::read_all;
pub(super) use consistency::{consistent_with, resource_pairs};
pub(super) use evidence::from_evidence;

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
        /// The release ZIP R03 opened (`mac-gui --zip`); only R03 records carry it, and only when
        /// a ZIP was given (plan 11 #phase-16-macos-host).
        zip: Option<OpenedZip>,
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
    match super::options::image_digest(image) {
        Ok(reference) => reference
            .rsplit_once('@')
            .map_or(reference.clone(), |(_, digest)| digest.to_owned()),
        Err(_) => image.to_owned(),
    }
}

/// Whether an image key names a content digest; one that does not (an old line's tag, a
/// malformed reference) leaves its record's identity unknown (review round 3, B5).
pub(crate) fn image_attributable(key: &str) -> bool {
    key.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
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

/// The release ZIP an R03 run opened, and whether its "nothing written" was read through the write
/// watch: the importer sets `write_watch` only from evidence whose R03 facts carry the watch, so
/// a record an older importer wrote, or of a driver that only compared snapshots, has none, and
/// the release gate does not count its PASS (review round 3, B1).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpenedZip {
    pub sha256: String,
    pub write_watch: bool,
}

impl Record {
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
                    hash("zip_sha256", &zip.sha256)?;
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
                if let Some(zip) = zip.as_mut() {
                    lower(&mut zip.sha256);
                }
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

/// A row as the matrices name it: Phase 16 rows are stored without their prefix.
pub(crate) fn label(row: &str) -> String {
    if row.starts_with("P18-") {
        row.to_owned()
    } else {
        format!("P16-{row}")
    }
}
