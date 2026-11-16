//! The session's notices: what happened is reported here and classified by the catalog; what
//! waits or runs is derived from the session's own state; the shell reads one view of it.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

use crate::flow::notice::{ConflictSide, DerivedFacts, Notice, NoticeView, RecoveryFacts};

use super::{AppSession, ShellEffect, ShellEffects};

impl AppSession {
    /// Shows `notice` under the class the catalog gives it. The shell reports its own events
    /// here and redraws; it never chooses a class.
    pub fn notify(&mut self, notice: Notice) {
        let now_ms = self.now_ms();
        self.notices
            .notify(notice.class(), notice.to_string(), now_ms);
    }

    /// `notify`, then the redraw that shows it.
    pub(super) fn show_notice(&mut self, notice: Notice, out: &mut ShellEffects) {
        self.notify(notice);
        out.push(ShellEffect::RedrawNeeded);
    }

    /// A new segment of the fault display: the save failure began or changed kind.
    pub(super) fn start_fault_display(&mut self, out: &mut ShellEffects) {
        let now_ms = self.now_ms();
        self.notices.start_fault_display(now_ms);
        out.push(ShellEffect::RedrawNeeded);
    }

    /// The toolbar marker was clicked: the failure's text shows for another 30 s. Returns
    /// whether there was a failure to show.
    pub fn show_save_fault(&mut self) -> bool {
        if self.persistence.unresolved_failure().is_none() {
            return false;
        }
        let now_ms = self.now_ms();
        self.notices.start_fault_display(now_ms);
        true
    }

    /// Drops every timed notice, so a test can watch for the next one.
    #[cfg(any(test, feature = "testing"))]
    pub fn clear_timed_notices(&mut self) {
        self.notices = crate::flow::notice::NoticeState::default();
    }

    /// The timed notices' texts, newest first; a fault display reads as its failure's text.
    #[cfg(any(test, feature = "testing"))]
    pub fn timed_notice_texts(&self) -> Vec<String> {
        let fault = self
            .persistence
            .unresolved_failure()
            .map(|fault| fault.text.clone());
        self.notices
            .timed()
            .filter_map(|notice| notice.text().map(str::to_owned).or_else(|| fault.clone()))
            .collect()
    }

    /// Whether the toolbar shows the save-failure marker: until a later save succeeds.
    pub fn save_fault_marked(&self) -> bool {
        self.persistence.unresolved_failure().is_some()
    }

    /// What the notice bar shows now, if anything.
    pub fn notice_view(&self) -> Option<NoticeView<'_>> {
        self.notices.view(
            &self.derived_notice_facts(),
            self.persistence
                .unresolved_failure()
                .map(|fault| fault.text.as_str()),
        )
    }

    /// When the shown timed notice ends.
    pub(super) fn notice_deadline(&self) -> Option<u64> {
        self.notices.deadline_ms()
    }

    /// Hands back the expiry of the shown notice if it is due by `now_ms`.
    pub(super) fn expire_notice(&mut self, now_ms: u64, out: &mut ShellEffects) {
        if let Some(id) = self.notices.due(now_ms)
            && self.notices.expire(id, now_ms)
        {
            out.push(ShellEffect::RedrawNeeded);
        }
    }

    /// The authoritative facts the derived layer reads (plan 04 #notice-state).
    fn derived_notice_facts(&self) -> DerivedFacts {
        DerivedFacts {
            conflict: self
                .persistence
                .conflict()
                .map(|conflict| ConflictSide::of(&conflict.external)),
            keep_local_in_flight: self.persistence.keep_local_in_progress(),
            recovery: self.recovery.is_pending().then(|| RecoveryFacts {
                operation: self.recovery.operation(),
                canonical_usable: self.recovery.canonical_usable(),
            }),
            manual_save: self.persistence.manual_save(),
            preparing_images: self.assets.paste_pending,
            checking_images: self.assets.reconcile_pending,
            export_in_flight: self.export_in_flight,
        }
    }
}
