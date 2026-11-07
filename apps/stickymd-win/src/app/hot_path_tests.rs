//! Phase 15 hot-path gate: full-text snapshots and background builds under a fixed typing
//! fixture, counted through the real windowless shell. The expected counts are the baseline
//! fixed before any code left this crate, so a migration that adds work fails here.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#phase-15-migration-gates

use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::*;
use crate::app::headless_tests::fixture;
use crate::app::input::ShellKey;

const TYPED: usize = 200;

fn letter() -> ShellKey<'static> {
    ShellKey {
        physical_key: PhysicalKey::Code(KeyCode::KeyA),
        state: ElementState::Pressed,
        repeat: false,
        ime_owned: false,
        text: Some("a"),
    }
}

/// Types the fixture text in `mode`, then lets every deadline pass once. Returns the snapshots
/// taken while typing, the snapshots taken while settling, and the note saves submitted.
fn type_and_settle(mode: ViewMode) -> (u64, u64, u64) {
    let mut app = fixture("start\n");
    app.config.update(|c| c.view_mode = mode).unwrap();
    let before = app.coordinator.snapshots_taken();
    for _ in 0..TYPED {
        app.apply_key(letter(), false);
    }
    assert_eq!(app.coordinator.view().text.len(), "start\n".len() + TYPED);
    let typing = app.coordinator.snapshots_taken() - before;
    let settled = app.coordinator.snapshots_taken();
    app.settle_event_pass(u64::MAX / 2, None);
    let settling = app.coordinator.snapshots_taken() - settled;
    (typing, settling, app.worker.metrics().note_submitted)
}

/// One burst settles into one save. In Source and Split a keystroke takes no snapshot, and Split
/// adds one debounced build. Pure Preview builds immediately (plan 06), so it takes one snapshot
/// per keystroke and the worker mailbox coalesces those builds.
#[test]
fn phase15_a_typing_burst_stays_at_the_baseline_snapshot_and_build_counts() {
    assert_eq!(type_and_settle(ViewMode::Source), (0, 1, 1));
    assert_eq!(type_and_settle(ViewMode::Split), (0, 2, 1));
    assert_eq!(type_and_settle(ViewMode::Preview), (TYPED as u64, 1, 1));
}
