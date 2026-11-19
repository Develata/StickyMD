//! Pure planning over the ledger: which rows run, which reuse a recorded PASS, which rows still
//! need the USER for this app.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::record::{Built, Identity, Record, Revoked};

/// Rows never carried over, as on Windows: a receipt or a run stands for one acceptance batch,
/// because the result depends on system state (the IME checks, the press-and-hold setting, App
/// Translocation of a downloaded package). Only a result of the batch being planned counts.
pub(super) const NEVER_REUSED: [&str; 4] = ["I01", "I02", "I03", "R03"];

/// Rows of the Phase 16 matrix judged on Windows (`docs/acceptance-cases/phase-16.md`, V05: the
/// Windows desktop regression of the shared render code); no macOS receipt stands for them.
pub(super) const WINDOWS_ONLY: [&str; 1] = ["V05"];

/// The Phase 16 rows by how they are judged (plan 11 #phase-16-macos-host, the acceptance ladder).
#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct Rows {
    /// Mode `Manual`, Windows-only rows left out: the USER judges by hand.
    pub manual: Vec<String>,
    /// Mode `Review`: the USER judges screenshots or video of a run.
    pub review: Vec<String>,
    /// Mode `Agent`: two agents judge the evidence of a run.
    pub agent: Vec<String>,
}

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
    /// Manual rows without one.
    pub unconfirmed: Vec<String>,
}

/// Whether a record of a never-reused row belongs to the batch being planned.
fn in_batch(row: &str, record_batch: Option<&String>, batch: Option<&str>) -> bool {
    !NEVER_REUSED.contains(&row) || (batch.is_some() && record_batch.map(String::as_str) == batch)
}

/// `executable` are the rows `mac-gui` can run; `manual` the rows the USER judges by hand.
/// For an executable row only its latest PASS recorded under exactly `identity` (product inputs,
/// driver, image, system) is a candidate. It is not reused when any FAIL or NOT_TESTED of the row
/// is recorded under that identity, before or after it (wall-clock run starts cannot prove which
/// came first, so the order is not relied on), or when the USER revoked it. A receipt holds until
/// a revocation for its row and inputs is recorded after it. For the
/// never-reused rows only records of `batch` count, and without a batch nothing does. `built`
/// is the app at hand, compared for audit only. An app without inputs reuses nothing and has no
/// confirmed manual rows.
pub(super) fn plan(
    records: &[Record],
    executable: &[&str],
    manual: &[String],
    identity: &Identity,
    built: &Built,
    batch: Option<&str>,
) -> Plan {
    let mut result = Plan::default();
    for &row in executable {
        let same = |r: &&Record| {
            matches!(r, Record::Automated { row: r_row, identity: r_identity, batch: b, .. }
                if r_row == row && r_identity.reusable_for(identity) && in_batch(row, b.as_ref(), batch))
        };
        let latest = records
            .iter()
            .enumerate()
            .rev()
            .find_map(|(at, r)| match r {
                Record::Automated {
                    verdict,
                    evidence,
                    evidence_sha256,
                    built: recorded,
                    ..
                } if same(&r) && verdict == "PASS" => {
                    Some((at, evidence, evidence_sha256, recorded))
                }
                _ => None,
            });
        // Err(None): no PASS to reuse; Err(Some(reason)): a PASS exists but must not be reused.
        let decision = match latest {
            None if NEVER_REUSED.contains(&row) && batch.is_none() => Err(Some(
                "never reused: plan with --batch to count an acceptance batch",
            )),
            None => Err(None),
            Some((_, _, digest, _)) if revoked(records, row, digest) => {
                Err(Some("the USER revoked its latest PASS for this identity"))
            }
            Some(_)
                if records.iter().any(|r| {
                    same(&r) && matches!(r, Record::Automated { verdict, .. } if verdict != "PASS")
                }) =>
            {
                Err(Some(
                    "a FAIL or NOT_TESTED is recorded for this identity; only a clean record is reused",
                ))
            }
            Some((_, evidence, digest, recorded)) => Ok(Reuse {
                row: row.to_owned(),
                evidence: evidence.clone(),
                evidence_sha256: digest.clone(),
                other_bytes: recorded != built,
            }),
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
                    batch: b,
                    ..
                } if r_row == row
                    && identity.inputs.as_ref() == Some(inputs)
                    && in_batch(row, b.as_ref(), batch) =>
                {
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
            Some((at, environment)) if !revoked_after(at) => {
                result.confirmed.push((row.clone(), environment));
            }
            _ => result.unconfirmed.push(row.clone()),
        }
    }
    result
}

/// Whether the USER revoked the PASS of `row` recorded from the evidence `digest`.
fn revoked(records: &[Record], row: &str, digest: &str) -> bool {
    records.iter().any(|r| {
        matches!(r, Record::Revocation { row: r_row, revoked: Revoked::Evidence(d), .. }
            if r_row == row && d == digest)
    })
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

/// The P16 rows of the Phase 16 matrix by mode, in document order, read by the same parser
/// governance uses (`governance::phase16_modes`), so a row the governance check refuses cannot be
/// planned or receive a receipt here. Windows-only rows are not macOS manual rows.
pub(super) fn matrix_rows(matrix: &str) -> Result<Rows, String> {
    let mut rows = Rows::default();
    let mut seen: Vec<String> = Vec::new();
    for (id, mode) in crate::governance::phase16_modes(matrix)? {
        let Some(id) = id.strip_prefix("P16-") else {
            continue;
        };
        if seen.iter().any(|s| s == id) {
            return Err(format!("P16-{id} appears twice in the matrix"));
        }
        seen.push(id.to_owned());
        match mode.as_str() {
            "Manual" if WINDOWS_ONLY.contains(&id) => {}
            "Manual" => rows.manual.push(id.to_owned()),
            "Review" => rows.review.push(id.to_owned()),
            "Agent" => rows.agent.push(id.to_owned()),
            _ => {}
        }
    }
    if rows.manual.is_empty() && rows.review.is_empty() && rows.agent.is_empty() {
        return Err("the Phase 16 matrix has no rows the USER or agents judge".into());
    }
    Ok(rows)
}
