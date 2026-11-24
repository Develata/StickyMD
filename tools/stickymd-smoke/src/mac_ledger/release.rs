//! Ledger records bound to one promoted macOS candidate (plan 11 #macos-readiness-check, USER
//! 2026-11-24): the USER's release receipt for a Manual row, the USER's waiver of a waivable row,
//! and the USER's record of the real download path (P18-D01–D05). Each names the digest of the
//! candidate receipt, which binds the ZIP and both byte identities of the bundle, so it stands
//! for that candidate only. The Phase 16 reuse rules ignore them; the release gate reads them.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use crate::release::json::Value;

/// The download-path rows of the Phase 18 matrix, stored with their prefix (Phase 16 rows are
/// stored without theirs).
/// What the USER records for the download paths whose result has more than one form (USER
/// 2026-11-24, decision (b)): P18-D04, which of the three first-open cases happened, and P18-D05,
/// whether Gatekeeper blocked the first open. Every other download row records none.
pub(crate) fn outcomes(row: &str) -> Option<&'static [&'static str]> {
    match row {
        "P18-D04" => Some(&[
            "gatekeeper-blocked",
            "translocated-prompt-exit",
            "portable-start",
        ]),
        "P18-D05" => Some(&["blocked", "not-blocked"]),
        _ => None,
    }
}

