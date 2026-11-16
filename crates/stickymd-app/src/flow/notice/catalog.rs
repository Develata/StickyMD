//! Every timed notice the application can show. A site reports what happened; the class comes
//! from one exhaustive `match` here, under the classification rules of plan 09 #notices. The
//! derived layer (decisions and operations in progress) and the fault display are not here:
//! they come from state, not from a site.
//!
//! plan_ref: docs/plan/09_windows_shell.md#notices

use std::fmt;

use super::timed::MessageClass;
use crate::flow::{ClipboardError, EditorFlowError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    // Success: a user action finished and its result is not on screen.
    Saved,
    KeptLocal,
    ConflictClearedByRestore,
    Exported {
        target: String,
        assets: usize,
    },
    Copied,
    Replaced(usize),
    Converted(usize),

    // Reminder, refusal: a precondition does not hold.
    ImeComposingInSearch,
    ImeComposingBeforeSearch,
    ImeComposingInOpacity,
    ImagePasteBusy,
    ImageQueueUnavailable,
    ExportBusy,
    ExportQueueUnavailable,
    RecoveryChoiceRunning,
    KeepLocalRunning,
    ExternalNotLoadable,
    CanonicalNotLoadable,
    HideNeedsDecision,
    QuitNeedsDecision,
    BlockedLinkScheme,

    // Reminder, a failure the user can retry at once.
    MoveResizeFailed(String),
    OpacityPreviewFailed(String),
    PlacementMissed(String),
    TopmostFailed(String),
    TopmostRefused(String),
    HideFailed(String),
    ShowFailed(String),
    OpenTargetFailed(String),
    ExportPickerFailed(String),
    ClipboardFailed(String),
    /// A Preview selection that does not map onto its source (plan 06 rule 6): nothing copied.
    CopyUnmapped,
    SearchReplaceFailed(String),

    // Reminder, information: something changed that the user should know of.
    ExternalLoaded,
    ExternalReloadedHistoryCleared,
    AssetCleanupDeferred,
    RecoveryCanonicalChanged,

    // Important: a failure that touches the user's data or needs the user.
    ExternalReadFailed(String),
    ExternalReconcileFailed(String),
    ExternalLoadFailed(String),
    RecoveryCleanupFailed(String),
    ConflictCleanupFailed(String),
    CanonicalTooLarge,
    CanonicalPreserveFailed(String),
    TemporaryLoadFailed(String),
    CanonicalRestoreFailed(String),
    AssetSequenceExhausted,
    ImagePasteFailed(String),
    ImageDocumentChanged,
    ImageCommitFailed(String),
    ManagedImagesMissing(usize),
    AssetReconcileFailed(String),
    ExportFailed(String),
    ConfigSaveFailed(String),
    ConfigFailed(String),
    EditFailed(String),
    PreviewFailed(String),
    PreviewWorkerUnavailable(String),
    StartupWarning(String),
    StartupDegraded(String),
    WatcherUnavailable(String),
    WatcherStopped(String),
    TrayUnavailable,
    HideCancelledSaveFailed,
    QuitTimedOut,
    QuitCancelled(String),
    QuitBarrierUnfinished(String),
    ToolWindowFailed(String),
    /// Painting, presenting, the surface, the caret overlay or the projection failed.
    RenderFailed(String),
    /// A choice or a save needs the persistence worker, which has stopped.
    PersistenceUnavailable,
}

