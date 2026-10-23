//! Exact-candidate G5 shell, compact-layout, presentation, and rendering qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cases;
#[cfg(test)]
mod preflight;

use std::path::Path;

use crate::cli::G5Case;

use super::exact_checks;
use super::exact_desktop::{self, CaseOperation, ExactGroup};

const DEFAULT_RECEIPT: &str = "dist/evidence/g5-exact-qualification.json";
/// Runs `exact_checks::G5` in order.
const OPERATIONS: [CaseOperation; exact_checks::G5.len()] =
    [cases::g5_01, cases::g5_02, cases::g5_03, cases::g5_04];

pub(super) fn run(
    repository: &Path,
    requested_zip: Option<&Path>,
    evidence_file: Option<&Path>,
    selected_case: Option<G5Case>,
) -> Result<(), String> {
    exact_desktop::run(
        repository,
        requested_zip,
        evidence_file,
        ExactGroup {
            name: "G5",
            default_receipt: DEFAULT_RECEIPT,
            cases: exact_checks::G5,
            operations: &OPERATIONS,
            selected_case: selected_case.map(G5Case::as_str),
            injects_input: true,
        },
    )
}
