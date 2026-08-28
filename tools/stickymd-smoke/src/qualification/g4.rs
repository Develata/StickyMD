//! Exact-candidate G4 tray, docking, editor, conversion, and identity qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cases;

use std::path::Path;

use crate::cli::G4Case;

use super::exact_desktop::{self, ExactCase, ExactGroup};

const DEFAULT_RECEIPT: &str = "dist/evidence/g4-exact-qualification.json";
const CASES: &[ExactCase] = &[
    ExactCase {
        id: "G4-01",
        operation: cases::g4_01,
    },
    ExactCase {
        id: "G4-02",
        operation: cases::g4_02,
    },
    ExactCase {
        id: "G4-03",
        operation: cases::g4_03,
    },
    ExactCase {
        id: "G4-04",
        operation: cases::g4_04,
    },
    ExactCase {
        id: "G4-05",
        operation: cases::g4_05,
    },
    ExactCase {
        id: "G4-06",
        operation: cases::g4_06,
    },
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
            cases: CASES,
            selected_case: selected_case.map(G4Case::as_str),
        },
    )
}
