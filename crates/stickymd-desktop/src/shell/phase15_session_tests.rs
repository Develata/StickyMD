//! Direct pins of the decisions Phase 15 C4 moves out of the shell, through the real shell and
//! written before anything moved: the save on focus loss, the watcher during a recovery choice,
//! every gate on an edit, the event pass and the clamp of a committed placement.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use crate::testing::HostCall;
use stickymd_app::execution::IoCompletion;
use stickymd_app::flow::window::geometry::WindowPlacement;
use stickymd_app::flow::window::state::{
    DockEdge, LifecycleState, QuitStage, StableVisibility, WindowEffect, WindowIntent,
};
use stickymd_app::ports::{AssetReconcileReport, PersistMode};
use stickymd_core::Generation;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Ime};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

use crate::shell::headless_tests::{HeadlessApp, fixture, recovering_at_start, window_flow};
use crate::shell::headless_tests::{offer_recovery, saves, type_letter};
use crate::shell::input::ShellKey;
use crate::shell::phase15_receipt_tests::saved;
use crate::shell::receipt_tests::recovering;
use stickymd_app::config::{ContentZoomPercent, ViewMode};

fn edited() -> HeadlessApp {
    let mut app = fixture("start");
    type_letter(&mut app);
    app
}

fn saving(app: &HeadlessApp) -> String {
    format!(
        "正在保存 generation {}…",
        app.app.coordinator.view().generation.value()
    )
}

#[test]
fn losing_focus_saves_a_dirty_note_and_nothing_else_does() {
    let mut app = edited();
    let before = saves(&app);
    app.apply_focus_change(false);
    assert_eq!(saves(&app), before + 1);
    assert!(app.app.persistence.note_save_in_flight());
    assert_eq!(app.diagnostic, Some(saving(&app)));

    // A clean note has nothing to save.
    let mut app = fixture("start");
    app.apply_focus_change(false);
    assert_eq!(saves(&app), 0);
    assert!(!app.app.persistence.note_save_in_flight());

    // Gaining focus saves nothing, however dirty the note is.
    let mut app = edited();
    let before = saves(&app);
    app.apply_focus_change(true);
    assert_eq!(saves(&app), before);
    assert!(!app.app.persistence.note_save_in_flight());
}

#[test]
fn no_watcher_starts_while_a_recovery_choice_waits() {
    assert!(fixture("start").started_watcher);

    let app = recovering_at_start();
    assert!(app.app.recovery.is_pending());
    assert!(!app.started_watcher);
    assert!(!app.host.watching);

    let mut app = recovering(None);
    app.start_watcher();
    assert!(!app.host.watching);
}

/// Which gates a row closes.
#[derive(Debug, Clone, Copy)]
struct Gates {
    recovery: bool,
    preview: bool,
    quitting: bool,
    reconcile: bool,
    input_off: bool,
}

fn every_gate_combination() -> impl Iterator<Item = Gates> {
    (0_u8..32).map(|mask| {
        let on = |bit: u8| mask & (1 << bit) != 0;
        Gates {
            recovery: on(0),
            preview: on(1),
            quitting: on(2),
            reconcile: on(3),
            input_off: on(4),
        }
    })
}

/// Closes the gates. A quit comes first: a waiting recovery choice would block it.
fn close(app: &mut HeadlessApp, gates: Gates) {
    if gates.quitting {
        let guards = app.window_guards();
        app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
        let lifecycle = app.app.window_flow.as_ref().unwrap().state().lifecycle();
        assert!(lifecycle.is_quitting());
    }
    if gates.recovery {
        offer_recovery(app, Some("start"), None);
    }
    app.preview_focused = gates.preview;
    app.app.assets.reconcile_pending = gates.reconcile;
    app.shell_input_enabled = !gates.input_off;
}

/// An IME commit edits only with every gate open: this is the IME gate of `handle_ime`.
#[test]
fn an_ime_commit_edits_the_note_only_with_every_gate_open() {
    for gates in every_gate_combination() {
        let mut app = fixture("start");
        close(&mut app, gates);
        app.handle_ime(Ime::Commit("x".into()));
        let allowed = !(gates.recovery
            || gates.preview
            || gates.quitting
            || gates.reconcile
            || gates.input_off);
        assert_eq!(
            app.app.coordinator.view().text != "start",
            allowed,
            "{gates:?}"
        );
    }
}

/// The keyboard gate of `apply_key` does not read Preview focus: a window-wide shortcut works
/// from Preview, and the Preview key handler comes later.
#[test]
fn a_keyboard_save_passes_preview_focus_and_every_other_gate_stops_it() {
    for gates in every_gate_combination() {
        let mut app = edited();
        close(&mut app, gates);
        let before = saves(&app);
        app.modifiers = ModifiersState::CONTROL;
        app.apply_key(
            ShellKey {
                physical_key: PhysicalKey::Code(KeyCode::KeyS),
                state: ElementState::Pressed,
                repeat: false,
                ime_owned: false,
                text: None,
            },
            false,
        );
        let allowed = !(gates.recovery || gates.quitting || gates.reconcile || gates.input_off);
        assert_eq!(saves(&app) != before, allowed, "{gates:?}");
    }
}

#[test]
fn a_shell_without_a_window_flow_takes_no_edit() {
    let mut app = fixture("start");
    app.app.window_flow = None;
    app.handle_ime(Ime::Commit("x".into()));
    assert_eq!(app.app.coordinator.view().text, "start");
}

