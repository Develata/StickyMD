use super::*;

/// The classification rules of plan 09 #notices, finer than the classes they lead to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// 成功：用户主动操作已完成，且结果不在屏幕上。
    FinishedOffScreen,
    /// 提醒·拒绝：前置条件不满足。
    Refusal,
    /// 提醒·可立即重试的失败：窗口操作、打开链接或位置选择器、复制、查找替换。
    RetryableFailure,
    /// 提醒·信息性变化：外部内容已载入、撤销历史已清空、物理清理延后。
    Information,
    /// 重要提醒：涉及用户数据或需要用户处理的失败。
    DataOrUserFailure,
    /// 重要提醒：绘制、呈现、表面尺寸等技术性失败。
    TechnicalFailure,
}

impl Rule {
    const fn class(self) -> MessageClass {
        match self {
            Self::FinishedOffScreen => MessageClass::Success,
            Self::Refusal | Self::RetryableFailure | Self::Information => MessageClass::Reminder,
            Self::DataOrUserFailure | Self::TechnicalFailure => MessageClass::Important,
        }
    }
}

/// The rule each notice falls under, written from plan 09 and not from `Notice::class`. The
/// `match` has no wildcard, so a new notice cannot compile until it is placed under a rule here.
fn rule(notice: &Notice) -> Rule {
    match notice {
        Notice::Saved
        | Notice::KeptLocal
        | Notice::ConflictClearedByRestore
        | Notice::Exported { .. }
        | Notice::Copied
        | Notice::Replaced(_)
        | Notice::Converted(_) => Rule::FinishedOffScreen,

        Notice::ImeComposingInSearch
        | Notice::ImeComposingBeforeSearch
        | Notice::ImeComposingInOpacity
        | Notice::ImagePasteBusy
        | Notice::ImageQueueUnavailable
        | Notice::ExportBusy
        | Notice::ExportQueueUnavailable
        | Notice::RecoveryChoiceRunning
        | Notice::KeepLocalRunning
        | Notice::ExternalNotLoadable
        | Notice::CanonicalNotLoadable
        | Notice::HideNeedsDecision
        | Notice::QuitNeedsDecision
        | Notice::BlockedLinkScheme => Rule::Refusal,

        Notice::MoveResizeFailed(_)
        | Notice::OpacityPreviewFailed(_)
        | Notice::PlacementMissed(_)
        | Notice::TopmostFailed(_)
        | Notice::TopmostRefused(_)
        | Notice::HideFailed(_)
        | Notice::ShowFailed(_)
        | Notice::OpenTargetFailed(_)
        | Notice::ExportPickerFailed(_)
        | Notice::ClipboardFailed(_)
        | Notice::CopyUnmapped
        | Notice::SearchReplaceFailed(_) => Rule::RetryableFailure,

        Notice::ExternalLoaded
        | Notice::ExternalReloadedHistoryCleared
        | Notice::AssetCleanupDeferred
        | Notice::RecoveryCanonicalChanged => Rule::Information,

        Notice::ExternalReadFailed(_)
        | Notice::ExternalReconcileFailed(_)
        | Notice::ExternalLoadFailed(_)
        | Notice::RecoveryCleanupFailed(_)
        | Notice::ConflictCleanupFailed(_)
        | Notice::CanonicalTooLarge
        | Notice::CanonicalPreserveFailed(_)
        | Notice::TemporaryLoadFailed(_)
        | Notice::CanonicalRestoreFailed(_)
        | Notice::AssetSequenceExhausted
        | Notice::ImagePasteFailed(_)
        | Notice::ImageDocumentChanged
        | Notice::ImageCommitFailed(_)
        | Notice::ManagedImagesMissing(_)
        | Notice::AssetReconcileFailed(_)
        | Notice::ExportFailed(_)
        | Notice::ConfigSaveFailed(_)
        | Notice::ConfigFailed(_)
        | Notice::StartupWarning(_)
        | Notice::StartupDegraded(_)
        | Notice::WatcherUnavailable(_)
        | Notice::WatcherStopped(_)
        | Notice::TrayUnavailable
        | Notice::HideCancelledSaveFailed
        | Notice::QuitTimedOut
        | Notice::QuitCancelled(_)
        | Notice::QuitBarrierUnfinished(_)
        | Notice::ToolWindowFailed(_)
        | Notice::PreviewFailed(_)
        | Notice::PreviewWorkerUnavailable(_)
        | Notice::PersistenceUnavailable => Rule::DataOrUserFailure,

        Notice::EditFailed(_) | Notice::RenderFailed(_) => Rule::TechnicalFailure,
    }
}

