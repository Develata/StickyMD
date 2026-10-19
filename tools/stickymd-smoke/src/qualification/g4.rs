//! Exact-candidate G4 tray, docking, editor, conversion, and identity qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cases;

use std::path::Path;

use crate::cli::G4Case;

use super::exact_checks;
use super::exact_desktop::{self, CaseOperation, ExactGroup};

const DEFAULT_RECEIPT: &str = "dist/evidence/g4-exact-qualification.json";
/// Runs `exact_checks::G4` in order.
const OPERATIONS: [CaseOperation; exact_checks::G4.len()] = [
    cases::g4_01,
    cases::g4_02,
    cases::g4_03,
    cases::g4_04,
    cases::g4_05,
    cases::g4_06,
];

pub(super) fn run(
    repository: &Path,
    requested_zip: Option<&Path>,
    evidence_file: Option<&Path>,
    selected_case: Option<G4Case>,
) -> Result<(), String> {
    exact_desktop::run(
        repository,
        requested_zip,
        evidence_file,
        ExactGroup {
            name: "G4",
            default_receipt: DEFAULT_RECEIPT,
            cases: exact_checks::G4,
            operations: &OPERATIONS,
            selected_case: selected_case.map(G4Case::as_str),
            injects_input: true,
        },
    )
}
