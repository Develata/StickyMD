//! The ledger record: one module success filed under its input fingerprint.
//!
//! Records are written only by this tool, so the parse is exact: the full field set,
//! strict JSON, and every digest and name in its canonical form. Lookup and cleanup
//! read records through the same parse.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use crate::integrity::{validate_hex, validate_sha256};
use crate::qualification::json::escape;
use crate::release::json::{self, Value};

const SCHEMA_VERSION: u64 = 2;
const FIELDS: [&str; 11] = [
    "schema_version",
    "status",
    "module_id",
    "input_fingerprint",
    "origin_source_commit",
    "origin_version",
    "origin_exe_sha256",
    "origin_zip_sha256",
    "evidence_file",
    "evidence_sha256",
    "recorded_at_unix",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LedgerRecord {
    pub(super) module_id: String,
    pub(super) input_fingerprint: String,
    pub(super) origin_source_commit: String,
    pub(super) origin_version: String,
    pub(super) origin_exe_sha256: String,
    pub(super) origin_zip_sha256: String,
    pub(super) evidence_sha256: String,
    pub(super) recorded_at_unix: u64,
}

impl LedgerRecord {
    /// The archive name of this record's evidence, `<module>-<sha256>.json`.
    pub(super) fn evidence_file(&self) -> String {
        format!("{}-{}.json", self.module_id, self.evidence_sha256)
    }

    pub(super) fn parse(text: &str) -> Result<Self, String> {
        let root = json::parse(text).map_err(|error| format!("record is malformed: {error}"))?;
        let Value::Object(fields) = &root else {
            return Err("record is not a JSON object".to_owned());
        };
        if fields.len() != FIELDS.len() || FIELDS.iter().any(|key| !fields.contains_key(*key)) {
            return Err(format!("record fields are not exactly {FIELDS:?}"));
        }
        if root.field("schema_version")?.unsigned()? != SCHEMA_VERSION
            || root.field("status")?.string()? != "PASSED"
        {
            return Err("record has an unsupported schema or status".to_owned());
        }
        let text = |key: &str| root.field(key)?.string().map(str::to_owned);
        let record = Self {
            module_id: text("module_id")?,
            input_fingerprint: text("input_fingerprint")?,
            origin_source_commit: text("origin_source_commit")?,
            origin_version: text("origin_version")?,
            origin_exe_sha256: text("origin_exe_sha256")?,
            origin_zip_sha256: text("origin_zip_sha256")?,
            evidence_sha256: text("evidence_sha256")?,
            recorded_at_unix: root.field("recorded_at_unix")?.unsigned()?,
        };
        validate_sha256(&record.input_fingerprint, "record input fingerprint")?;
        validate_sha256(&record.origin_exe_sha256, "record origin EXE SHA-256")?;
        validate_sha256(&record.origin_zip_sha256, "record origin ZIP SHA-256")?;
        validate_sha256(&record.evidence_sha256, "record evidence SHA-256")?;
        validate_hex(
            &record.origin_source_commit,
            40,
            "record origin source commit",
        )?;
        if record.origin_version.is_empty() {
            return Err("record has no origin version".to_owned());
        }
        if text("evidence_file")? != record.evidence_file() {
            return Err("record evidence name does not match its module and digest".to_owned());
        }
        Ok(record)
    }

    pub(super) fn render(&self) -> String {
        format!(
            concat!(
                "{{\"schema_version\":{},\"status\":\"PASSED\",",
                "\"module_id\":\"{}\",\"input_fingerprint\":\"{}\",",
                "\"origin_source_commit\":\"{}\",\"origin_version\":\"{}\",",
                "\"origin_exe_sha256\":\"{}\",\"origin_zip_sha256\":\"{}\",",
                "\"evidence_file\":\"{}\",\"evidence_sha256\":\"{}\",",
                "\"recorded_at_unix\":{}}}\n"
            ),
            SCHEMA_VERSION,
            escape(&self.module_id),
            escape(&self.input_fingerprint),
            escape(&self.origin_source_commit),
            escape(&self.origin_version),
            escape(&self.origin_exe_sha256),
            escape(&self.origin_zip_sha256),
            escape(&self.evidence_file()),
            escape(&self.evidence_sha256),
            self.recorded_at_unix,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::LedgerRecord;

    fn record() -> LedgerRecord {
        LedgerRecord {
            module_id: "g4".to_owned(),
            input_fingerprint: "1".repeat(64),
            origin_source_commit: "a".repeat(40),
            origin_version: "0.1.0".to_owned(),
            origin_exe_sha256: "c".repeat(64),
            origin_zip_sha256: "d".repeat(64),
            evidence_sha256: "e".repeat(64),
            recorded_at_unix: 7,
        }
    }

    #[test]
    fn records_round_trip_and_reject_anything_but_the_exact_form() {
        let text = record().render();
        assert_eq!(LedgerRecord::parse(&text).unwrap(), record());
        for broken in [
            text.trim_end().trim_end_matches('}').to_owned(),
            text.replace(
                "\"status\":\"PASSED\"",
                "\"status\" : \"FAILED\",\"x\":{\"status\":\"PASSED\"}",
            ),
            text.replace(
                "\"recorded_at_unix\":7",
                "\"recorded_at_unix\":7,\"recorded_at_unix\":8",
            ),
            text.replace(
                "\"recorded_at_unix\":7",
                "\"recorded_at_unix\":7,\"extra\":1",
            ),
            text.replace("\"schema_version\":2", "\"schema_version\":1"),
            text.replace("\"origin_version\":\"0.1.0\"", "\"origin_version\":\"\""),
            text.replace(&"e".repeat(64), &"f".repeat(64)).replacen(
                &"f".repeat(64),
                &"e".repeat(64),
                1,
            ),
            text.replace(&"a".repeat(40), "not-a-commit"),
        ] {
            assert!(LedgerRecord::parse(&broken).is_err(), "{broken}");
        }
    }
}
