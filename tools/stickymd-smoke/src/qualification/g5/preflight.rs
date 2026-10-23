//! Opt-in G5 checks reuse the existing isolated, hash-bound local preflight runner.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::super::{exact_checks, g4::preflight::run};

#[test]
#[ignore = "exclusive Windows desktop; explicitly supplied EXE; no formal candidate receipt"]
fn acceptance_automation_shell_local_preflight() {
    run(&exact_checks::G5[0], super::cases::g5_01);
}

#[test]
#[ignore = "exclusive Windows desktop; explicitly supplied EXE; no formal candidate receipt"]
fn acceptance_automation_compact_local_preflight() {
    run(&exact_checks::G5[1], super::cases::g5_02);
}

#[test]
#[ignore = "exclusive Windows desktop; explicitly supplied EXE; no formal candidate receipt"]
fn acceptance_automation_presentation_local_preflight() {
    run(&exact_checks::G5[2], super::cases::g5_03);
}

#[test]
#[ignore = "exclusive Windows desktop; explicitly supplied EXE; no formal candidate receipt"]
fn acceptance_automation_rendering_local_preflight() {
    run(&exact_checks::G5[3], super::cases::g5_04);
}
