//! The window title: one writer, the plan 09 priority, and the quit-cancelled notice.
//!
//! The title is a projection of state owned elsewhere (recovery, the quit notice, the external
//! conflict, the dirty flag and a formula-error hover); `window_title` is the only place that
//! turns those facts into text, so no caller can show a lower-priority state over a higher one.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tray-lifecycle

use crate::flow::window::state::QuitBarrier;

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
}

impl QuitCancelNotice {
    pub(super) const fn active(self) -> bool {
        self.active
    }

    /// Records a cancelled quit; returns whether the notice changed.
    pub(super) fn on_quit_cancelled(&mut self, barrier: QuitBarrier) -> bool {
        let establish = barrier == QuitBarrier::NoteSave && !self.active;
        self.active |= establish;
        establish
    }

    /// Runs after a complete main-thread event pass.
    pub(super) fn settle(&mut self, note_barrier_required: bool) {
        self.active &= note_barrier_required;
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn each_state_has_its_title_and_higher_states_win() {
        let all = TitleFacts {
            recovery_pending: true,
            quit_cancelled: true,
            conflict: true,
            dirty: true,
            formula_error: Some("x"),
        };
        // Clearing the highest remaining fact reveals the next one, in plan 09 order.
        let mut current = all;
        let mut seen = Vec::new();
        for clear in [
            |f: &mut TitleFacts<'_>| f.recovery_pending = false,
            |f: &mut TitleFacts<'_>| f.quit_cancelled = false,
            |f: &mut TitleFacts<'_>| f.conflict = false,
            |f: &mut TitleFacts<'_>| f.dirty = false,
            |f: &mut TitleFacts<'_>| f.formula_error = None,
        ] {
            seen.push(window_title(current).into_owned());
            clear(&mut current);
        }
        seen.push(window_title(current).into_owned());
        assert_eq!(
            seen,
            [
                RECOVERY,
                QUIT_CANCELLED,
                CONFLICT,
                DIRTY,
                "StickyMD — 公式错误：x",
                PLAIN
            ]
        );
        assert_eq!(window_title(facts()), PLAIN);
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
