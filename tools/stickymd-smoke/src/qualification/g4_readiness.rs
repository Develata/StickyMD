//! G4 exact-candidate readiness projection.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::exact_readiness;
use super::module_ledger::ModuleId;
use super::receipt::Candidate;

pub(super) const G4_RECEIPT: &str = "dist/evidence/g4-exact-qualification.json";
const EXPECTED_CASES: [&str; 6] = ["G4-01", "G4-02", "G4-03", "G4-04", "G4-05", "G4-06"];

pub(super) fn check(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) -> bool {
    exact_readiness::check(
        root,
        candidate,
        ModuleId::G4,
        "G4",
        G4_RECEIPT,
        &EXPECTED_CASES,
        blockers,
    )
}