fn every_notice() -> Vec<Notice> {
    let text = || "x".to_owned();
    vec![
        Notice::Saved,
        Notice::KeptLocal,
        Notice::ConflictClearedByRestore,
        Notice::Exported {
            target: text(),
            assets: 2,
        },
        Notice::Copied,
        Notice::Replaced(4),
        Notice::Converted(2),
        Notice::ImeComposingInSearch,
        Notice::ImeComposingBeforeSearch,
        Notice::ImeComposingInOpacity,
        Notice::ImagePasteBusy,
        Notice::ImageQueueUnavailable,
        Notice::ExportBusy,
        Notice::ExportQueueUnavailable,
        Notice::RecoveryChoiceRunning,
        Notice::KeepLocalRunning,
        Notice::ExternalNotLoadable,
        Notice::CanonicalNotLoadable,
        Notice::HideNeedsDecision,
        Notice::QuitNeedsDecision,
        Notice::BlockedLinkScheme,
        Notice::MoveResizeFailed(text()),
        Notice::OpacityPreviewFailed(text()),
        Notice::PlacementMissed(text()),
        Notice::TopmostFailed(text()),
        Notice::TopmostRefused(text()),
        Notice::HideFailed(text()),
        Notice::ShowFailed(text()),
        Notice::OpenTargetFailed(text()),
        Notice::ExportPickerFailed(text()),
        Notice::ClipboardFailed(text()),
        Notice::CopyUnmapped,
        Notice::SearchReplaceFailed(text()),
        Notice::ExternalLoaded,
        Notice::ExternalReloadedHistoryCleared,
        Notice::AssetCleanupDeferred,
        Notice::RecoveryCanonicalChanged,
        Notice::ExternalReadFailed(text()),
        Notice::ExternalReconcileFailed(text()),
        Notice::ExternalLoadFailed(text()),
        Notice::RecoveryCleanupFailed(text()),
        Notice::ConflictCleanupFailed(text()),
        Notice::CanonicalTooLarge,
        Notice::CanonicalPreserveFailed(text()),
        Notice::TemporaryLoadFailed(text()),
        Notice::CanonicalRestoreFailed(text()),
        Notice::AssetSequenceExhausted,
        Notice::ImagePasteFailed(text()),
        Notice::ImageDocumentChanged,
        Notice::ImageCommitFailed(text()),
        Notice::ManagedImagesMissing(3),
        Notice::AssetReconcileFailed(text()),
        Notice::ExportFailed(text()),
        Notice::ConfigSaveFailed(text()),
        Notice::ConfigFailed(text()),
        Notice::EditFailed(text()),
        Notice::PreviewFailed(text()),
        Notice::PreviewWorkerUnavailable(text()),
        Notice::StartupWarning(text()),
        Notice::StartupDegraded(text()),
        Notice::WatcherUnavailable(text()),
        Notice::WatcherStopped(text()),
        Notice::TrayUnavailable,
        Notice::HideCancelledSaveFailed,
        Notice::QuitTimedOut,
        Notice::QuitCancelled(text()),
        Notice::QuitBarrierUnfinished(text()),
        Notice::ToolWindowFailed(text()),
        Notice::RenderFailed(text()),
        Notice::PersistenceUnavailable,
    ]
}

/// P17-N09: every notice is classified by the plan 09 rule it falls under.
#[test]
fn every_notice_takes_the_class_of_its_plan_rule() {
    let notices = every_notice();
    for notice in &notices {
        assert_eq!(notice.class(), rule(notice).class(), "{notice:?}");
        assert!(!notice.to_string().is_empty(), "{notice:?} has no text");
    }
    // Two samples of one variant would hide a missing one: each variant appears once.
    let mut names: Vec<_> = notices
        .iter()
        .map(|notice| {
            format!("{notice:?}")
                .split(['(', ' '])
                .next()
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), notices.len());
}

/// Editor failures are reported by what failed: the clipboard and a stale Search match can be
/// tried again, a document or conversion failure is technical.
#[test]
fn an_editor_failure_is_reported_by_what_failed() {
    let clipboard = EditorFlowError::Clipboard(ClipboardError::Unavailable("busy".into()));
    assert_eq!(
        Notice::editor_failure(&clipboard).class(),
        MessageClass::Reminder
    );
    assert_eq!(
        Notice::editor_failure(&EditorFlowError::SearchRangeMismatch).class(),
        MessageClass::Reminder
    );
}
