//! G5 exact-candidate case routing.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod compact;
mod presentation;
mod rendering;
mod shell;
mod support;

use std::path::Path;

use super::super::exact_desktop::CaseEvidence;

pub(super) fn g5_01(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    shell::run(repository, program)
}

pub(super) fn g5_02(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    compact::run(repository, program)
}

pub(super) fn g5_03(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    presentation::run(repository, program)
}

pub(super) fn g5_04(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    rendering::run(repository, program)
}