impl Notice {
    /// The class of the notice (plan 09 #notices). A site that fits no rule is important.
    pub const fn class(&self) -> MessageClass {
        match self {
            Self::Saved
            | Self::KeptLocal
            | Self::ConflictClearedByRestore
            | Self::Exported { .. }
            | Self::Copied
            | Self::Replaced(_)
            | Self::Converted(_) => MessageClass::Success,

            Self::ImeComposingInSearch
            | Self::ImeComposingBeforeSearch
            | Self::ImeComposingInOpacity
            | Self::ImagePasteBusy
            | Self::ImageQueueUnavailable
            | Self::ExportBusy
            | Self::ExportQueueUnavailable
            | Self::RecoveryChoiceRunning
            | Self::KeepLocalRunning
            | Self::ExternalNotLoadable
            | Self::CanonicalNotLoadable
            | Self::HideNeedsDecision
            | Self::QuitNeedsDecision
            | Self::BlockedLinkScheme
            | Self::MoveResizeFailed(_)
            | Self::OpacityPreviewFailed(_)
            | Self::PlacementMissed(_)
            | Self::TopmostFailed(_)
            | Self::TopmostRefused(_)
            | Self::HideFailed(_)
            | Self::ShowFailed(_)
            | Self::OpenTargetFailed(_)
            | Self::ExportPickerFailed(_)
            | Self::ClipboardFailed(_)
            | Self::CopyUnmapped
            | Self::SearchReplaceFailed(_)
            | Self::ExternalLoaded
            | Self::ExternalReloadedHistoryCleared
            | Self::AssetCleanupDeferred
            | Self::RecoveryCanonicalChanged => MessageClass::Reminder,

            Self::ExternalReadFailed(_)
            | Self::ExternalReconcileFailed(_)
            | Self::ExternalLoadFailed(_)
            | Self::RecoveryCleanupFailed(_)
            | Self::ConflictCleanupFailed(_)
            | Self::CanonicalTooLarge
            | Self::CanonicalPreserveFailed(_)
            | Self::TemporaryLoadFailed(_)
            | Self::CanonicalRestoreFailed(_)
            | Self::AssetSequenceExhausted
            | Self::ImagePasteFailed(_)
            | Self::ImageDocumentChanged
            | Self::ImageCommitFailed(_)
            | Self::ManagedImagesMissing(_)
            | Self::AssetReconcileFailed(_)
            | Self::ExportFailed(_)
            | Self::ConfigSaveFailed(_)
            | Self::ConfigFailed(_)
            | Self::EditFailed(_)
            | Self::PreviewFailed(_)
            | Self::PreviewWorkerUnavailable(_)
            | Self::StartupWarning(_)
            | Self::StartupDegraded(_)
            | Self::WatcherUnavailable(_)
            | Self::WatcherStopped(_)
            | Self::TrayUnavailable
            | Self::HideCancelledSaveFailed
            | Self::QuitTimedOut
            | Self::QuitCancelled(_)
            | Self::QuitBarrierUnfinished(_)
            | Self::ToolWindowFailed(_)
            | Self::RenderFailed(_)
            | Self::PersistenceUnavailable => MessageClass::Important,
        }
    }

    /// The notice for a refused or failed editor intent.
    pub fn editor_failure(error: &EditorFlowError) -> Self {
        match error {
            EditorFlowError::Clipboard(error) => Self::clipboard_failure(error),
            EditorFlowError::SearchRangeMismatch => Self::SearchReplaceFailed(error.to_string()),
            EditorFlowError::Document(_) | EditorFlowError::SemanticConversion(_) => {
                Self::EditFailed(error.to_string())
            }
        }
    }

    pub fn clipboard_failure(error: &ClipboardError) -> Self {
        Self::ClipboardFailed(error.to_string())
    }
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Saved => f.write_str("已保存"),
            Self::KeptLocal => f.write_str("已保留本地内容并覆盖外部文件。"),
            Self::ConflictClearedByRestore => f.write_str("外部文件已恢复为已知版本，冲突已解除。"),
            Self::Exported { target, assets } => {
                write!(f, "导出完成：{target}（{assets} 个本地资源）")
            }
            Self::Copied => f.write_str("已复制"),
            Self::Replaced(count) => write!(f, "已替换 {count} 处"),
            Self::Converted(count) => write!(f, "已转换 {count} 处"),

            Self::ImeComposingInSearch => f.write_str(crate::interaction::rules::SEARCH_COMPOSING),
            Self::ImeComposingBeforeSearch => f.write_str("请先完成当前输入法组合，再打开查找。"),
            Self::ImeComposingInOpacity => f.write_str("请先完成当前输入法组合。"),
            Self::ImagePasteBusy => f.write_str("上一批图片仍在处理中；本次粘贴未开始。"),
            Self::ImageQueueUnavailable => f.write_str("图片工作队列暂不可用；未修改文档。"),
            Self::ExportBusy => f.write_str("已有导出任务正在进行。"),
            Self::ExportQueueUnavailable => f.write_str("导出队列暂时不可用。"),
            Self::RecoveryChoiceRunning => f.write_str("恢复操作正在完成，请稍候。"),
            Self::KeepLocalRunning => f.write_str("正在保存本地内容，请稍候。"),
            Self::ExternalNotLoadable => f.write_str("外部文件不可载入；只能 F7 保留本地覆盖。"),
            Self::CanonicalNotLoadable => f.write_str(
                "当前 note.md 无法载入（不是有效 UTF-8 或过大）；只能 F6 恢复有效临时内容。",
            ),
            Self::HideNeedsDecision => f.write_str("恢复或外部冲突未解决，窗口不会隐藏。"),
            Self::QuitNeedsDecision => f.write_str("退出前必须先解决恢复或外部修改冲突。"),
            Self::BlockedLinkScheme => f.write_str("preview destination uses a blocked URI scheme"),

