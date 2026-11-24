//! The ledger's results for rows on the candidate's bytes, identity by identity (plan 11
//! #macos-readiness-check): for each group of driver, VM image and VM system, whether every row
//! asked about holds a clean PASS there, or one of them failed there. How each row of the gate
//! uses these results is `judge`'s.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use crate::mac_ledger::decide::NEVER_REUSED;
use crate::mac_ledger::record::{Built, Record, Revoked, image_attributable};

/// The ledger's verdict on one or more rows for the candidate's bytes.
pub(super) enum Ledger {
    /// Some group holds a clean PASS of every row; the first such group's latest PASS evidence
    /// of the first row.
    Clean(String),
    Fail(String),
    None(String),
}

/// What a group shares once the bytes are the candidate's: driver, VM image and VM system
/// (plan 11 #macos-readiness-check). The product inputs are left out: equal bytes carry equal
/// inputs, and a record may lack them.
pub(super) type Group = (String, String, String);

/// Where results count: the ledger, the candidate's bytes and the batch being judged; for R03,
/// the candidate ZIP its counted PASS must have opened. The ZIP does not split groups: a FAIL,
/// NOT_TESTED or revoked PASS through another ZIP, or through none, still spoils its group.
#[derive(Clone, Copy)]
pub(super) struct Scope<'a> {
    pub records: &'a [Record],
    pub built: &'a Built,
    pub batch: Option<&'a str>,
    pub zip: Option<&'a str>,
    /// The SHA-256 of R's `mac-gui` driver (`macos-ready --driver`, checked against the driver the
    /// run on R built), or why R's driver is not known: only a group run by it can hold a PASS; a
    /// FAIL counts whatever the driver (USER 2026-11-24; review round 4, BLOCKER 1).
    pub driver: Result<&'a str, &'a str>,
}

/// One row's standing in one group.
enum Status {
    /// Only PASS records, none revoked; the latest one's evidence.
    Clean(String),
    /// At least one FAIL.
    Failed,
    /// Some NOT_TESTED or revoked PASS, and no FAIL.
    Incomplete,
    /// Only PASS records, but none opened the ZIP the scope requires.
    OtherZip,
}

fn group_of(record: &Record) -> Option<Group> {
    match record {
        Record::Automated { identity, .. } => Some((
            identity.driver.clone(),
            identity.image.clone(),
            identity.system.clone(),
        )),
        _ => None,
    }
}

/// Records of `row` on exactly the candidate's bytes (both digests) and, for the never-reused
/// rows, of the batch.
fn scoped<'a>(scope: &Scope<'a>, row: &str) -> Vec<&'a Record> {
    scope
        .records
        .iter()
        .filter(|record| match record {
            Record::Automated {
                row: r,
                built: b,
                batch: rb,
                ..
            } => {
                r == row
                    && b == scope.built
                    && (!NEVER_REUSED.contains(&row)
                        || (scope.batch.is_some() && rb.as_deref() == scope.batch))
            }
            _ => false,
        })
        .collect()
}

/// `row`'s standing in each group, in the order the groups first appear. Neither run start nor
/// import order proves which came first (plan 11 #phase-16-macos-host), so any FAIL, NOT_TESTED
/// or revoked PASS spoils the group whatever its place.
fn statuses(scope: &Scope<'_>, row: &str) -> Vec<(Group, Status)> {
    let records = scoped(scope, row);
    let revoked = |digest: &str| {
        scope.records.iter().any(|r| {
            matches!(r, Record::Revocation { row: rr, revoked: Revoked::Evidence(d), .. }
                if rr == row && d == digest)
        })
    };
    let mut out: Vec<(Group, Status)> = Vec::new();
    for record in &records {
        let Some(key) = group_of(record) else {
            continue;
        };
        if out.iter().any(|(seen, _)| *seen == key) {
            continue;
        }
        let group: Vec<&&Record> = records
            .iter()
            .filter(|r| group_of(r).as_ref() == Some(&key))
            .collect();
        let status = if group
            .iter()
            .any(|r| matches!(r, Record::Automated { verdict, .. } if verdict == "FAIL"))
        {
            Status::Failed
        } else if group.iter().any(|r| {
            matches!(r, Record::Automated { verdict, evidence_sha256, .. }
                if verdict != "PASS" || revoked(evidence_sha256))
        }) {
            Status::Incomplete
        } else {
            let counted = group.iter().rev().find_map(|r| match **r {
                // For R03, a PASS that opened the candidate ZIP with "nothing written" read
                // through the write watch (review round 3, B1).
                Record::Automated { evidence, zip, .. }
                    if scope.zip.is_none_or(|wanted| {
                        zip.as_ref()
                            .is_some_and(|z| z.sha256 == wanted && z.write_watch)
                    }) =>
                {
                    Some(evidence.clone())
                }
                _ => None,
            });
            match counted {
                Some(evidence) => Status::Clean(evidence),
                None => Status::OtherZip,
            }
        };
        out.push((key, status));
    }
    out
}

