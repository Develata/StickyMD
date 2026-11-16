//! The two layers together: the timed stack on top, the derived layer below, and the save
//! failure that a fault display shows.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

use std::borrow::Cow;

use super::derived::{Choices, DerivedFacts};
use super::timed::{MessageClass, NoticeId, TimedClass, TimedNotices};

/// How the shown notice looks; the shell picks colours from it and decides nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeTone {
    Success,
    Reminder,
    Important,
    Fault,
    Pending,
    InProgress,
}

/// What the notice bar shows now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeView<'a> {
    pub text: Cow<'a, str>,
    pub tone: NoticeTone,
    /// The buttons of the waiting decision. They stay whatever timed notice is shown above it,
    /// so a failed "keep local" shows its reason and the conflict's choices together.
    pub choices: Option<Choices>,
}

#[derive(Debug, Default)]
pub struct NoticeState {
    timed: TimedNotices,
}

impl NoticeState {
    /// Pushes a notice with its own text (`TimedNotices::push_message` says when it shows).
    pub fn notify(&mut self, class: MessageClass, text: String, now_ms: u64) -> Option<NoticeId> {
        self.timed.push_message(class, text, now_ms)
    }

    /// Starts a new segment of the fault display: the failure began, changed kind, or the user
    /// clicked the toolbar marker.
    pub fn start_fault_display(&mut self, now_ms: u64) -> NoticeId {
        self.timed.push_fault_display(now_ms)
    }

    /// The save failure was resolved.
    pub fn clear_fault_displays(&mut self) {
        self.timed.remove_fault_displays();
    }

    /// The timed notices, newest first: the shown one, then those it covers.
    pub fn timed(&self) -> impl Iterator<Item = &super::timed::TimedNotice> {
        self.timed.iter().rev()
    }

    /// When the shown timed notice ends.
    pub fn deadline_ms(&self) -> Option<u64> {
        self.timed.deadline_ms()
    }

    /// The shown timed notice's id if it is due by `now_ms`: the expiry receipt to hand back.
    pub fn due(&self, now_ms: u64) -> Option<NoticeId> {
        self.timed.due(now_ms)
    }

    /// The expiry receipt of `id`; see `TimedNotices::expire`.
    pub fn expire(&mut self, id: NoticeId, now_ms: u64) -> bool {
        self.timed.expire(id, now_ms)
    }

    /// What to show over `derived`; `fault_text` is the text of the unresolved save failure.
    pub fn view<'a>(
        &'a self,
        derived: &DerivedFacts,
        fault_text: Option<&'a str>,
    ) -> Option<NoticeView<'a>> {
        let pending = derived.pending();
        let choices = pending.as_ref().map(|decision| decision.choices());
        if let Some(top) = self.timed.top() {
            let shown = match (top.class(), top.text()) {
                (TimedClass::FaultDisplay, _) => fault_text.map(|text| (text, NoticeTone::Fault)),
                (TimedClass::Success, Some(text)) => Some((text, NoticeTone::Success)),
                (TimedClass::Reminder, Some(text)) => Some((text, NoticeTone::Reminder)),
                (TimedClass::Important, Some(text)) => Some((text, NoticeTone::Important)),
                (_, None) => None,
            };
            if let Some((text, tone)) = shown {
                return Some(NoticeView {
                    text: Cow::Borrowed(text),
                    tone,
                    choices,
                });
            }
        }
        if let Some(decision) = pending {
            return Some(NoticeView {
                text: Cow::Owned(decision.text()),
                tone: NoticeTone::Pending,
                choices,
            });
        }
        derived.in_progress().map(|operation| NoticeView {
            text: Cow::Owned(operation.text()),
            tone: NoticeTone::InProgress,
            choices: None,
        })
    }
}

#[cfg(test)]
mod tests;
