//! Ledger records: one JSON object per line, append-only. An automated record keeps the identity
//! its PASS can be reused under; a USER receipt keeps the product inputs it holds for. Both keep
//! the built app's hashes for audit. A revocation records the USER withdrawing one of them; the
//! withdrawn result stays as history and is never reused. Every line is validated on read: a malformed or out-of-range
//! record rejects the whole ledger. Records written before the input fingerprint (no
//! `app_inputs`) are read as history and never reused.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use crate::release::json::{self, Value};

/// What a reused PASS must match exactly (plan 11 #phase-16-macos-host).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Identity {
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
pub(super) struct Built {
    pub executable: String,
    pub bundle: String,
}

/// What a revocation withdraws: an automated PASS, named by the digest of the evidence it was
/// recorded from, or the receipts for a row, named by the product inputs they hold for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Revoked {
    Evidence(String),
    Inputs(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Record {
    Automated {
        row: String,
        verdict: String,
        identity: Identity,
        built: Built,
        evidence: String,
        evidence_sha256: String,
        recorded_unix: u64,
    },
    Manual {
        row: String,
        /// The product inputs the receipt holds for; `None` in receipts written before them.
        inputs: Option<String>,
        app_executable: String,
        environment: String,
        quote: String,
        recorded_unix: u64,
    },
    /// The USER withdrew a result after reviewing it (plan 11 #phase-16-macos-host).
    Revocation {
        row: String,
        revoked: Revoked,
        quote: String,
        recorded_unix: u64,
    },
}

const VERDICTS: [&str; 3] = ["PASS", "FAIL", "NOT_TESTED"];

/// `sw_vers` text with every run of whitespace made one space, so the VM's own report and the
/// copy an operator passes compare equal.
pub(super) fn system(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn quote(s: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(s))
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
                recorded_unix,
            } => format!(
                "{{\"kind\":\"automated\",\"row\":{},\"verdict\":{}{},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"image\":{},\"system\":{},\"evidence\":{},\"evidence_sha256\":{},\"recorded_unix\":{recorded_unix}}}",
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
            ),
            Self::Manual {
                row,
                inputs,
                app_executable,
                environment,
                quote: words,
                recorded_unix,
            } => format!(
                "{{\"kind\":\"manual\",\"row\":{},\"verdict\":\"MANUAL PASS\"{},\"app_executable_sha256\":{},\"environment\":{},\"quote\":{},\"recorded_unix\":{recorded_unix}}}",
                quote(row),
                inputs_member(inputs.as_ref()),
                quote(app_executable),
                quote(environment),
                quote(words),
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
                };
                format!(
                    "{{\"kind\":\"revocation\",\"row\":{},{target},\"quote\":{},\"recorded_unix\":{recorded_unix}}}",
                    quote(row),
                    quote(words),
                )
            }
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
        match self {
            Self::Automated {
                row,
                verdict,
                identity,
                built,
                evidence,
                evidence_sha256,
                ..
            } => {
                nonempty("row", row)?;
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
                ..
            } => {
                nonempty("row", row)?;
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
                }
                nonempty("quote", quote)
            }
        }
    }

    fn parse(line: &str) -> Result<Self, String> {
        let value = json::parse(line)?;
        let text =
            |key: &str| -> Result<String, String> { Ok(value.field(key)?.string()?.to_owned()) };
        let recorded_unix = value.field("recorded_unix")?.unsigned()?;
        let inputs = optional_text(&value, "app_inputs")?;
        let record = match text("kind")?.as_str() {
            "automated" => Self::Automated {
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
                recorded_unix,
            },
            "manual" if text("verdict")? == "MANUAL PASS" => Self::Manual {
                row: text("row")?,
                inputs,
                app_executable: text("app_executable_sha256")?,
                environment: text("environment")?,
                quote: text("quote")?,
                recorded_unix,
            },
            "revocation" => {
                let revoked = match (optional_text(&value, "evidence_sha256")?, inputs) {
                    (Some(digest), None) => Revoked::Evidence(digest),
                    (None, Some(inputs)) => Revoked::Inputs(inputs),
                    _ => {
                        return Err(
                            "a revocation names exactly one of evidence_sha256 and app_inputs"
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
            other => return Err(format!("unknown ledger record kind {other}")),
        };
        record.validate()?;
        Ok(record)
    }
}

/// A string member that may be absent; present with any other type, it is an error.
fn optional_text(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.field(key) {
        Ok(member) => Ok(Some(member.string()?.to_owned())),
        Err(_) => Ok(None),
    }
}

/// Every record of a ledger; one bad line rejects the whole ledger rather than skipping it.
pub(super) fn read_all(text: &str) -> Result<Vec<Record>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| Record::parse(line).map_err(|e| format!("ledger line {}: {e}", i + 1)))
        .collect()
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
    let field =
        |v: &Value, key: &str| -> Result<String, String> { Ok(v.field(key)?.string()?.to_owned()) };
    let driver = field(&value, "driver_sha256")
        .map_err(|e| format!("evidence names no driver ({e}); it cannot be recorded"))?;
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
    value
        .field("scenarios")?
        .array()?
        .iter()
        .map(|scenario| {
            let id = field(scenario, "id")?;
            let record = Record::Automated {
                row: id.strip_prefix("P16-").unwrap_or(&id).to_owned(),
                verdict: field(scenario, "verdict")?,
                identity: identity.clone(),
                built: built.clone(),
                evidence: evidence.to_owned(),
                evidence_sha256: evidence_sha256.clone(),
                recorded_unix,
            };
            record.validate()?;
            Ok(record)
        })
        .collect()
}
