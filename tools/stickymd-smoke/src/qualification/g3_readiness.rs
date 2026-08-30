//! G3 exact-candidate readiness projection.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::exact_readiness;
use super::module_ledger::ModuleId;
use super::receipt::Candidate;

pub(super) const G3_RECEIPT: &str = "dist/evidence/g3-exact-qualification.json";
const EXPECTED_CASES: [&str; 5] = ["G3-01", "G3-02", "G3-03", "G3-04", "G3-05"];

pub(super) fn check(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) -> bool {
    exact_readiness::check(
        root,
        candidate,
        ModuleId::G3,
        "G3",
        G3_RECEIPT,
        &EXPECTED_CASES,
        blockers,
    )
}
