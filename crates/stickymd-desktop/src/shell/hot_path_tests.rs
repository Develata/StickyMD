//! Phase 15 hot-path gate: full-text snapshots and background builds under a fixed typing
//! fixture, counted through the real windowless shell with a viewport, so Preview jobs reach the
//! real worker. The expected counts were measured at the baseline `b846e83`, before any code left
//! this crate, so a migration that adds work fails here.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#phase-15-migration-gates

use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime};
use winit::keyboard::{KeyCode, PhysicalKey};

use stickymd_app::ports::PreviewCompletion;
use stickymd_core::Generation;
use stickymd_render::preview::PreviewPipelineError;

use super::*;
use crate::shell::headless_tests::{HeadlessApp, fixture};
use crate::shell::input::ShellKey;

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
/// taken while typing, the snapshots taken while settling, the note saves submitted and the
/// Preview jobs the worker received.
fn type_and_settle(mode: ViewMode) -> (u64, u64, u64, u64) {
    let mut app = fixture("start\n");
    app.headless_view = Some((PhysicalSize::new(900, 600), 1.0));
    app.app.config.update(|c| c.view_mode = mode).unwrap();
    let before = app.app.coordinator.snapshots_taken();
    for _ in 0..TYPED {
        app.apply_key(letter(), false);
    }
    assert_eq!(
        app.app.coordinator.view().text.len(),
        "start\n".len() + TYPED
    );
    let typing = app.app.coordinator.snapshots_taken() - before;
    let settled = app.app.coordinator.snapshots_taken();
    app.settle_event_pass(u64::MAX / 2);
    let settling = app.app.coordinator.snapshots_taken() - settled;
    let preview_jobs = app
        .app
        .preview_worker
        .as_ref()
        .map_or(0, |worker| worker.metrics().unwrap().submitted);
    (
        typing,
        settling,
        app.app.worker.metrics().note_submitted,
        preview_jobs,
    )
}

/// One burst settles into one save. In Source and Split a keystroke takes no snapshot, and Split
/// adds one debounced build. Pure Preview builds immediately (plan 06), so it takes one snapshot
/// and submits one job per keystroke, and the worker mailbox coalesces those builds.
#[test]
fn phase15_a_typing_burst_stays_at_the_baseline_snapshot_and_build_counts() {
    assert_eq!(type_and_settle(ViewMode::Source), (0, 1, 1, 0));
    assert_eq!(type_and_settle(ViewMode::Split), (0, 2, 1, 1));
    assert_eq!(
        type_and_settle(ViewMode::Preview),
        (TYPED as u64, 1, 1, TYPED as u64)
    );
}

fn failed_result(generation: Generation, viewport: PreviewViewport) -> PreviewCompletion {
    PreviewCompletion {
        generation,
        viewport,
        result: Err(PreviewPipelineError::NoDocument),
    }
}

/// The Preview jobs the worker receives when each keystroke in pure Preview is followed by three
/// results: one for the text before it (asks for a build of the current text), one for the current
/// text at another viewport (asks for a relayout) and one for the current text and viewport (asks
/// for nothing). The results go to the real completion handler in a fixed order, so the count does
/// not depend on the worker's timing.
fn receipt_driven_preview_jobs() -> u64 {
    let mut app = fixture("start\n");
    app.headless_view = Some((PhysicalSize::new(900, 600), 1.0));
    app.app
        .config
        .update(|c| c.view_mode = ViewMode::Preview)
        .unwrap();
    for _ in 0..TYPED {
        let before = app.app.coordinator.view().generation;
        app.apply_key(letter(), false);
        let current = app.app.coordinator.view().generation;
        let viewport = app.preview_viewport().unwrap();
        let mut other = viewport.clone();
        other.width_px += 1;
        app.handle_preview_completion(failed_result(before, viewport.clone()));
        app.handle_preview_completion(failed_result(current, other));
        app.handle_preview_completion(failed_result(current, viewport));
    }
    app.settle_event_pass(u64::MAX / 2);
    app.app
        .preview_worker
        .as_ref()
        .map_or(0, |worker| worker.metrics().unwrap().submitted)
}

/// The rebuilds and relayouts that Preview results trigger stay at the baseline: the same
/// fixture on `b846e83`, with the viewport and worker-count patch the typing burst used, gives the
/// same count (task file, D record).
#[test]
fn phase15_d_receipt_driven_preview_jobs_stay_at_the_baseline() {
    assert_eq!(receipt_driven_preview_jobs(), 3 * TYPED as u64);
}

const SEARCHED: usize = 50;
const TICKED: usize = 50;

/// What a scenario costs: full-text snapshots, note saves and Preview jobs submitted.
fn costs(app: &HeadlessApp, snapshots_before: u64) -> (u64, u64, u64) {
    (
        app.app.coordinator.snapshots_taken() - snapshots_before,
        app.app.worker.metrics().note_submitted,
        app.app
            .preview_worker
            .as_ref()
            .map_or(0, |worker| worker.metrics().unwrap().submitted),
    )
}

fn split_fixture() -> HeadlessApp {
    let mut app = fixture("start\n");
    app.headless_view = Some((PhysicalSize::new(900, 600), 1.0));
    app.app
        .config
        .update(|c| c.view_mode = ViewMode::Split)
        .unwrap();
    app
}

/// Each character composed and committed through the IME, then every deadline once.
fn ime_burst() -> (u64, u64, u64) {
    let mut app = split_fixture();
    let before = app.app.coordinator.snapshots_taken();
    for _ in 0..TYPED {
        app.handle_ime(Ime::Preedit("a".into(), Some((1, 1))));
        app.handle_ime(Ime::Commit("a".into()));
    }
    assert_eq!(
        app.app.coordinator.view().text.len(),
        "start\n".len() + TYPED
    );
    app.settle_event_pass(u64::MAX / 2);
    costs(&app, before)
}

/// Typing into an open Search field: the note does not change.
fn search_burst() -> (u64, u64, u64) {
    let mut app = split_fixture();
    app.open_search(false);
    let before = app.app.coordinator.snapshots_taken();
    for _ in 0..SEARCHED {
        app.apply_key(letter(), false);
    }
    assert_eq!(app.app.coordinator.view().text, "start\n");
    app.settle_event_pass(u64::MAX / 2);
    costs(&app, before)
}

/// Every deadline passes after each keystroke, so each one settles on its own.
fn ticked_typing() -> (u64, u64, u64) {
    let mut app = split_fixture();
    let before = app.app.coordinator.snapshots_taken();
    for _ in 0..TICKED {
        app.apply_key(letter(), false);
        let now = app.app.now_ms() + 10_000;
        app.settle_event_pass(now);
    }
    costs(&app, before)
}

/// IME commits, Search typing and typing with a tick after every key stay at the counts the
/// same fixtures give on `b846e83` (task file, D record).
#[test]
fn phase15_d_ime_search_and_ticked_typing_stay_at_the_baseline() {
    assert_eq!(ime_burst(), (2, 1, 1));
    assert_eq!(search_burst(), (0, 0, 0));
    assert_eq!(
        ticked_typing(),
        (2 * TICKED as u64, TICKED as u64, TICKED as u64)
    );
}
