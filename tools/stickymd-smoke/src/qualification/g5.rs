//! Exact-candidate G5 shell, compact-layout, presentation, and rendering qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cases;

use std::path::Path;

use crate::cli::G5Case;

use super::exact_desktop::{self, ExactCase, ExactGroup};

const DEFAULT_RECEIPT: &str = "dist/evidence/g5-exact-qualification.json";
const CASES: &[ExactCase] = &[
    ExactCase {
        id: "G5-01",
        operation: cases::g5_01,
    },
    ExactCase {
        id: "G5-02",
        operation: cases::g5_02,
    },
    ExactCase {
        id: "G5-03",
        operation: cases::g5_03,
    },
    ExactCase {
        id: "G5-04",
        operation: cases::g5_04,
    },
];

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
            cases: CASES,
            selected_case: selected_case.map(G5Case::as_str),
        },
    )
}