/// A FAIL of `row` recorded on the candidate's bytes in the batch, under any identity.
pub(super) fn failure(scope: &Scope<'_>, row: &str) -> Option<String> {
    scoped(scope, row)
        .iter()
        .any(|r| matches!(r, Record::Automated { verdict, .. } if verdict == "FAIL"))
        .then(|| format!("a FAIL of P16-{row} is recorded on the candidate's bytes"))
}

/// The joint verdict on `rows`, judged inside each group first: a group counts when every row
/// holds a clean PASS in it; with no such group, a group in which any row failed makes the
/// verdict FAIL; otherwise the evidence is missing.
pub(super) fn joint(scope: &Scope<'_>, rows: &[String]) -> Ledger {
    // A record whose VM image names no digest (an old line's tag) cannot be placed in or apart
    // from any group, so no group of these rows is shown clean while it stands; a FAIL still
    // counts (review round 3, B5).
    let unknown = rows.iter().find_map(|row| {
        scoped(scope, row)
            .into_iter()
            .find_map(|record| match record {
                Record::Automated { identity, .. } if !image_attributable(&identity.image) => {
                    Some((row, identity.image.clone()))
                }
                _ => None,
            })
    });
    let per_row: Vec<Vec<(Group, Status)>> = rows.iter().map(|row| statuses(scope, row)).collect();
    let mut keys: Vec<&Group> = Vec::new();
    for statuses in &per_row {
        for (key, _) in statuses {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    let status = |row: usize, key: &Group| per_row[row].iter().find(|(k, _)| k == key);
    let mut clean = Vec::new();
    let mut evidence = String::new();
    let mut failed: Option<&String> = None;
    for key in keys {
        let all_clean = scope.driver == Ok(key.0.as_str())
            && (0..rows.len()).all(|i| matches!(status(i, key), Some((_, Status::Clean(_)))));
        if all_clean {
            if clean.is_empty()
                && let Some((_, Status::Clean(found))) = status(0, key)
            {
                evidence = found.clone();
            }
            clean.push(key.clone());
        } else if failed.is_none() {
            failed = (0..rows.len())
                .find(|&i| matches!(status(i, key), Some((_, Status::Failed))))
                .map(|i| &rows[i]);
        }
    }
    if !clean.is_empty() && unknown.is_none() {
        return Ledger::Clean(evidence);
    }
    if let Some(row) = failed {
        return Ledger::Fail(format!(
            "a FAIL of P16-{row} is recorded on the candidate's bytes, and no identity holds a clean run of every row it needs"
        ));
    }
    if let Some((row, image)) = unknown {
        return Ledger::None(format!(
            "a record of P16-{row} on the candidate's bytes names the VM image {image:?}, not a digest: its identity is unknown, so no group can be shown clean"
        ));
    }
    let lacking = rows.iter().zip(&per_row).find(|(_, statuses)| {
        !statuses.iter().any(|(key, status)| {
            scope.driver == Ok(key.0.as_str()) && matches!(status, Status::Clean(_))
        })
    });
    Ledger::None(match lacking {
        Some((row, _)) => why_none(scope, row),
        None => format!(
            "{} have no clean identity in common",
            rows.iter()
                .map(|row| format!("P16-{row}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

/// Why `row` has no clean group.
fn why_none(scope: &Scope<'_>, row: &str) -> String {
    let same_executable = scope.records.iter().any(|r| {
        matches!(r, Record::Automated { row: rr, built: b, verdict, .. }
            if rr == row && verdict == "PASS" && b.executable == scope.built.executable && b.bundle != scope.built.bundle)
    });
    let statuses = statuses(scope, row);
    let other_zip = statuses.iter().any(|(key, status)| {
        scope.driver == Ok(key.0.as_str()) && matches!(status, Status::OtherZip)
    });
    let other_driver = statuses.iter().any(|(key, status)| {
        scope.driver != Ok(key.0.as_str()) && matches!(status, Status::Clean(_) | Status::OtherZip)
    });
    if let Err(reason) = scope.driver {
        format!("no run of P16-{row} counts as R's driver's: {reason}")
    } else if let (true, Some(zip)) = (other_zip, scope.zip) {
        format!(
            "no clean group of P16-{row} on the candidate's bytes holds a PASS that opened the candidate ZIP {zip} under the write watch (a ZIP zipped from the .app names none; a PASS recorded without the watch, as an older driver's, does not count)"
        )
    } else if other_driver {
        format!(
            "a clean run of P16-{row} on the candidate's bytes exists only under another driver than R's ({})",
            scope.driver.unwrap_or_default()
        )
    } else if !scoped(scope, row).is_empty() {
        format!(
            "every record of P16-{row} on the candidate's bytes is NOT_TESTED or revoked, or sits beside one"
        )
    } else if same_executable {
        format!(
            "a PASS of P16-{row} names the candidate's executable but another bundle; it does not count"
        )
    } else if NEVER_REUSED.contains(&row) && scope.batch.is_none() {
        format!("P16-{row} counts only the batch named with --batch")
    } else {
        format!("no PASS of P16-{row} is recorded on the candidate's bytes")
    }
}
