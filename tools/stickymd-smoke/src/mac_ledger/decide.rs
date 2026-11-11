//! Pure planning over the ledger: which rows run, which reuse a recorded PASS, which manual rows
//! still need the USER for this app.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::record::{Identity, Record};

/// Manual rows never carried over by a receipt, as on Windows: each confirmation stands for one
/// sitting (the IME checks, App Translocation of a downloaded package).
pub(super) const NEVER_REUSED: [&str; 4] = ["I01", "I02", "I03", "R03"];

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Reuse {
    pub row: String,
    pub evidence: String,
    pub evidence_sha256: String,
    /// A FAIL or NOT_TESTED under the same identity was recorded after this PASS.
    pub later_non_pass: bool,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Plan {
    /// Executable rows without a PASS for this identity.
    pub run: Vec<String>,
    /// Executable rows whose latest PASS under this identity is reused.
    pub reused: Vec<Reuse>,
    /// Manual rows with a USER receipt for this app executable, with its environment.
    pub confirmed: Vec<(String, String)>,
    /// Manual rows without one, and the never-reused rows.
    pub unconfirmed: Vec<String>,
}

/// `executable` are the rows `mac-gui` can run; `manual` the matrix rows whose mode is Manual.
/// Only a PASS recorded under exactly `identity` is reused (an earlier success stays evidence of
/// itself, as for the Windows last-success receipts); a later non-PASS is reported with it.
pub(super) fn plan(
    records: &[Record],
    executable: &[&str],
    manual: &[String],
    identity: &Identity,
) -> Plan {
    let mut result = Plan::default();
    for &row in executable {
        let same = |r: &&Record| {
            matches!(r, Record::Automated { row: r_row, identity: r_identity, .. }
                if r_row == row && r_identity == identity)
        };
        let latest_pass = records.iter().enumerate().rev().find(|(_, r)| {
            same(r) && matches!(r, Record::Automated { verdict, .. } if verdict == "PASS")
        });
        match latest_pass {
            Some((
                at,
                Record::Automated {
                    evidence,
                    evidence_sha256,
                    ..
                },
            )) => result.reused.push(Reuse {
                row: row.to_owned(),
                evidence: evidence.clone(),
                evidence_sha256: evidence_sha256.clone(),
                later_non_pass: records[at + 1..].iter().any(|r| same(&r)),
            }),
            _ => result.run.push(row.to_owned()),
        }
    }
    for row in manual {
        let receipt = records.iter().rev().find_map(|r| match r {
            Record::Manual {
                row: r_row,
                app_executable,
                environment,
                ..
            } if r_row == row && *app_executable == identity.app_executable => {
                Some(environment.clone())
            }
            _ => None,
        });
        match receipt {
            Some(environment) if !NEVER_REUSED.contains(&row.as_str()) => {
                result.confirmed.push((row.clone(), environment));
            }
            _ => result.unconfirmed.push(row.clone()),
        }
    }
    result
}

/// An image digest stands for one VM system: across all the given records, each digest must
/// carry a single `sw_vers` text, or the digest no longer identifies the environment.
pub(super) fn check_system(records: &[Record]) -> Result<(), String> {
    let mut known: Vec<(&str, &str)> = Vec::new();
    for record in records {
        let Record::Automated { identity, .. } = record else {
            continue;
        };
        match known.iter().find(|(image, _)| *image == identity.image) {
            Some((image, system)) if *system != identity.system => {
                return Err(format!(
                    "image {image} carries two systems, {system:?} and {:?}; record a new digest",
                    identity.system
                ));
            }
            Some(_) => {}
            None => known.push((&identity.image, &identity.system)),
        }
    }
    Ok(())
}

/// The P16 rows whose mode is `Manual` in the Phase 16 matrix, in document order.
pub(super) fn manual_rows(matrix: &str) -> Result<Vec<String>, String> {
    let mut rows = Vec::new();
    for line in matrix.lines() {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(id) = cells.first().and_then(|c| c.strip_prefix("P16-")) else {
            continue;
        };
        if cells.len() != 5 {
            return Err(format!("P16-{id}: a matrix row needs five cells"));
        }
        if cells[2] == "Manual" {
            rows.push(id.to_owned());
        }
    }
    if rows.is_empty() {
        return Err("the Phase 16 matrix has no Manual rows".into());
    }
    Ok(rows)
}
