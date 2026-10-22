//! The window title: one writer, the plan 09 priority, and the quit-cancelled notice.
//!
//! The title is a projection of state owned elsewhere (recovery, the quit notice, the external
//! conflict, the dirty flag and a formula-error hover); `window_title` is the only place that
//! turns those facts into text, so no caller can show a lower-priority state over a higher one.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

use crate::flow::window::state::{QuitBarrier, WindowEffect};

pub(super) const PLAIN: &str = "StickyMD";
pub(super) const DIRTY: &str = "StickyMD *";
pub(super) const RECOVERY: &str = "StickyMD — 恢复选择未完成";
pub(super) const QUIT_CANCELLED: &str = "StickyMD * — 退出已取消：保存失败";
pub(super) const CONFLICT: &str = "StickyMD * — 外部修改冲突";

/// The facts the title projects.
#[derive(Clone, Copy, Debug)]
pub(super) struct TitleFacts<'a> {
    pub(super) recovery_pending: bool,
    pub(super) quit_cancelled: bool,
    pub(super) conflict: bool,
    pub(super) dirty: bool,
    /// The formula error under the pointer in Preview, if any.
    pub(super) formula_error: Option<&'a str>,
}

/// The title for `facts`, by priority: recovery → quit cancelled by a failed save → conflict →
/// dirty → formula-error hover → plain.
pub(super) fn window_title(facts: TitleFacts<'_>) -> std::borrow::Cow<'static, str> {
    use std::borrow::Cow::{Borrowed, Owned};
    if facts.recovery_pending {
        Borrowed(RECOVERY)
    } else if facts.quit_cancelled {
        Borrowed(QUIT_CANCELLED)
    } else if facts.conflict {
        Borrowed(CONFLICT)
    } else if facts.dirty {
        Borrowed(DIRTY)
    } else if let Some(detail) = facts.formula_error {
        Owned(format!("StickyMD — 公式错误：{detail}"))
    } else {
        Borrowed(PLAIN)
    }
}

/// Whether the window is in the "quit cancelled: save failed" state.
///
/// Invariant: set only by a quit that a failed canonical-note save cancelled; any other cancel
/// reason (a guard, an unsettled paste) neither sets nor clears it. Cleared once a complete
/// main-thread event pass ends with the quit barrier no longer requiring the note
/// (`dirty || durability_required || note_save_in_flight` false).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct QuitCancelNotice {
    active: bool,
    #[cfg(test)]
    pub(super) settle_count: usize,
}

impl QuitCancelNotice {
    pub(super) const fn active(self) -> bool {
        self.active
    }

    /// Applies one window effect, the only way the notice is established: a quit cancelled by
    /// a failed canonical-note save establishes it; every other effect, a quit a guard blocked
    /// included, leaves it alone. Returns whether the notice changed.
    pub(super) fn on_window_effect(&mut self, effect: &WindowEffect) -> bool {
        match *effect {
            WindowEffect::QuitCancelled(barrier) => self.on_quit_cancelled(barrier),
            _ => false,
        }
    }

    /// Records a cancelled quit; returns whether the notice changed.
    fn on_quit_cancelled(&mut self, barrier: QuitBarrier) -> bool {
        let establish = barrier == QuitBarrier::NoteSave && !self.active;
        self.active |= establish;
        establish
    }

    /// Runs after a complete main-thread event pass.
    pub(super) fn settle(&mut self, note_barrier_required: bool) {
        #[cfg(test)]
        {
            self.settle_count += 1;
        }
        self.active &= note_barrier_required;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    fn facts() -> TitleFacts<'static> {
        TitleFacts {
            recovery_pending: false,
            quit_cancelled: false,
            conflict: false,
            dirty: false,
            formula_error: None,
        }
    }

    /// All 32 combinations of the five facts: the highest present fact names the title, in plan
    /// 09 order. The expected titles are spelled out here, not taken from the product's
    /// constants, so a changed literal fails (the quit-cancelled one is the plan's).
    #[test]
    fn each_state_has_its_title_and_higher_states_win() {
        for mask in 0_u8..32 {
            let on = |bit: u8| mask & (1 << bit) != 0;
            let facts = TitleFacts {
                recovery_pending: on(0),
                quit_cancelled: on(1),
                conflict: on(2),
                dirty: on(3),
                formula_error: on(4).then_some("x"),
            };
            let expected = [
                "StickyMD — 恢复选择未完成",
                "StickyMD * — 退出已取消：保存失败",
                "StickyMD * — 外部修改冲突",
                "StickyMD *",
                "StickyMD — 公式错误：x",
            ]
            .into_iter()
            .enumerate()
            .find_map(|(bit, title)| on(bit as u8).then_some(title))
            .unwrap_or("StickyMD");
            assert_eq!(window_title(facts), expected, "{facts:?}");
        }
        assert_eq!(window_title(facts()), "StickyMD");
    }

