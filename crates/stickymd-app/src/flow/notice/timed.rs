//! The timed layer of the notices: a bounded stack whose top is shown, each entry with its own
//! deadline on the injected clock.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

/// At most this many timed notices are kept; pushing past it drops the oldest.
pub const MAX_TIMED_NOTICES: usize = 8;

/// The class of a timed notice, which fixes how long it stays (plan 09 #notices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedClass {
    Success,
    Reminder,
    Important,
    /// Shows `SaveState.unresolved_failure`; it carries no text of its own.
    FaultDisplay,
}

impl TimedClass {
    pub const fn duration_ms(self) -> u64 {
        match self {
            Self::Success => 3_000,
            Self::Reminder => 5_000,
            Self::Important | Self::FaultDisplay => 30_000,
        }
    }
}

/// The class of a timed notice that carries its own text: every class but the fault display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageClass {
    Success,
    Reminder,
    Important,
}

impl From<MessageClass> for TimedClass {
    fn from(class: MessageClass) -> Self {
        match class {
            MessageClass::Success => Self::Success,
            MessageClass::Reminder => Self::Reminder,
            MessageClass::Important => Self::Important,
        }
    }
}

/// Names one pushed notice. Ids increase monotonically, so a receipt for a notice that was
/// replaced or dropped can never name a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NoticeId(u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedNotice {
    id: NoticeId,
    class: TimedClass,
    /// `None` exactly when `class` is `FaultDisplay`.
    text: Option<String>,
    deadline_ms: u64,
}

impl TimedNotice {
    pub const fn id(&self) -> NoticeId {
        self.id
    }

    pub const fn class(&self) -> TimedClass {
        self.class
    }

    /// The notice's own text; a fault display takes its text from the save failure instead.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub const fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

/// The timed stack. Invariants: at most `MAX_TIMED_NOTICES` entries; ids strictly increase from
/// bottom to top, since every push takes a fresh id and only ever lands on top.
#[derive(Debug, Default)]
pub struct TimedNotices {
    stack: Vec<TimedNotice>,
    issued: u64,
}

impl TimedNotices {
    /// The notice shown now, if any; with none the derived layer shows through.
    pub fn top(&self) -> Option<&TimedNotice> {
        self.stack.last()
    }

    /// When the shown notice ends; covered notices are not woken for, since only the top's
    /// expiry is ever acted on.
    pub fn deadline_ms(&self) -> Option<u64> {
        self.top().map(TimedNotice::deadline_ms)
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    /// Pushes a notice with its own text and returns its id, or `None` when it is not shown.
    ///
    /// Failures and refusals (`Reminder`, `Important`) always land on top. A `Success` is shown
    /// only on an empty stack or in place of a `Success` on top; otherwise it is dropped, so a
    /// success never covers a failure.
    pub fn push_message(
        &mut self,
        class: MessageClass,
        text: String,
        now_ms: u64,
    ) -> Option<NoticeId> {
        if class == MessageClass::Success {
            match self.top().map(TimedNotice::class) {
                None => {}
                Some(TimedClass::Success) => {
                    self.stack.pop();
                }
                Some(_) => return None,
            }
        }
        Some(self.push(class.into(), Some(text), now_ms))
    }

    /// Starts a new segment of the fault display on top and returns its id.
    pub fn push_fault_display(&mut self, now_ms: u64) -> NoticeId {
        self.push(TimedClass::FaultDisplay, None, now_ms)
    }

    fn push(&mut self, class: TimedClass, text: Option<String>, now_ms: u64) -> NoticeId {
        self.issued += 1;
        let id = NoticeId(self.issued);
        self.stack.push(TimedNotice {
            id,
            class,
            text,
            deadline_ms: now_ms.saturating_add(class.duration_ms()),
        });
        if self.stack.len() > MAX_TIMED_NOTICES {
            self.stack.remove(0);
        }
        id
    }

    /// The expiry receipt of notice `id` at `now_ms`. Only the shown notice, at or past its
    /// deadline, is popped; any other receipt is ignored and `false` is returned. After a pop
    /// every covered notice already past its deadline is dropped too: its time ran while covered.
    pub fn expire(&mut self, id: NoticeId, now_ms: u64) -> bool {
        match self.top() {
            Some(top) if top.id == id && now_ms >= top.deadline_ms => {
                self.stack.pop();
                self.stack.retain(|notice| notice.deadline_ms > now_ms);
                true
            }
            _ => false,
        }
    }

    /// The id of the shown notice if its deadline has passed by `now_ms`.
    pub fn due(&self, now_ms: u64) -> Option<NoticeId> {
        self.top()
            .filter(|top| now_ms >= top.deadline_ms)
            .map(TimedNotice::id)
    }

    /// The save failure was resolved: every fault display goes, the rest keep their place.
    pub fn remove_fault_displays(&mut self) {
        self.stack
            .retain(|notice| notice.class != TimedClass::FaultDisplay);
    }
}

#[cfg(test)]
mod tests;
