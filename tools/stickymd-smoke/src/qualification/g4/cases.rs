//! G4 exact-candidate case routing.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod dock;
mod editor;
mod identity;
mod ime;
mod tray;

use std::path::Path;

use super::super::exact_desktop::CaseEvidence;

pub(super) fn g4_01(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    tray::g4_01(repository, program)?;
    Ok(CaseEvidence::default())
}

pub(super) fn g4_02(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    dock::g4_02(repository, program)?;
    Ok(CaseEvidence::default())
}

pub(super) fn g4_03(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    editor::g4_03(repository, program)?;
    Ok(CaseEvidence::default())
}

pub(super) fn g4_04(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    editor::g4_04(repository, program)?;
    Ok(CaseEvidence::default())
}

pub(super) fn g4_05(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    identity::g4_05(repository, program)?;
    Ok(CaseEvidence::default())
}

pub(super) fn g4_06(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    ime::g4_06(repository, program)?;
    Ok(CaseEvidence::default())
}
