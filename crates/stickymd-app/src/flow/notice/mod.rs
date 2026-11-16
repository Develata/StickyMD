//! The notices the user sees: a stack of timed notices over a layer derived from the
//! authoritative state. The shell only reads what to show and hands expiries and clicks back.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

mod derived;
mod state;
mod timed;

pub use derived::{
    Choice, Choices, ConflictSide, DerivedFacts, InProgress, PendingDecision, RecoveryFacts,
};
pub use state::{NoticeState, NoticeTone, NoticeView};
pub use timed::{MAX_TIMED_NOTICES, MessageClass, NoticeId, TimedClass, TimedNotice, TimedNotices};
