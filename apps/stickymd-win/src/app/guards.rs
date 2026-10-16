//! The shell facts the window reducer's guards are projected from. Which facts make a guard
//! (for example that a Search composition counts as composing) is decided here, once.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use super::StickyApp;
use crate::flow::window::state::WindowGuardSnapshot;

/// The shell state the guards read.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct GuardFacts {
    pub(super) window_focused: bool,
    pub(super) editor_composing: bool,
    pub(super) search_composing: bool,
    pub(super) move_resize_active: bool,
    pub(super) scrollbar_dragging: bool,
    pub(super) opacity_popup_open: bool,
    pub(super) search_open: bool,
    pub(super) export_in_flight: bool,
    pub(super) recovery_pending: bool,
    pub(super) conflict: bool,
    pub(super) paste_pending: bool,
    pub(super) asset_sync_in_flight: bool,
    pub(super) dirty: bool,
    pub(super) durability_required: bool,
    pub(super) note_save_in_flight: bool,
}

pub(super) const fn guard_snapshot(facts: GuardFacts) -> WindowGuardSnapshot {
    WindowGuardSnapshot {
        window_focused: facts.window_focused,
        ime_composing: facts.editor_composing || facts.search_composing,
        dragging: facts.move_resize_active || facts.scrollbar_dragging,
        popup_open: facts.opacity_popup_open || facts.search_open || facts.export_in_flight,
        conflict_or_recovery: facts.recovery_pending || facts.conflict,
        paste_pending: facts.paste_pending,
        asset_transaction_pending: facts.asset_sync_in_flight,
        note_save_required: facts.dirty || facts.durability_required,
        note_save_in_flight: facts.note_save_in_flight,
    }
}

impl StickyApp {
    pub(super) fn window_guards(&self) -> WindowGuardSnapshot {
        guard_snapshot(GuardFacts {
            window_focused: self.session.focused,
            editor_composing: self.session.is_composing(),
            search_composing: self.search.is_composing(),
            move_resize_active: self.move_resize_active,
            scrollbar_dragging: self.scrollbars.drag.is_some(),
            opacity_popup_open: self.controls.opacity_popup_open,
            search_open: self.search.open,
            export_in_flight: self.export_in_flight,
            recovery_pending: self.recovery.is_pending(),
            conflict: self.persistence.conflict().is_some(),
            paste_pending: self.asset_paste_pending,
            asset_sync_in_flight: self.asset_sync_in_flight,
            dirty: self.coordinator.view().dirty,
            durability_required: self.persistence.durability_required(),
            note_save_in_flight: self.persistence.note_save_in_flight(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each fact alone raises exactly the guard it belongs to.
    #[test]
    fn each_fact_maps_to_its_guard() {
        type Set = fn(&mut GuardFacts);
        type Expect = fn(&mut WindowGuardSnapshot);
        let cases: [(Set, Expect); 15] = [
            (|f| f.window_focused = true, |g| g.window_focused = true),
            (|f| f.editor_composing = true, |g| g.ime_composing = true),
            (|f| f.search_composing = true, |g| g.ime_composing = true),
            (|f| f.move_resize_active = true, |g| g.dragging = true),
            (|f| f.scrollbar_dragging = true, |g| g.dragging = true),
            (|f| f.opacity_popup_open = true, |g| g.popup_open = true),
            (|f| f.search_open = true, |g| g.popup_open = true),
            (|f| f.export_in_flight = true, |g| g.popup_open = true),
            (
                |f| f.recovery_pending = true,
                |g| g.conflict_or_recovery = true,
            ),
            (|f| f.conflict = true, |g| g.conflict_or_recovery = true),
            (|f| f.paste_pending = true, |g| g.paste_pending = true),
            (
                |f| f.asset_sync_in_flight = true,
                |g| g.asset_transaction_pending = true,
            ),
            (|f| f.dirty = true, |g| g.note_save_required = true),
            (
                |f| f.durability_required = true,
                |g| g.note_save_required = true,
            ),
            (
                |f| f.note_save_in_flight = true,
                |g| g.note_save_in_flight = true,
            ),
        ];
        assert_eq!(
            guard_snapshot(GuardFacts::default()),
            WindowGuardSnapshot::default()
        );
        for (set, expect) in cases {
            let mut facts = GuardFacts::default();
            set(&mut facts);
            let mut expected = WindowGuardSnapshot::default();
            expect(&mut expected);
            assert_eq!(guard_snapshot(facts), expected, "{facts:?}");
        }
    }

    /// A composition in the editor or in the Search field alone keeps a docked window from
    /// auto-collapsing (plan 09: never collapse during IME composition).
    #[test]
    fn a_composition_in_either_field_blocks_auto_collapse() {
        for facts in [
            GuardFacts {
                editor_composing: true,
                ..GuardFacts::default()
            },
            GuardFacts {
                search_composing: true,
                ..GuardFacts::default()
            },
        ] {
            assert!(guard_snapshot(facts).blocks_auto_collapse(), "{facts:?}");
        }
    }
}