    #[test]
    fn a_formula_error_never_hides_unsaved_text() {
        let hover = TitleFacts {
            dirty: true,
            formula_error: Some("missing }"),
            ..facts()
        };
        assert_eq!(window_title(hover), DIRTY);
    }

    /// `update_window_title` is the product's only native title write; another `set_title`
    /// would bypass the priority. A lexical guard over the product sources (comments included,
    /// which only makes it stricter).
    #[test]
    fn the_title_has_one_native_writer() {
        // Split so this guard does not count itself.
        let needle = [".set_", "title("].concat();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut pending = vec![source];
        let mut writers = Vec::new();
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let count = text.matches(needle.as_str()).count();
                if count > 0 {
                    writers.push((
                        path.file_name().unwrap().to_string_lossy().into_owned(),
                        count,
                    ));
                }
            }
        }
        assert_eq!(writers, [("persistence_runtime.rs".to_owned(), 1)]);
    }

    #[test]
    fn only_a_failed_note_save_establishes_the_notice() {
        let mut notice = QuitCancelNotice::default();
        assert!(!notice.on_quit_cancelled(QuitBarrier::Paste));
        assert!(!notice.active());
        assert!(notice.on_quit_cancelled(QuitBarrier::NoteSave));
        assert!(notice.active());
        // A later cancel for another reason leaves it alone.
        assert!(!notice.on_quit_cancelled(QuitBarrier::Paste));
        assert!(notice.active());
    }

    /// Every window effect the reducer can emit, one of each variant (each quit barrier for the
    /// barrier-carrying ones). The `match` stops compiling when a variant is added.
    pub(in crate::app) fn every_effect() -> Vec<WindowEffect> {
        use crate::flow::window::geometry::{PhysicalRect, WindowPlacement};
        use crate::flow::window::state::{DockEdge, WindowSaveReason};
        let barriers = [
            QuitBarrier::Paste,
            QuitBarrier::NoteSave,
            QuitBarrier::AssetGc,
            QuitBarrier::Config,
        ];
        let mut effects = vec![
            WindowEffect::ApplyFrame(PhysicalRect::new(0, 0, 10, 10)),
            WindowEffect::SetTemporarySensorTopmost(true),
            WindowEffect::SetTemporarySensorTopmost(false),
            WindowEffect::SetVisible(false),
            WindowEffect::SetVisible(true),
            WindowEffect::FocusWindow,
            WindowEffect::CancelImePreedit,
            WindowEffect::SetImeAllowed(false),
            WindowEffect::SetImeAllowed(true),
            WindowEffect::SetEditorInputEnabled(false),
            WindowEffect::SetEditorInputEnabled(true),
            WindowEffect::RequestNoteSave(WindowSaveReason::Shutdown),
            WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray),
            WindowEffect::RequestSafeAssetGc,
            WindowEffect::RequestConfigFlush,
            WindowEffect::ExitProcess,
            WindowEffect::CommitPlacement {
                placement: WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
                dock_edge: Some(DockEdge::Left),
            },
            WindowEffect::ReleaseHiddenCaches,
            WindowEffect::ReportTrayUnavailable,
            WindowEffect::HideBlocked,
            WindowEffect::HideCancelled,
            WindowEffect::QuitBlocked,
            WindowEffect::RequestRedraw,
            WindowEffect::WakeAt(1),
        ];
        effects.extend(barriers.map(WindowEffect::QuitCancelled));
        effects.extend(barriers.map(WindowEffect::QuitWarning));
        for effect in &effects {
            match effect {
                WindowEffect::ApplyFrame(_)
                | WindowEffect::SetTemporarySensorTopmost(_)
                | WindowEffect::SetVisible(_)
                | WindowEffect::FocusWindow
                | WindowEffect::CancelImePreedit
                | WindowEffect::SetImeAllowed(_)
                | WindowEffect::SetEditorInputEnabled(_)
                | WindowEffect::RequestNoteSave(_)
                | WindowEffect::RequestSafeAssetGc
                | WindowEffect::RequestConfigFlush
                | WindowEffect::ExitProcess
                | WindowEffect::CommitPlacement { .. }
                | WindowEffect::ReleaseHiddenCaches
                | WindowEffect::ReportTrayUnavailable
                | WindowEffect::HideBlocked
                | WindowEffect::HideCancelled
                | WindowEffect::QuitBlocked
                | WindowEffect::QuitCancelled(_)
                | WindowEffect::QuitWarning(_)
                | WindowEffect::RequestRedraw
                | WindowEffect::WakeAt(_) => {}
            }
        }
        effects
    }

    /// From either state, only a quit cancelled by a failed note save changes the notice, and
    /// only by establishing it: a guard-blocked quit, any other cancel reason and every other
    /// effect leave it as it was. The runtime hands every effect to this one decision.
    #[test]
    fn only_a_quit_cancelled_by_a_failed_save_changes_the_notice() {
        for effect in every_effect() {
            let establishes = effect == WindowEffect::QuitCancelled(QuitBarrier::NoteSave);
            let mut inactive = QuitCancelNotice::default();
            assert_eq!(
                inactive.on_window_effect(&effect),
                establishes,
                "{effect:?}"
            );
            assert_eq!(inactive.active(), establishes, "{effect:?}");
            let mut active = QuitCancelNotice::default();
            active.on_window_effect(&WindowEffect::QuitCancelled(QuitBarrier::NoteSave));
            assert!(!active.on_window_effect(&effect), "{effect:?}");
            assert!(active.active(), "{effect:?} cleared the notice");
        }
    }

    /// No product source other than this file establishes the notice directly: effects reach
    /// it only through `on_window_effect` (a lexical guard; comments count).
    #[test]
    fn the_notice_has_one_entry_for_effects() {
        let needle = ["on_quit", "_cancelled("].concat();
        let mut pending = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut mentions = Vec::new();
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && path.file_name().is_some_and(|name| name != "title.rs")
                && std::fs::read_to_string(&path).unwrap().contains(&needle)
            {
                mentions.push(path);
            }
        }
        assert!(mentions.is_empty(), "{mentions:?}");
    }

    /// The notice settles once per complete main-thread event pass: `finish_event_pass` is
    /// called from exactly one place in the pass settlement after the window tick.
    /// Winit's `about_to_wait` delegates to that settlement once (a lexical guard,
    /// supplemented by lifecycle runtime tests of actual calls and state).
    #[test]
    fn the_notice_settles_at_the_end_of_each_event_pass() {
        let call = ["finish_event", "_pass();"].concat();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut pending = vec![source];
        let mut calls = Vec::new();
        while let Some(path) = pending.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).unwrap() {
                    pending.push(entry.unwrap().path());
                }
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (offset, _) in text.match_indices(call.as_str()) {
                    calls.push((path.clone(), offset, text.clone()));
                }
            }
        }
        assert_eq!(
            calls.len(),
            1,
            "{:?}",
            calls.iter().map(|call| &call.0).collect::<Vec<_>>()
        );
        let (path, offset, text) = &calls[0];
        assert!(path.ends_with("lifecycle.rs"), "{path:?}");
        let pass = text
            .find("fn settle_event_pass(")
            .expect("settle_event_pass");
        let end = text[pass + 1..]
            .find("\n    fn ")
            .map_or(text.len(), |next| pass + 1 + next);
        let tick = text[pass..end]
            .find("WindowIntent::Tick")
            .expect("the window tick in settle_event_pass");
        assert!(
            (pass + tick..end).contains(offset),
            "finish_event_pass runs in settle_event_pass after the window tick"
        );
        let native_pass = text
            .split("fn about_to_wait(")
            .nth(1)
            .unwrap()
            .split("fn user_event(")
            .next()
            .unwrap();
        assert_eq!(
            native_pass
                .matches("self.settle_event_pass(now_ms, Some(event_loop));")
                .count(),
            1
        );
    }

    #[test]
    fn the_notice_clears_only_when_the_note_no_longer_needs_the_barrier() {
        let mut notice = QuitCancelNotice::default();
        notice.on_quit_cancelled(QuitBarrier::NoteSave);
        notice.settle(true);
        assert!(notice.active());
        notice.settle(false);
        assert!(!notice.active());
        // Settling never establishes it.
        notice.settle(true);
        assert!(!notice.active());
    }
}
