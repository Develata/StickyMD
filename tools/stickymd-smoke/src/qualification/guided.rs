//! Guided human sessions map compact observations back to exact Phase 12 case IDs.

use crate::cli::GuidedSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GuidedStep {
    pub(super) session: GuidedSession,
    pub(super) id: &'static str,
    pub(super) case_ids: &'static [&'static str],
    pub(super) action: &'static str,
    pub(super) expected: &'static str,
}

pub(super) const STEPS: &[GuidedStep] = &[
    step(
        GuidedSession::G1,
        "G1-01",
        &["P12-M01"],
        "完成 Microsoft Pinyin 连续中文、中英混输、selection composition、cancel 与一次 Undo",
        "候选框跟 caret，commit 一次撤销，cancel 不污染 canonical/undo",
    ),
    step(
        GuidedSession::G1,
        "G1-02",
        &["P12-M02"],
        "用 WeChat Input Method 重复真实 IME 矩阵",
        "行为与 Microsoft Pinyin 同等正确；环境缺失记录 NOT TESTED",
    ),
    step(
        GuidedSession::G1,
        "G1-03",
        &["P12-M21"],
        "在 Source/Preview/Split 实测 50/100/300% zoom、滚轮缩放与 reset",
        "字体、caret、selection、公式和图片缩放正确且流畅",
    ),
    step(
        GuidedSession::G1,
        "G1-04",
        &["P12-M22"],
        "把 opacity 调到 40 并输入、预览和使用 IME",
        "整窗 alpha、候选框、caret、焦点和控件均可用",
    ),
    step(
        GuidedSession::G1,
        "G1-05",
        &["P12-M24"],
        "用代表性 Markdown 观察 Preview 并选择文字/打开允许链接",
        "标题、列表、表格、引用、代码、链接和 selection 视觉正确",
    ),
    step(
        GuidedSession::G1,
        "G1-06",
        &["P12-M25"],
        "用正确和错误公式观察 Preview",
        "RaTeX 视觉正确；错误公式保留原文并显示错误态",
    ),
    step(
        GuidedSession::G1,
        "G1-07",
        &["P12-M26"],
        "滚动检查 PNG/JPEG/WebP/GIF、超限图片和 viewport 下方图片",
        "方向、缩放、lazy display、placeholder 与滚动后的图片显示正确",
    ),
    step(
        GuidedSession::G1,
        "G1-08",
        &["P12-M27"],
        "使用 Shift+Insert、Ctrl+Insert、Shift+Delete 操作文本并 Undo/Redo",
        "传统剪贴板语义与 Undo/Redo 正确",
    ),
    step(
        GuidedSession::G1,
        "G1-09",
        &["P12-M31"],
        "转换真实 \\(x\\) 与多行 \\[y\\]，再执行一次 Undo",
        "转换为 dollar delimiters；literal 不变；一次 Undo 全部恢复",
    ),
    step(
        GuidedSession::G2,
        "G2-01",
        &["P12-M03"],
        "观察 Windows taskbar",
        "StickyMD 不出现在任务栏",
    ),
    step(
        GuidedSession::G2,
        "G2-02",
        &["P12-M04"],
        "打开 Alt+Tab switcher",
        "StickyMD 不出现在 Alt+Tab 列表",
    ),
    step(
        GuidedSession::G2,
        "G2-03",
        &["P12-M05"],
        "聚焦后 Alt+Tab 离开，再经点击/托盘/传感区恢复并输入",
        "away、focus restore 与 IME 正常",
    ),
    step(
        GuidedSession::G2,
        "G2-04",
        &["P12-M06", "P12-M07", "P12-M08"],
        "检查 tray 菜单，关闭纸张到 tray，再从 tray 显示",
        "菜单受限；同一窗口/文本保留并可立即输入",
    ),
    step(
        GuidedSession::G2,
        "G2-05",
        &["P12-M10", "P12-M16"],
        "Top dock，失焦收起，再由 3 DIP sensor hover 展开/离开",
        "Top-only dock 与 700/100/500 ms 时序符合合同",
    ),
    step(
        GuidedSession::G2,
        "G2-06",
        &["P12-M11", "P12-M14"],
        "Left dock 并实测 24 DIP capture threshold",
        "Left dock 可用；阈值内吸附、阈值外不吸附",
    ),
    step(
        GuidedSession::G2,
        "G2-07",
        &["P12-M12", "P12-M15"],
        "Right dock，并在角落/等距位置实测 nearest-edge/tie",
        "Right dock 与 reducer 最近边/tie 结果稳定",
    ),
    step(
        GuidedSession::G2,
        "G2-08",
        &["P12-M13"],
        "把窗口拖向底边",
        "不进入 Bottom dock",
    ),
    step(
        GuidedSession::G2,
        "G2-09",
        &["P12-M17"],
        "Docked 时分别 Pin ON/OFF 后失焦并重复 sensor 流程",
        "Pin 与 auto-hide 正交，时序不变",
    ),
    step(
        GuidedSession::G2,
        "G2-10",
        &["P12-M18", "P12-M19", "P12-M20"],
        "在 220x120 分别操作 Source/Preview/Split",
        "三种模式均可用且几何可恢复",
    ),
    step(
        GuidedSession::G2,
        "G2-11",
        &["P12-M23"],
        "实测 Light/Dark/System 与运行时系统主题切换",
        "背景、文字、公式、图片和控件一致更新",
    ),
    step(
        GuidedSession::G2,
        "G2-12",
        &["P12-M09"],
        "确认保存后从 tray 执行退出",
        "保存完成、worker join 后进程退出",
    ),
    step(
        GuidedSession::G3,
        "G3-01",
        &["P12-M28"],
        "从 Explorer、Snipping Tool 与可用 browser 粘贴真实图片",
        "格式优先级、文件写入、Markdown 插入与 Undo 原子性正确",
    ),
    step(
        GuidedSession::G3,
        "G3-02",
        &["P12-M29"],
        "执行 Ctrl+Shift+S native Export dialog",
        "原生对话框、路径重写、图片复制正确且 active note 不切换",
    ),
    step(
        GuidedSession::G3,
        "G3-03",
        &["P12-M30"],
        "在保存窗口附近强杀复制目录中的 Release EXE 并重启",
        "note 完整或 tmp 可恢复；没有 half UTF-8；证据未被静默删除",
    ),
    step(
        GuidedSession::G3,
        "G3-04",
        &["P12-M32"],
        "完成真实 user asset edit/undo/redo/GC/export/quit/restart",
        "非 managed 用户文件从未被自动移动或删除",
    ),
    step(
        GuidedSession::G3,
        "G3-05",
        &["P12-M33"],
        "放置 managed-looking fake file 并完成 GC/restart",
        "无 ownership evidence 的伪 managed 文件不被删除",
    ),
];

const fn step(
    session: GuidedSession,
    id: &'static str,
    case_ids: &'static [&'static str],
    action: &'static str,
    expected: &'static str,
) -> GuidedStep {
    GuidedStep {
        session,
        id,
        case_ids,
        action,
        expected,
    }
}

pub(super) fn session_for_case(case_id: &str) -> Option<GuidedSession> {
    STEPS
        .iter()
        .find(|step| step.case_ids.contains(&case_id))
        .map(|step| step.session)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{STEPS, session_for_case};

    #[test]
    fn guided_steps_map_each_tier_a_case_at_most_once() {
        let mut observed = BTreeSet::new();
        for step in STEPS {
            assert!(!step.case_ids.is_empty());
            for case in step.case_ids {
                assert!(observed.insert(*case), "duplicate guided case {case}");
                assert_eq!(session_for_case(case), Some(step.session));
            }
        }
        assert_eq!(observed.len(), 33);
    }
}
