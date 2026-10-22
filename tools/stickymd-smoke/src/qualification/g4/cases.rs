//! G4 exact-candidate case routing.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod diagnostics;
mod dock;
mod editor;
mod escape;
mod identity;
mod ime;
mod opacity;
mod scroll;
mod search;
mod tray;
mod tray_topmost;

use std::path::Path;

use super::super::exact_desktop::{CaseEvidence, Checks};

pub(super) fn g4_01(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| tray::g4_01(repository, program, checks))
}

pub(super) fn g4_02(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| {
        dock::g4_02(repository, program, checks)?;
        scroll::run(program, checks)
    })
}

pub(super) fn g4_03(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| {
        editor::g4_03(repository, program, checks)?;
        search::run(repository, program, checks)
    })
}

pub(super) fn g4_04(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| editor::g4_04(repository, program, checks))
}

pub(super) fn g4_05(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| identity::g4_05(repository, program, checks))
}

pub(super) fn g4_06(repository: &Path, program: &Path) -> Result<CaseEvidence, String> {
    checked(|checks| ime::g4_06(repository, program, checks))
}

/// Runs `case` with a fresh recorder; its evidence is the checks it recorded.
fn checked(case: impl FnOnce(&mut Checks) -> Result<(), String>) -> Result<CaseEvidence, String> {
    let mut checks = Checks::default();
    case(&mut checks)?;
    Ok(CaseEvidence {
        checks,
        ..CaseEvidence::default()
    })
}
