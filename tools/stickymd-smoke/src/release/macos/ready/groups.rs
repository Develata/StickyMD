//! The ledger's results for one row on the candidate's bytes, identity by identity (plan 11
//! #macos-readiness-check): which groups of driver, VM image and VM system hold a clean PASS,
//! and whether any recorded a FAIL. How each row of the gate uses these results is `judge`'s.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use crate::mac_ledger::decide::NEVER_REUSED;
use crate::mac_ledger::record::{Built, Record, Revoked};

/// The ledger's verdict on `row` for the candidate's bytes.
pub(super) enum Ledger {
    /// The clean groups, and the latest PASS's evidence.
    Clean(Vec<Group>, String),
    Fail(String),
    None(String),
}

/// What a group shares once the bytes are the candidate's: driver, VM image and VM system
/// (plan 11 #macos-readiness-check). The product inputs are left out: equal bytes carry equal
/// inputs, and a record may lack them.
pub(super) type Group = (String, String, String);

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
/// rows, of `batch`, grouped by driver, image and system. A group is clean when it holds a PASS
/// of the row and no FAIL, NOT_TESTED or revoked PASS of it: neither run start nor import order
/// proves which came first (plan 11 #phase-16-macos-host), so any such record spoils the group.
pub(super) fn ledger(records: &[Record], row: &str, built: &Built, batch: Option<&str>) -> Ledger {
    let in_scope = |record: &&Record| match record {
        Record::Automated {
            row: r,
            built: b,
            batch: rb,
            ..
        } => {
            r == row
                && b == built
                && (!NEVER_REUSED.contains(&row) || (batch.is_some() && rb.as_deref() == batch))
        }
        _ => false,
    };
    let scoped: Vec<&Record> = records.iter().filter(in_scope).collect();
    let revoked = |digest: &str| {
        records.iter().any(|r| {
            matches!(r, Record::Revocation { row: rr, revoked: Revoked::Evidence(d), .. }
                if rr == row && d == digest)
        })
    };
    let mut clean: Vec<Group> = Vec::new();
    let mut latest = String::new();
    for record in &scoped {
        let Some(key) = group_of(record) else {
            continue;
        };
        if clean.contains(&key) {
            continue;
        }
        let group: Vec<&&Record> = scoped
            .iter()
            .filter(|r| group_of(r).as_ref() == Some(&key))
            .collect();
        let spoiled = group.iter().any(|r| {
            matches!(r, Record::Automated { verdict, evidence_sha256, .. }
                if verdict != "PASS" || revoked(evidence_sha256))
        });
        if !spoiled {
            if let Some(Record::Automated { evidence, .. }) = group.last().map(|r| **r) {
                latest = evidence.clone();
            }
            clean.push(key);
        }
    }
    if !clean.is_empty() {
        return Ledger::Clean(clean, latest);
    }
    if scoped
        .iter()
        .any(|r| matches!(r, Record::Automated { verdict, .. } if verdict == "FAIL"))
    {
        return Ledger::Fail(format!(
            "a FAIL of P16-{row} is recorded on the candidate's bytes"
        ));
    }
    let same_executable = records.iter().any(|r| {
        matches!(r, Record::Automated { row: rr, built: b, verdict, .. }
            if rr == row && verdict == "PASS" && b.executable == built.executable && b.bundle != built.bundle)
    });
    Ledger::None(if !scoped.is_empty() {
        format!(
            "every record of P16-{row} on the candidate's bytes is NOT_TESTED or revoked, or sits beside one"
        )
    } else if same_executable {
        format!(
            "a PASS of P16-{row} names the candidate's executable but another bundle; it does not count"
        )
    } else if NEVER_REUSED.contains(&row) && batch.is_none() {
        format!("P16-{row} counts only the batch named with --batch")
    } else {
        format!("no PASS of P16-{row} is recorded on the candidate's bytes")
    })
}
