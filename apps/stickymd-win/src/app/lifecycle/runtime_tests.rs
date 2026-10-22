//! Actual event-pass settlement and canonical persistence barriers, without windows.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

use crate::app::headless_tests::fixture;
use crate::flow::SaveTrigger;
use crate::flow::window::state::{QuitBarrier, WindowEffect};
use crate::instruction::AppIntent;
use stickymd_core::{EditKind, Selection};

#[test]
fn runtime_event_pass_reads_all_barriers_once_after_scheduling() {
    for dirty in [false, true] {
        for durability in [false, true] {
            for in_flight in [false, true] {
                let mut app = fixture("saved text");
                if dirty {
                    let generation = app.coordinator.view().generation;
                    app.coordinator
                        .dispatch(AppIntent::Edit {
                            expected_generation: generation,
                            selection: Selection::caret(0),
                            inserted: "new".into(),
                            kind: EditKind::Typing,
                            timestamp_ms: 1,
                        })
                        .unwrap();
                }
                if durability {
                    app.persistence
                        .note_save_submitted(SaveTrigger::RecreateMissing);
                    app.persistence.note_save_finished(false);
                }
                if in_flight {
                    app.persistence.note_save_submitted(SaveTrigger::Debounce);
                }
                app.quit_notice
                    .on_window_effect(&WindowEffect::QuitCancelled(QuitBarrier::NoteSave));
                for pass in 1..=3 {
                    app.settle_event_pass(pass as u64, None);
                    assert_eq!(app.quit_notice.settle_count, pass);
                    assert_eq!(app.quit_notice.active(), dirty || durability || in_flight);
                }
            }
        }
    }

    // A queued autosave may outlive an undo back to a clean document. Its tick
    // raises the in-flight barrier in this very pass, before the notice settles.
    let mut app = fixture("clean again");
    let generation = app.coordinator.view().generation;
    app.persistence.on_document_changed(0, generation);
    let deadline = app.persistence.autosave_deadline().unwrap();
    app.quit_notice
        .on_window_effect(&WindowEffect::QuitCancelled(QuitBarrier::NoteSave));
    assert!(!app.window_guards().requires_note_barrier());
    app.settle_event_pass(deadline, None);
    assert_eq!(app.worker.metrics().note_submitted, 1);
    assert!(app.persistence.note_save_in_flight());
    assert!(
        app.quit_notice.active(),
        "settling before the tick would incorrectly clear it"
    );
    assert_eq!(app.quit_notice.settle_count, 1);
    app.persistence.note_save_finished(true);
    app.settle_event_pass(deadline + 1, None);
    assert!(!app.quit_notice.active());
    assert_eq!(app.quit_notice.settle_count, 2);
}
