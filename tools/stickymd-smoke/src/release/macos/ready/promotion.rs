//! The promotion-check record as the gate reads and judges it (plan 11 #macos-readiness-check,
//! step 3; M5 gate review B2): the package holds only when the promotion's own re-checks ran on
//! R for the receipt the resolver read. Pure, so the bad samples run on every host.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::judge::{Candidate, Read};
use crate::release::macos::promotion_check::{self, Check};

/// Whether the promotion's own re-checks ran on R for the receipt the resolver read (plan 11
/// #macos-readiness-check, step 3): the record must name both, or why not.
pub(super) fn promoted_on_r(
    promotion: &Read<&Check>,
    release: &str,
    candidate: &Candidate,
) -> Result<(), String> {
    let path = promotion_check::PATH;
    match promotion {
        Read::Present(check) if check.release_source != release => Err(format!(
            "the promotion-check record {path} says the promotion was checked on {}, not on R: promote the candidate again on R",
            check.release_source
        )),
        Read::Present(check) if check.candidate_receipt_sha256 != candidate.receipt_sha256 => {
            Err(format!(
                "the promotion-check record {path} names another candidate receipt ({}): promote the candidate again on R",
                check.candidate_receipt_sha256
            ))
        }
        Read::Present(_) => Ok(()),
        Read::Missing(reason) | Read::Invalid(reason) => Err(reason.clone()),
    }
}

/// The promotion-check record as read: absent, or its bytes. Every reason names the record, so
/// the package line says what to redo.
pub(super) fn read(found: Option<&[u8]>) -> Read<Check> {
    let path = promotion_check::PATH;
    match found {
        None => Read::Missing(format!(
            "no promotion-check record {path}: promote the candidate again on R"
        )),
        Some(bytes) => match promotion_check::parse(bytes) {
            Ok(check) => Read::Present(check),
            Err(error) => Read::Invalid(format!(
                "the promotion-check record {path} is unreadable ({error}): promote the candidate again on R"
            )),
        },
    }
}
