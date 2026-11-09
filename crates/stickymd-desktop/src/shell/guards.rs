//! The shell facts the window reducer's guards are projected from. Which facts make a guard is
//! `stickymd_app::interaction::rules::guard_snapshot`.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use super::DesktopShell;
use crate::host::Host;
use stickymd_app::flow::window::state::WindowGuardSnapshot;
use stickymd_app::session::ViewFacts;

impl<H: Host> DesktopShell<H> {
    pub(super) fn view_facts(&self) -> ViewFacts {
        ViewFacts {
            move_resize_active: self.move_resize_active,
            scrollbar_dragging: self.scrollbars.drag.is_some()
                || self.preview_horizontal.drag.is_some(),
            opacity_popup_open: self.controls.opacity_popup_open,
        }
    }

    pub(super) fn window_guards(&self) -> WindowGuardSnapshot {
        self.app.guards(self.view_facts())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn runtime_guards_read_both_compositions_and_focus_loss_cancels_them() {
        use crate::shell::headless_tests::fixture;
        use stickymd_app::interaction::ImeSignal;
        for editor in [false, true] {
            for search in [false, true] {
                let mut app = fixture("canonical text");
                let snapshot = app.app.coordinator.snapshot();
                app.app.editor.focused = true;
                if editor {
                    app.app
                        .editor
                        .handle_ime(ImeSignal::Enabled, snapshot.generation, 1);
                    app.app.editor.handle_ime(
                        ImeSignal::Preedit {
                            text: "nihao".into(),
                            cursor: Some(0..5),
                        },
                        snapshot.generation,
                        2,
                    );
                }
                if search {
                    app.app.search.open = true;
                    app.app
                        .search
                        .insert("canonical", &snapshot.text, snapshot.generation);
                    app.app.search.set_preedit("zhong".into(), Some((0, 5)));
                }
                assert_eq!(app.window_guards().ime_composing, editor || search);
                assert!(app.window_guards().window_focused);
                app.apply_focus_change(false);
                assert!(!app.app.editor.is_composing());
                assert!(
                    !app.app.search.is_composing(),
                    "Search composition survived focus loss"
                );
                assert!(!app.window_guards().ime_composing);
                assert!(!app.window_guards().window_focused);
                assert_eq!(app.app.search.open, search);
                assert_eq!(app.app.search.query, if search { "canonical" } else { "" });
                assert_eq!(app.app.coordinator.snapshot(), snapshot);
                assert!(!app.app.coordinator.view().dirty);
            }
        }
    }
}