            Self::MoveResizeFailed(error) => write!(f, "无法开始窗口拖动或缩放：{error}"),
            Self::OpacityPreviewFailed(error) => write!(f, "无法预览透明度：{error}"),
            Self::PlacementMissed(kind) => write!(f, "窗口未能到达目标位置（{kind}）。"),
            Self::TopmostFailed(reason) => write!(f, "无法应用窗口置顶状态（{reason}）。"),
            Self::TopmostRefused(error) => write!(f, "无法应用窗口置顶状态：{error}"),
            Self::HideFailed(kind) => write!(f, "窗口未能隐藏，保持显示（{kind}）。"),
            Self::ShowFailed(kind) => write!(f, "窗口未能显示（{kind}）。"),
            Self::ExportPickerFailed(error) => write!(f, "无法打开导出位置选择器：{error}"),
            Self::CopyUnmapped => f.write_str("复制失败：选区无法对应到源码，剪贴板未改动。"),
            Self::OpenTargetFailed(error)
            | Self::ClipboardFailed(error)
            | Self::SearchReplaceFailed(error)
            | Self::ConfigFailed(error)
            | Self::EditFailed(error)
            | Self::StartupWarning(error)
            | Self::StartupDegraded(error)
            | Self::RenderFailed(error)
            | Self::PreviewFailed(error) => f.write_str(error),

            Self::ExternalLoaded => f.write_str("已载入外部修改。"),
            Self::ExternalReloadedHistoryCleared => {
                f.write_str("已载入最新外部内容；撤销历史已清空。")
            }
            Self::AssetCleanupDeferred => {
                f.write_str("便签磁盘状态正在变化；已保留回收站证据并延后物理清理。")
            }
            Self::RecoveryCanonicalChanged => {
                f.write_str("恢复等待期间 note.md 已变化，未覆盖外部内容；请重新选择 F6/F7。")
            }

            Self::ExternalReadFailed(error) => {
                write!(f, "外部文件读取失败，未改变当前文本：{error}")
            }
            Self::ExternalReconcileFailed(error) => write!(f, "外部内容协调失败：{error}"),
            Self::ExternalLoadFailed(error) => write!(f, "载入外部内容失败：{error}"),
            Self::RecoveryCleanupFailed(error) => {
                write!(f, "临时恢复证据无法清理，仍保持恢复状态：{error}")
            }
            Self::ConflictCleanupFailed(error) => write!(
                f,
                "外部内容已载入，但本地临时副本无法清理；下次启动可能再次提示恢复：{error}"
            ),
            Self::CanonicalTooLarge => f.write_str(
                "note.md 过大，未恢复：请结束进程，移走 note.md（保留 .tmp）后重启按 F6。",
            ),
            Self::CanonicalPreserveFailed(error) => write!(
                f,
                "无效的 canonical note.md 无法先行保留；未执行恢复覆盖：{error}"
            ),
            Self::TemporaryLoadFailed(error) => write!(f, "无法载入临时内容：{error}"),
            Self::CanonicalRestoreFailed(error) => write!(f, "无法恢复当前 note.md 状态：{error}"),
            Self::AssetSequenceExhausted => f.write_str("图片事务序号已耗尽；已停止自动整理。"),
            Self::ImagePasteFailed(error) => write!(
                f,
                "图片粘贴失败；未插入引用，资产正按最新引用状态收敛：{error}"
            ),
            Self::ImageDocumentChanged => {
                f.write_str("图片准备期间文档已变化；未插入引用，资产正按最新引用状态收敛。")
            }
            Self::ImageCommitFailed(error) => write!(
                f,
                "图片已准备但文档事务未提交；资产正按最新引用状态收敛：{error}"
            ),
            Self::ManagedImagesMissing(count) => {
                write!(f, "有 {count} 个受管图片引用缺少文件；Markdown 未修改。")
            }
            Self::AssetReconcileFailed(error) => {
                write!(f, "图片整理未完成；用户文件与文档均未删除：{error}")
            }
            Self::ExportFailed(error) => write!(f, "导出失败；工作文档未改变：{error}"),
            Self::ConfigSaveFailed(error) => {
                write!(f, "配置保存失败；笔记保存不受影响：{error}")
            }
            Self::PreviewWorkerUnavailable(error) => {
                write!(f, "Preview worker unavailable: {error}")
            }
            Self::WatcherUnavailable(error) => write!(
                f,
                "外部文件自动检测不可用；安全保存仍受指纹校验保护：{error}"
            ),
            Self::WatcherStopped(error) => write!(
                f,
                "外部文件自动检测已停止；安全保存仍受指纹校验保护：{error}"
            ),
            Self::TrayUnavailable => f.write_str("系统托盘不可用；窗口未隐藏。"),
            Self::HideCancelledSaveFailed => {
                f.write_str("保存失败，窗口保持显示，内存文本仍保留。")
            }
            Self::QuitTimedOut => f.write_str("退出等待超时，已取消；保存可能仍在进行。"),
            Self::QuitCancelled(barrier) => write!(f, "退出已取消：{barrier} 未安全完成。"),
            Self::QuitBarrierUnfinished(barrier) => {
                write!(f, "退出前的 {barrier} 未完成；证据已保留。")
            }
            Self::ToolWindowFailed(error) => write!(
                f,
                "无法保持工具窗口身份；已恢复任务栏入口以避免窗口不可达：{error}"
            ),
            Self::PersistenceUnavailable => {
                f.write_str("持久化不可用：工作线程已停止，本次操作未执行；内存文本仍保留。")
            }
        }
    }
}

#[cfg(test)]
mod tests;