#[test]
fn the_event_pass_flushes_the_zoom_only_at_its_deadline() {
    let mut app = fixture("start");
    app.app.config_persistence_allowed = true;
    app.app
        .config
        .update(|config| config.content_zoom_percent = ContentZoomPercent::new_clamped(150))
        .unwrap();
    app.app.zoom_config_deadline = Some(100);

    app.settle_event_pass(99);
    assert_eq!(app.app.zoom_config_deadline, Some(100));
    assert!(!app.app.config.is_saving());

    app.settle_event_pass(100);
    assert_eq!(app.app.zoom_config_deadline, None);
    assert!(app.app.config.is_saving());
}

/// An autosave, a debounced Preview build and a dock timer all fall due in one pass.
#[test]
fn the_event_pass_runs_the_note_the_preview_and_the_window_ticks() {
    let mut app = fixture("start");
    app.headless_view = Some((PhysicalSize::new(900, 600), 1.0));
    app.app
        .config
        .update(|config| config.view_mode = ViewMode::Split)
        .unwrap();
    let docked = StableVisibility::DockedExpanded(DockEdge::Left);
    app.app.window_flow = Some(window_flow(docked));
    app.apply_focus_change(false);
    type_letter(&mut app);
    let deadlines = [
        app.app.persistence.autosave_deadline().unwrap(),
        app.app.preview_flow.deadline().unwrap(),
        flow_deadline(&app),
    ];
    std::mem::take(&mut app.applied_window_effects);
    let saved = saves(&app);

    app.settle_event_pass(deadlines.into_iter().max().unwrap());

    assert_eq!(saves(&app), saved + 1);
    let worker = app.app.preview_worker.as_ref().unwrap();
    assert_eq!(worker.metrics().unwrap().submitted, 1);
    assert!(!app.applied_window_effects.is_empty());
}

fn flow_deadline(app: &HeadlessApp) -> u64 {
    let flow = app.app.window_flow.as_ref().unwrap();
    flow.state().next_deadline_ms().unwrap()
}

fn placed(width_dip: f64, height_dip: f64) -> (u32, u32) {
    let mut app = fixture("start");
    let placement = WindowPlacement {
        width_dip,
        height_dip,
        monitor_identity: None,
        dock_offset_ratio: 0.0,
        floating_x_ratio: 0.5,
        floating_y_ratio: 0.5,
    };
    app.apply_window_effect(WindowEffect::CommitPlacement {
        placement,
        dock_edge: None,
    });
    let window = &app.app.config.current().window;
    (window.width_dip, window.height_dip)
}

#[test]
fn a_committed_placement_stays_inside_the_window_limits() {
    assert_eq!(placed(1.0, 1.0), (220, 120));
    assert_eq!(placed(219.4, 119.4), (220, 120));
    assert_eq!(placed(220.0, 120.0), (220, 120));
    assert_eq!(placed(900.4, 600.6), (900, 601));
    assert_eq!(placed(16_384.0, 16_384.0), (16_384, 16_384));
    assert_eq!(placed(16_384.6, 16_385.0), (16_384, 16_384));
    assert_eq!(placed(99_999.0, 99_999.0), (16_384, 16_384));
}

/// The tray goes with the effect, and the event loop ends only when the callback does: once.
#[test]
fn exit_comes_after_the_tray_is_destroyed() {
    let mut app = fixture("start");
    app.apply_window_effect(WindowEffect::ExitProcess);
    assert_eq!(app.host.take_calls(), [HostCall::TrayDestroyed]);
    let mut exits = 0;
    app.finish_callback(|| exits += 1);
    assert_eq!(exits, 1);

    app.finish_callback(|| panic!("an exit is consumed once"));
}

/// Every barrier of a quit holds the exit back until its own receipt arrives.
#[test]
fn exit_does_not_appear_while_a_barrier_is_unsettled() {
    let mut app = edited();
    app.app.config_persistence_allowed = true;
    app.app.config.update(|config| config.opacity = 50).unwrap();

    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
    assert_eq!(stage(&app), QuitStage::AwaitingNoteSave);
    assert!(!app.exit_pending);

    let receipt = saved(&app, PersistMode::Guarded { expected: None });
    app.handle_io_completion(receipt);
    assert_eq!(stage(&app), QuitStage::AwaitingAssetGc);
    assert!(!app.exit_pending);

    let request_id = app.app.assets.quit_sync_request_id.unwrap();
    app.handle_io_completion(IoCompletion::AssetSync {
        request_id,
        generation: Generation::initial(),
        result: Ok(AssetReconcileReport::default()),
    });
    assert_eq!(stage(&app), QuitStage::AwaitingConfig);
    assert!(app.app.config.is_saving());
    assert!(!app.exit_pending);

    let revision = app.app.config.revision();
    app.handle_io_completion(IoCompletion::Config {
        revision,
        result: Ok(()),
    });
    assert_eq!(stage(&app), QuitStage::ReadyToExit);
    assert!(app.exit_pending);
}

fn stage(app: &HeadlessApp) -> QuitStage {
    match app.app.window_flow.as_ref().unwrap().state().lifecycle() {
        LifecycleState::Quitting(stage) => stage,
        LifecycleState::Running => panic!("the quit ended"),
    }
}
