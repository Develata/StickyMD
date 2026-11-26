//! The candidate as the readiness check reads it (plan 11 #macos-readiness-check, steps 3 and
//! 5): the resolver's verdict on the staging and its receipt, the promotion-check record on R,
//! and the whole state of both, which must be the same when the verdicts are printed.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::judge::{self, Read};
use crate::release::macos::promotion_check::{self, Check};
use std::path::Path;

/// One reading of the candidate.
pub(super) struct Snapshot {
    pub candidate: Read<judge::Candidate>,
    pub promotion: Read<Check>,
    /// The whole state read (absent, not resolving, or resolving to which receipt; and the
    /// record's bytes), compared between the first and the last reading.
    pub state: String,
}

pub(super) fn read(root: &Path, release: &str) -> Snapshot {
    let candidate = match crate::release::macos::resolve::receipt_present(root) {
        Ok(true) => match crate::release::macos::resolve::resolve(root, release) {
            Ok(resolved) => Read::Present(judge::Candidate {
                receipt_sha256: resolved.receipt_sha256,
                zip_sha256: resolved.zip_sha256,
                built: crate::mac_ledger::record::Built {
                    executable: resolved.executable_sha256,
                    bundle: resolved.bundle_sha256,
                },
                signing: resolved.signing,
            }),
            Err(error) => Read::Invalid(format!("the candidate does not resolve: {error}")),
        },
        Ok(false) => Read::Missing("no macOS candidate has been promoted".to_owned()),
        Err(error) => Read::Invalid(error),
    };
    // Read after the receipt the resolver checked: a promotion writes the record after it.
    let found = promotion_check::load(root);
    let record = match &found {
        Ok(None) => "no record".to_owned(),
        Ok(Some(bytes)) => crate::integrity::sha256_bytes(bytes).map_or_else(
            |_| "record unhashable".to_owned(),
            |d| format!("record {d}"),
        ),
        Err(_) => "record unreadable".to_owned(),
    };
    let state = match &candidate {
        Read::Present(candidate) => format!("present {}", candidate.receipt_sha256),
        Read::Missing(_) => "missing".to_owned(),
        Read::Invalid(_) => "invalid".to_owned(),
    };
    Snapshot {
        candidate,
        promotion: judge::read_promotion(found),
        state: format!("{state}; {record}"),
    }
}
