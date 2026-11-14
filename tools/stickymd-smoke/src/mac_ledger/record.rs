//! Ledger records: one JSON object per line, append-only. An automated record keeps the identity
//! its PASS can be reused under; a USER receipt keeps the app executable it holds for. Every line
//! is validated on read: a malformed or out-of-range record rejects the whole ledger.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use crate::release::json::{self, Value};

/// What a reused PASS must match exactly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Identity {
    pub app_executable: String,
    pub app_bundle: String,
    pub driver: String,
    pub image: String,
    /// The VM's `sw_vers` output, whitespace collapsed.
    pub system: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Record {
    Automated {
        row: String,
        verdict: String,
        identity: Identity,
        evidence: String,
        evidence_sha256: String,
        recorded_unix: u64,
    },
    Manual {
        row: String,
        app_executable: String,
        environment: String,
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

impl Record {
    pub fn line(&self) -> String {
        match self {
            Self::Automated {
                row,
                verdict,
                identity,
                evidence,
                evidence_sha256,
                recorded_unix,
            } => format!(
                "{{\"kind\":\"automated\",\"row\":{},\"verdict\":{},\"app_executable_sha256\":{},\"app_bundle_manifest_sha256\":{},\"driver_sha256\":{},\"image\":{},\"system\":{},\"evidence\":{},\"evidence_sha256\":{},\"recorded_unix\":{recorded_unix}}}",
                quote(row),
                quote(verdict),
                quote(&identity.app_executable),
                quote(&identity.app_bundle),
                quote(&identity.driver),
                quote(&identity.image),
                quote(&identity.system),
                quote(evidence),
                quote(evidence_sha256),
            ),
            Self::Manual {
                row,
                app_executable,
                environment,
                quote: words,
                recorded_unix,
            } => format!(
                "{{\"kind\":\"manual\",\"row\":{},\"verdict\":\"MANUAL PASS\",\"app_executable_sha256\":{},\"environment\":{},\"quote\":{},\"recorded_unix\":{recorded_unix}}}",
                quote(row),
                quote(app_executable),
                quote(environment),
                quote(words),
            ),
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
        match self {
            Self::Automated {
                row,
                verdict,
                identity,
                evidence,
                evidence_sha256,
                ..
            } => {
                nonempty("row", row)?;
                if !VERDICTS.contains(&verdict.as_str()) {
                    return Err(format!("unknown verdict {verdict:?}"));
                }
                hash("app_executable_sha256", &identity.app_executable)?;
                hash("app_bundle_manifest_sha256", &identity.app_bundle)?;
                hash("driver_sha256", &identity.driver)?;
                hash("evidence_sha256", evidence_sha256)?;
                nonempty("image", &identity.image)?;
                nonempty("system", &identity.system)?;
                nonempty("evidence", evidence)
            }
            Self::Manual {
                row,
                app_executable,
                environment,
                quote,
                ..
            } => {
                nonempty("row", row)?;
                hash("app_executable_sha256", app_executable)?;
                nonempty("environment", environment)?;
                nonempty("quote", quote)
            }
        }
    }

    fn parse(line: &str) -> Result<Self, String> {
        let value = json::parse(line)?;
        let text =
            |key: &str| -> Result<String, String> { Ok(value.field(key)?.string()?.to_owned()) };
        let recorded_unix = value.field("recorded_unix")?.unsigned()?;
        let record = match text("kind")?.as_str() {
            "automated" => Self::Automated {
                row: text("row")?,
                verdict: text("verdict")?,
                identity: Identity {
                    app_executable: text("app_executable_sha256")?,
                    app_bundle: text("app_bundle_manifest_sha256")?,
                    driver: text("driver_sha256")?,
                    image: text("image")?,
                    system: text("system")?,
                },
                evidence: text("evidence")?,
                evidence_sha256: text("evidence_sha256")?,
                recorded_unix,
            },
            "manual" if text("verdict")? == "MANUAL PASS" => Self::Manual {
                row: text("row")?,
                app_executable: text("app_executable_sha256")?,
                environment: text("environment")?,
                quote: text("quote")?,
                recorded_unix,
            },
            other => return Err(format!("unknown ledger record kind {other}")),
        };
        record.validate()?;
        Ok(record)
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
        app_executable: field(&value, "app_executable_sha256")?,
        app_bundle: field(&value, "app_bundle_manifest_sha256")?,
        driver,
        image: image.to_owned(),
        system: system(&field(value.field("environment")?, "os")?),
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
                evidence: evidence.to_owned(),
                evidence_sha256: evidence_sha256.clone(),
                recorded_unix,
            };
            record.validate()?;
            Ok(record)
        })
        .collect()
}
