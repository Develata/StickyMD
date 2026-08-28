//! G4 exact-candidate case routing.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod dock;
mod editor;
mod identity;
mod tray;

use std::path::Path;

pub(super) fn g4_01(repository: &Path, program: &Path) -> Result<(), String> {
    tray::g4_01(repository, program)
}

pub(super) fn g4_02(repository: &Path, program: &Path) -> Result<(), String> {
    dock::g4_02(repository, program)
}

pub(super) fn g4_03(repository: &Path, program: &Path) -> Result<(), String> {
    editor::g4_03(repository, program)
}

pub(super) fn g4_04(repository: &Path, program: &Path) -> Result<(), String> {
    editor::g4_04(repository, program)
}

pub(super) fn g4_05(repository: &Path, program: &Path) -> Result<(), String> {
    identity::g4_05(repository, program)
}
