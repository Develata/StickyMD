//! Pure planning over the ledger: which rows run, which reuse a recorded PASS, which manual rows
//! still need the USER for this app.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::record::{Built, Identity, Record, Revoked};

/// Rows never carried over, as on Windows: a receipt, a run or a judgement stands for one
/// acceptance batch, because the result depends on system state (the IME checks, the press-and-hold
/// setting, App Translocation of a downloaded package). Every batch runs them again.
pub(super) const NEVER_REUSED: [&str; 4] = ["I01", "I02", "I03", "R03"];

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Reuse {
    pub row: String,
    pub evidence: String,
    pub evidence_sha256: String,
    /// The PASS was recorded for other app bytes built from the same inputs; reported for audit.
    pub other_bytes: bool,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Plan {
    /// Executable rows without a reusable PASS for this identity.
    pub run: Vec<String>,
    /// Rows in `run` although a PASS exists for this identity, with the reason.
    pub rerun: Vec<(String, &'static str)>,
    /// Executable rows whose latest PASS under this identity is reused.
    pub reused: Vec<Reuse>,
    /// Manual rows with a USER receipt for these product inputs, with its environment.
    pub confirmed: Vec<(String, String)>,
    /// Manual rows without one, and the never-reused rows.
    pub unconfirmed: Vec<String>,
}

/// `executable` are the rows `mac-gui` can run; `manual` the matrix rows the USER judges.
/// Only the latest PASS recorded under exactly `identity` (product inputs, driver, image, system)
/// is reused, and not when a FAIL or NOT_TESTED under that identity came after it, when the USER
/// revoked any PASS of the row under that identity, or when the row is never reused. A receipt
/// holds until a revocation for its row and inputs is recorded after it. `built` is the app at
/// hand, compared for audit only. An app without inputs reuses nothing and has no confirmed
/// manual rows.
pub(super) fn plan(
    records: &[Record],
    executable: &[&str],
    manual: &[String],
    identity: &Identity,
    built: &Built,
) -> Plan {
    let mut result = Plan::default();
    for &row in executable {
        let same = |r: &&Record| {
            matches!(r, Record::Automated { row: r_row, identity: r_identity, .. }
                if r_row == row && r_identity.reusable_for(identity))
        };
        let passes: Vec<(usize, &Record)> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                same(r) && matches!(r, Record::Automated { verdict, .. } if verdict == "PASS")
            })
            .collect();
        let revoked = passes.iter().any(|(_, r)| {
            matches!(r, Record::Automated { evidence_sha256, .. }
                if records.iter().any(|v| matches!(v, Record::Revocation {
                    row: v_row, revoked: Revoked::Evidence(digest), ..
                } if v_row == row && digest == evidence_sha256)))
        });
        // Err(None): no PASS to reuse; Err(Some(reason)): a PASS exists but must not be reused.
        let decision = match passes.last() {
            None => Err(None),
            Some(_) if NEVER_REUSED.contains(&row) => {
                Err(Some("never reused: every acceptance batch runs it"))
            }
            Some(_) if revoked => Err(Some(
                "the USER revoked a PASS of this row for this identity",
            )),
            Some((at, _)) if records[at + 1..].iter().any(|r| same(&r)) => Err(Some(
                "a FAIL or NOT_TESTED was recorded after its latest PASS",
            )),
            Some((
                _,
                Record::Automated {
                    evidence,
                    evidence_sha256,
                    built: recorded,
                    ..
                },
            )) => Ok(Reuse {
                row: row.to_owned(),
                evidence: evidence.clone(),
                evidence_sha256: evidence_sha256.clone(),
                other_bytes: recorded != built,
            }),
            Some(_) => unreachable!("only automated records are collected as passes"),
        };
        match decision {
            Ok(reuse) => result.reused.push(reuse),
            Err(reason) => {
                result.run.push(row.to_owned());
                if let Some(reason) = reason {
                    result.rerun.push((row.to_owned(), reason));
                }
            }
        }
    }
    for row in manual {
        let receipt = records
            .iter()
            .enumerate()
            .rev()
            .find_map(|(at, r)| match r {
                Record::Manual {
                    row: r_row,
                    inputs: Some(inputs),
                    environment,
                    ..
                } if r_row == row && identity.inputs.as_ref() == Some(inputs) => {
                    Some((at, environment.clone()))
                }
                _ => None,
            });
        let revoked_after = |at: usize| {
            records[at + 1..].iter().any(|r| {
                matches!(r, Record::Revocation { row: r_row, revoked: Revoked::Inputs(inputs), .. }
                    if r_row == row && identity.inputs.as_ref() == Some(inputs))
            })
        };
        match receipt {
            Some((at, environment))
                if !NEVER_REUSED.contains(&row.as_str()) && !revoked_after(at) =>
            {
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

/// The P16 rows the USER judges, in document order: mode `Manual` (by hand) and `Review` (from
/// screenshots or video). An `Agent` row is refused until its judge records are implemented, so
/// it is never silently left out of the plan.
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
        match cells[2] {
            "Manual" | "Review" => rows.push(id.to_owned()),
            "Agent" => {
                return Err(format!(
                    "P16-{id} is an Agent row; mac-ledger has no judge records yet"
                ));
            }
            _ => {}
        }
    }
    if rows.is_empty() {
        return Err("the Phase 16 matrix has no Manual rows".into());
    }
    Ok(rows)
}
