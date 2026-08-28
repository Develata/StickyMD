//! Exact-candidate G3 clipboard, export, recovery, and asset-safety qualification.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cases;

use std::path::Path;

use crate::cli::G3Case;

use super::exact_desktop::{self, ExactCase, ExactGroup};

const DEFAULT_RECEIPT: &str = "dist/evidence/g3-exact-qualification.json";
const CASES: &[ExactCase] = &[
    ExactCase {
        id: "G3-01",
        operation: cases::g3_01,
    },
    ExactCase {
        id: "G3-02",
        operation: cases::g3_02,
    },
    ExactCase {
        id: "G3-03",
        operation: cases::g3_03,
    },
    ExactCase {
        id: "G3-04",
        operation: cases::g3_04,
    },
    ExactCase {
        id: "G3-05",
        operation: cases::g3_05,
    },
];

pub(super) fn run(
    repository: &Path,
    requested_zip: Option<&Path>,
    evidence_file: Option<&Path>,
    selected_case: Option<G3Case>,
) -> Result<(), String> {
    exact_desktop::run(
        repository,
        requested_zip,
        evidence_file,
        ExactGroup {
            name: "G3",
            default_receipt: DEFAULT_RECEIPT,
            cases: CASES,
            selected_case: selected_case.map(G3Case::as_str),
        },
    )
}
