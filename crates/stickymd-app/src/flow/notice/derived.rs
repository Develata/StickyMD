//! The derived layer of the notices: what waits for the user's choice and what is in progress,
//! read from the authoritative state each time and never stored.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

use stickymd_core::{ExternalFileState, Generation};

use crate::flow::RecoveryOperation;

/// One answer to a waiting decision, bound to the key that gives it (plan 09 #notices).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// F6 in a conflict.
    LoadExternal,
    /// F7 in a conflict.
    KeepLocal,
    /// F6 in a recovery.
    RestoreTemporary,
    /// F7 in a recovery.
    UseCanonical,
}

impl Choice {
    /// Whether the choice is the primary (F6) answer; the other one is F7.
    pub const fn is_primary(self) -> bool {
        matches!(self, Self::LoadExternal | Self::RestoreTemporary)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::LoadExternal => "F6 载入外部",
            Self::KeepLocal => "F7 保留本地",
            Self::RestoreTemporary => "F6 恢复临时内容",
            Self::UseCanonical => "F7 使用当前文件",
        }
    }
}

/// The answers a waiting decision offers: one button per available option, F6 first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choices {
    pub primary: Option<Choice>,
    pub secondary: Option<Choice>,
}

impl Choices {
    /// The offered choices in button order.
    pub fn iter(self) -> impl Iterator<Item = Choice> {
        [self.primary, self.secondary].into_iter().flatten()
    }
}

/// A decision the user must make; it never expires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingDecision {
    /// The note changed outside while the text in memory was not saved.
    Conflict(ConflictSide),
    /// A temporary file from an unfinished save was found at startup.
    Recovery {
        /// Whether the current note.md can be used; an invalid or too large one leaves only F6.
        canonical_usable: bool,
    },
}

/// What the external side of a conflict is, which decides whether it can be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictSide {
    Loadable,
    InvalidUtf8,
    TooLarge {
        bytes: u64,
    },
    /// Missing, or no conflict detail: only keeping the local text is possible.
    Other,
}

impl ConflictSide {
    pub fn of(external: &ExternalFileState) -> Self {
        match external {
            ExternalFileState::Present(_) => Self::Loadable,
            ExternalFileState::InvalidUtf8 { .. } => Self::InvalidUtf8,
            ExternalFileState::TooLarge { bytes } => Self::TooLarge { bytes: *bytes },
            ExternalFileState::Missing => Self::Other,
        }
    }
}

impl PendingDecision {
    pub fn text(&self) -> String {
        match self {
            Self::Conflict(ConflictSide::Loadable) => "文件已在外部修改".into(),
            Self::Conflict(ConflictSide::InvalidUtf8) => {
                "外部文件不是有效 UTF-8；只能保留本地并覆盖。".into()
            }
            Self::Conflict(ConflictSide::TooLarge { bytes }) => {
                format!("外部 note.md 过大（{bytes} bytes）；只能保留本地并覆盖。")
            }
            Self::Conflict(ConflictSide::Other) => "外部文件状态冲突；只能保留本地。".into(),
            Self::Recovery {
                canonical_usable: true,
            } => "发现未完成保存的内容".into(),
            Self::Recovery {
                canonical_usable: false,
            } => "发现未完成保存的内容；当前 note.md 无法载入，只能恢复临时内容。".into(),
        }
    }

    pub const fn choices(&self) -> Choices {
        match self {
            Self::Conflict(ConflictSide::Loadable) => Choices {
                primary: Some(Choice::LoadExternal),
                secondary: Some(Choice::KeepLocal),
            },
            Self::Conflict(_) => Choices {
                primary: None,
                secondary: Some(Choice::KeepLocal),
            },
            Self::Recovery { canonical_usable } => Choices {
                primary: Some(Choice::RestoreTemporary),
                secondary: if *canonical_usable {
                    Some(Choice::UseCanonical)
                } else {
                    None
                },
            },
        }
    }
}

/// An operation that has started and is settled by its own result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InProgress {
    Recovery(RecoveryOperation),
    KeepLocal,
    ManualSave(Generation),
    PreparingImages,
    CheckingImages,
    Export,
}

impl InProgress {
    pub fn text(self) -> String {
        match self {
            Self::Recovery(RecoveryOperation::Restoring) => "正在恢复临时内容…".into(),
            Self::Recovery(RecoveryOperation::UsingCanonical) => "正在确认使用当前 note.md…".into(),
            Self::KeepLocal => "正在用内存文本执行“保留本地”…".into(),
            Self::ManualSave(generation) => format!("正在保存 generation {}…", generation.value()),
            Self::PreparingImages => "正在准备图片…".into(),
            Self::CheckingImages => "恢复选择已完成；正在核对受管图片后启用编辑。".into(),
            Self::Export => "正在导出当前快照…".into(),
        }
    }
}

/// The recovery facts the derived layer reads while a candidate exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryFacts {
    pub operation: Option<RecoveryOperation>,
    pub canonical_usable: bool,
}

/// The authoritative facts the derived layer reads, gathered by the session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DerivedFacts {
    pub conflict: Option<ConflictSide>,
    /// "Keep local" was chosen and its write has not reported back.
    pub keep_local_in_flight: bool,
    pub recovery: Option<RecoveryFacts>,
    pub manual_save: Option<Generation>,
    pub preparing_images: bool,
    pub checking_images: bool,
    pub export_in_flight: bool,
}

impl DerivedFacts {
    /// The decision waiting for the user, only while no choice for it is being carried out.
    pub fn pending(&self) -> Option<PendingDecision> {
        if let Some(side) = self.conflict
            && !self.keep_local_in_flight
        {
            return Some(PendingDecision::Conflict(side));
        }
        self.recovery
            .filter(|recovery| recovery.operation.is_none())
            .map(|recovery| PendingDecision::Recovery {
                canonical_usable: recovery.canonical_usable,
            })
    }

    pub fn in_progress(&self) -> Option<InProgress> {
        if let Some(operation) = self.recovery.and_then(|recovery| recovery.operation) {
            return Some(InProgress::Recovery(operation));
        }
        if self.keep_local_in_flight {
            return Some(InProgress::KeepLocal);
        }
        if let Some(generation) = self.manual_save {
            return Some(InProgress::ManualSave(generation));
        }
        if self.preparing_images {
            return Some(InProgress::PreparingImages);
        }
        if self.checking_images {
            return Some(InProgress::CheckingImages);
        }
        self.export_in_flight.then_some(InProgress::Export)
    }
}
