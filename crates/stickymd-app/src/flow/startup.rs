//! What a failure during the window's startup means. Two types, so the shell can neither end the
//! startup over something the note survives nor go on after something it does not.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-failure-paths

use std::fmt;

use crate::ports::HostFailure;

/// The window cannot be used or placed safely. The startup ends: the reason is shown, the tray is
/// gone first and the process reports failure. The note was not touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupStop {
    /// The platform's text for a window it could not create.
    Window(String),
    /// The platform's text for a drawing surface it could not create.
    Surface(String),
    NoDisplay,
    /// A tool window without this identity has no way back once hidden; `shown` is whether it
    /// was already on screen.
    ToolIdentity {
        shown: bool,
        cause: HostFailure,
    },
}

impl fmt::Display for StartupStop {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(cause) => {
                write!(formatter, "无法创建窗口；StickyMD 将停止启动。\n\n{cause}")
            }
            Self::Surface(cause) => {
                write!(
                    formatter,
                    "无法创建绘制表面；StickyMD 将停止启动。\n\n{cause}"
                )
            }
            Self::NoDisplay => formatter.write_str("没有可用的显示器；无法安全放置窗口。"),
            Self::ToolIdentity {
                shown: false,
                cause,
            } => write!(
                formatter,
                "无法建立安全的工具窗口身份；StickyMD 不会显示无恢复入口的窗口。\n\n{cause}"
            ),
            Self::ToolIdentity { shown: true, cause } => write!(
                formatter,
                "窗口显示后无法保持安全的工具窗口身份；StickyMD 将停止启动。\n\n{cause}"
            ),
        }
    }
}

/// The note stays usable without this. The startup goes on and the status line says why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupDegraded {
    Tray(HostFailure),
    Opacity(HostFailure),
    Topmost(HostFailure),
}

impl StartupDegraded {
    /// Whether the user is also interrupted with a dialog. Only a missing tray changes what the
    /// next action does: closing the window now ends the program instead of hiding it.
    pub fn needs_dialog(&self) -> bool {
        matches!(self, Self::Tray(_))
    }
}

impl fmt::Display for StartupDegraded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tray(cause) => write!(
                formatter,
                "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\n{cause}"
            ),
            Self::Opacity(cause) => write!(formatter, "无法应用窗口透明度：{cause}"),
            Self::Topmost(cause) => write!(formatter, "无法应用窗口置顶状态：{cause}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::FailureKind;

    fn cause() -> HostFailure {
        HostFailure::new(FailureKind::Failed, "the cause")
    }

    #[test]
    fn every_stop_says_why_with_the_platform_text() {
        for (stop, expected) in [
            (
                StartupStop::Window("the cause".into()),
                "无法创建窗口；StickyMD 将停止启动。\n\nthe cause",
            ),
            (
                StartupStop::Surface("the cause".into()),
                "无法创建绘制表面；StickyMD 将停止启动。\n\nthe cause",
            ),
            (
                StartupStop::ToolIdentity {
                    shown: false,
                    cause: cause(),
                },
                "无法建立安全的工具窗口身份；StickyMD 不会显示无恢复入口的窗口。\n\nthe cause",
            ),
            (
                StartupStop::ToolIdentity {
                    shown: true,
                    cause: cause(),
                },
                "窗口显示后无法保持安全的工具窗口身份；StickyMD 将停止启动。\n\nthe cause",
            ),
        ] {
            assert_eq!(stop.to_string(), expected);
        }
        assert_eq!(
            StartupStop::NoDisplay.to_string(),
            "没有可用的显示器；无法安全放置窗口。"
        );
    }

    #[test]
    fn only_a_missing_tray_interrupts_a_startup_that_goes_on() {
        assert!(StartupDegraded::Tray(cause()).needs_dialog());
        assert!(!StartupDegraded::Opacity(cause()).needs_dialog());
        assert!(!StartupDegraded::Topmost(cause()).needs_dialog());
    }

    #[test]
    fn a_degraded_startup_names_what_is_missing() {
        assert_eq!(
            StartupDegraded::Opacity(cause()).to_string(),
            "无法应用窗口透明度：the cause"
        );
        assert_eq!(
            StartupDegraded::Topmost(cause()).to_string(),
            "无法应用窗口置顶状态：the cause"
        );
        assert_eq!(
            StartupDegraded::Tray(cause()).to_string(),
            "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\nthe cause"
        );
    }
}