pub(crate) const DOWNLOAD_ROWS: [&str; 5] = ["P18-D01", "P18-D02", "P18-D03", "P18-D04", "P18-D05"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReleaseKind {
    /// The USER judged a Manual row on this candidate and it holds.
    Receipt,
    /// The USER waives a waivable row that stays NOT TESTED for this candidate.
    Waiver,
    /// The USER walked one download path with this candidate's ZIP: the signing the candidate
    /// carries and the `sw_vers` of the system it ran on, kept to judge when it must be redone.
    Download {
        signing: String,
        system: String,
        /// The observed result, for P18-D04 and P18-D05 only (see [`outcomes`]).
        outcome: Option<String>,
    },
}

impl ReleaseKind {
    fn name(&self) -> &'static str {
        match self {
            Self::Receipt => "release-receipt",
            Self::Waiver => "release-waiver",
            Self::Download { .. } => "download",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReleaseRecord {
    pub kind: ReleaseKind,
    /// A Phase 16 row without its prefix, or one of [`DOWNLOAD_ROWS`].
    pub row: String,
    /// The SHA-256 of the candidate receipt the USER's words hold for.
    pub candidate: String,
    pub environment: String,
    pub quote: String,
    pub batch: Option<String>,
    pub recorded_unix: u64,
}

fn quote(s: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(s))
}

impl ReleaseRecord {
    pub fn line(&self) -> String {
        let download = match &self.kind {
            ReleaseKind::Download {
                signing,
                system,
                outcome,
            } => format!(
                ",\"signing\":{},\"system\":{}{}",
                quote(signing),
                quote(system),
                outcome
                    .as_ref()
                    .map_or_else(String::new, |o| format!(",\"outcome\":{}", quote(o)))
            ),
            _ => String::new(),
        };
        let batch = self
            .batch
            .as_ref()
            .map_or_else(String::new, |b| format!(",\"batch\":{}", quote(b)));
        format!(
            "{{\"kind\":\"{}\",\"row\":{},\"candidate_receipt_sha256\":{}{download},\"environment\":{},\"quote\":{}{batch},\"recorded_unix\":{}}}",
            self.kind.name(),
            quote(&self.row),
            quote(&self.candidate),
            quote(&self.environment),
            quote(&self.quote),
            self.recorded_unix
        )
    }

    pub fn validate(&self) -> Result<(), String> {
        let nonempty = |name: &str, value: &str| {
            if value.trim().is_empty() {
                Err(format!("{name} is empty"))
            } else {
                Ok(())
            }
        };
        nonempty("row", &self.row)?;
        nonempty("environment", &self.environment)?;
        nonempty("quote", &self.quote)?;
        if !(self.candidate.len() == 64 && self.candidate.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(format!(
                "candidate_receipt_sha256 is not a SHA-256: {:?}",
                self.candidate
            ));
        }
        if let Some(batch) = self.batch.as_deref().filter(|b| !super::is_batch(b)) {
            return Err(format!("batch {batch:?} is not 1-64 of [A-Za-z0-9._-]"));
        }
        // The same rows on read as on record, so a hand-written line cannot add what the
        // recorder refuses.
        let download_row = DOWNLOAD_ROWS.contains(&self.row.as_str());
        match &self.kind {
            ReleaseKind::Download {
                signing,
                system,
                outcome,
            } => {
                if !download_row {
                    return Err(format!("{} is not a download-path row", self.row));
                }
                nonempty("signing", signing)?;
                nonempty("system", system)?;
                match (outcomes(&self.row), outcome) {
                    (None, None) => Ok(()),
                    (Some(allowed), Some(found)) if allowed.contains(&found.as_str()) => Ok(()),
                    (Some(allowed), _) => {
                        Err(format!("{} records one outcome of {allowed:?}", self.row))
                    }
                    (None, Some(_)) => Err(format!("{} records no outcome", self.row)),
                }
            }
            _ if download_row || self.row.starts_with("P18-") => Err(format!(
                "{} takes a download record, not a {}",
                self.row,
                self.kind.name()
            )),
            ReleaseKind::Receipt if !crate::release::macos::receipt_row(&self.row) => Err(format!(
                "P16-{} is not a row a USER release receipt stands for",
                self.row
            )),
            ReleaseKind::Waiver if !crate::release::macos::waivable(&self.row) => {
                Err(format!("P16-{} cannot be waived for a release", self.row))
            }
            _ => Ok(()),
        }
    }

    /// The record of kind `kind` in `value`; `None` for any other kind.
    pub fn parse(kind: &str, value: &Value, recorded_unix: u64) -> Result<Option<Self>, String> {
        let text =
            |key: &str| -> Result<String, String> { Ok(value.field(key)?.string()?.to_owned()) };
        let mut allowed = vec![
            "kind",
            "row",
            "candidate_receipt_sha256",
            "environment",
            "quote",
            "batch",
            "recorded_unix",
        ];
        if kind == "download" {
            allowed.extend(["signing", "system", "outcome"]);
        }
        if let (true, Value::Object(fields)) = (
            matches!(kind, "release-receipt" | "release-waiver" | "download"),
            value,
        ) && let Some(extra) = fields.keys().find(|key| !allowed.contains(&key.as_str()))
        {
            return Err(format!("a {kind} record has no field {extra}"));
        }
        let kind = match kind {
            "release-receipt" => ReleaseKind::Receipt,
            "release-waiver" => ReleaseKind::Waiver,
            "download" => ReleaseKind::Download {
                signing: text("signing")?,
                system: text("system")?,
                outcome: match value.field("outcome") {
                    Ok(member) => Some(member.string()?.to_owned()),
                    Err(_) => None,
                },
            },
            _ => return Ok(None),
        };
        let batch = match value.field("batch") {
            Ok(member) => Some(member.string()?.to_owned()),
            Err(_) => None,
        };
        Ok(Some(Self {
            kind,
            row: text("row")?,
            candidate: text("candidate_receipt_sha256")?.to_ascii_lowercase(),
            environment: text("environment")?,
            quote: text("quote")?,
            batch,
            recorded_unix,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::options::{Command, parse};
    use super::super::record::{Record, Revoked, consistent_with, read_all};
    use super::*;

    fn hex(c: char) -> String {
        std::iter::repeat_n(c, 64).collect()
    }

    fn record(kind: ReleaseKind, row: &str) -> ReleaseRecord {
        ReleaseRecord {
            kind,
            row: row.into(),
            candidate: hex('c'),
            environment: "USER desktop, recorded on macOS 27.2".into(),
            quote: "看过了".into(),
            batch: Some("rc1".into()),
            recorded_unix: 5,
        }
    }

    fn download(row: &str) -> ReleaseRecord {
        record(
            ReleaseKind::Download {
                signing: "AD_HOC".into(),
                system: "ProductName: macOS ProductVersion: 27.2".into(),
                outcome: outcomes(row).map(|o| o[0].to_owned()),
            },
            row,
        )
    }

    #[test]
    fn release_records_round_trip_through_the_ledger() {
        let records = [
            Record::Release(record(ReleaseKind::Receipt, "W01")),
            Record::Release(record(ReleaseKind::Waiver, "R01")),
            Record::Release(download("P18-D03")),
            Record::Revocation {
                row: "W01".into(),
                revoked: Revoked::Candidate(hex('c')),
                quote: "撤回".into(),
                recorded_unix: 6,
            },
        ];
        let text: String = records.iter().map(|r| format!("{}\n", r.line())).collect();
        assert_eq!(read_all(&text).unwrap(), records);
        // Digests are compared in lower case, as every other ledger digest.
        let upper = text.replacen(&hex('c'), &hex('C'), 1);
        assert_eq!(read_all(&upper).unwrap(), records);
    }

    #[test]
    fn a_release_record_must_fit_its_kind_and_name_a_candidate() {
        assert!(record(ReleaseKind::Receipt, "W01").validate().is_ok());
        let error = record(ReleaseKind::Receipt, "P18-D01")
            .validate()
            .unwrap_err();
        assert!(error.contains("download record"), "{error}");
        assert!(
            record(ReleaseKind::Waiver, "P18-D01").validate().is_err(),
            "the download path is not waivable through the ledger either"
        );
        let error = download("W01").validate().unwrap_err();
        assert!(error.contains("not a download-path row"), "{error}");
        let mut unsigned = download("P18-D01");
        unsigned.kind = ReleaseKind::Download {
            signing: " ".into(),
            system: "macOS".into(),
            outcome: None,
        };
        assert!(unsigned.validate().is_err());
        let mut short = record(ReleaseKind::Receipt, "W01");
        short.candidate = "c".repeat(63);
        assert!(short.validate().unwrap_err().contains("not a SHA-256"));
        let mut silent = record(ReleaseKind::Waiver, "R01");
        silent.quote = String::new();
        assert!(silent.validate().is_err());
    }

    /// Review S6: a line is read as strictly as it is recorded: no field beyond its kind's, and
    /// only the rows the recorder accepts.
    #[test]
    fn a_hand_written_release_line_cannot_add_what_the_recorder_refuses() {
        let receipt = Record::Release(record(ReleaseKind::Receipt, "W05")).line();
        assert!(read_all(&receipt).is_ok());
        let failed = receipt.replacen("\"row\"", "\"verdict\":\"FAIL\",\"row\"", 1);
        assert!(read_all(&failed).unwrap_err().contains("no field verdict"));
        let r03 = Record::Release(record(ReleaseKind::Waiver, "R03")).line();
        assert!(read_all(&r03).unwrap_err().contains("cannot be waived"));
        let gui = Record::Release(record(ReleaseKind::Receipt, "W02")).line();
        assert!(read_all(&gui).unwrap_err().contains("not a row"));
        let source = Record::Release(record(ReleaseKind::Waiver, "S01")).line();
        assert!(read_all(&source).is_err());
    }

    /// Decision (b): D04 and D05 carry one of their outcomes, every other download row none.
    #[test]
    fn only_d04_and_d05_record_an_outcome_and_only_one_of_theirs() {
        let with = |row: &str, outcome: Option<&str>| {
            record(
                ReleaseKind::Download {
                    signing: "AD_HOC".into(),
                    system: "ProductName: macOS ProductVersion: 27.2".into(),
                    outcome: outcome.map(str::to_owned),
                },
                row,
            )
            .validate()
        };
        assert!(with("P18-D04", Some("translocated-prompt-exit")).is_ok());
        assert!(with("P18-D05", Some("not-blocked")).is_ok());
        assert!(with("P18-D04", None).unwrap_err().contains("one outcome"));
        assert!(with("P18-D04", Some("blocked")).is_err(), "a D05 outcome");
        assert!(with("P18-D05", Some("looked fine")).is_err());
        assert!(
            with("P18-D01", Some("blocked"))
                .unwrap_err()
                .contains("no outcome")
        );
        assert!(with("P18-D01", None).is_ok());
    }

    /// Decision (c): a release receipt or waiver names the candidate the USER checked.
    #[test]
    fn release_records_name_the_candidate_checked() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let r = "1".repeat(40);
        let c = hex('C');
        assert!(
            parse(&args(&format!(
                "receipt --release-source {r} --rows W01 --quote=q"
            )))
            .is_err()
        );
        assert!(
            parse(&args(&format!(
                "waive --release-source {r} --rows R01 --quote=q"
            )))
            .is_err()
        );
        let receipt = parse(&args(&format!(
            "receipt --release-source {r} --candidate-sha256 {c} --rows P18-D04 --system s --outcome portable-start --quote=q"
        )))
        .unwrap();
        assert_eq!(receipt.candidate_sha256.as_deref(), Some(hex('c').as_str()));
        assert_eq!(receipt.outcome.as_deref(), Some("portable-start"));
        assert!(
            parse(&args(
                "receipt --app a.app --rows W01 --outcome blocked --quote=q"
            ))
            .is_err(),
            "--outcome belongs to release records"
        );
    }

    #[test]
    fn a_candidate_revocation_needs_a_release_record_of_its_row_before_it() {
        let held = [Record::Release(record(ReleaseKind::Waiver, "R01"))];
        let revoke = |row: &str, digest: char| Record::Revocation {
            row: row.into(),
            revoked: Revoked::Candidate(hex(digest)),
            quote: "撤回".into(),
            recorded_unix: 6,
        };
        assert!(consistent_with(&held, &revoke("R01", 'c')).is_ok());
        assert!(consistent_with(&held, &revoke("R02", 'c')).is_err());
        assert!(consistent_with(&held, &revoke("R01", 'd')).is_err());
        let error = consistent_with(&[], &revoke("P18-D01", 'c')).unwrap_err();
        assert!(error.contains("of P18-D01 names"), "{error}");
    }

    #[test]
    fn the_release_options_name_exactly_one_target() {
        let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let r = "1".repeat(40);
        let receipt = parse(&args(&format!(
            "receipt --release-source {r} --candidate-sha256 {} --rows W01,P18-D02 --system s.txt --quote=q",
            hex('c')
        )))
        .unwrap();
        assert_eq!(receipt.rows, ["W01", "P18-D02"]);
        assert_eq!(receipt.release.as_deref(), Some(r.as_str()));
        assert!(
            parse(&args(&format!(
                "receipt --app a.app --release-source {r} --rows W01 --quote=q"
            )))
            .is_err(),
            "one binding only"
        );
        assert!(
            parse(&args("receipt --app a.app --rows W01 --system s --quote=q")).is_err(),
            "--system belongs to release records"
        );
        let waive = parse(&args(&format!(
            "waive --release-source {r} --candidate-sha256 {} --rows R01 --quote=q --batch rc1",
            hex('c')
        )))
        .unwrap();
        assert_eq!(waive.command, Some(Command::Waive));
        assert!(parse(&args("waive --rows R01 --quote=q")).is_err());
        assert!(
            parse(&args(&format!(
                "waive --release-source {r} --rows R01 --app a.app --quote=q"
            )))
            .is_err()
        );
        assert!(
            parse(&args(&format!(
                "revoke --rows R01 --release-source {r} --quote=q"
            )))
            .is_ok()
        );
        assert!(
            parse(&args(&format!(
                "revoke --rows R01 --release-source {r} --app a.app --quote=q"
            )))
            .is_err()
        );
    }
}
