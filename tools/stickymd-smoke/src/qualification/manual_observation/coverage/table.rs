//! The obligation data: one entry per `(source, assertion, parameter)`, in source order.
//! Edited together with the documents it decomposes (the digests in `tests` say when).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#manual-observation-log

use super::{Obligation, Target};
use crate::qualification::exact_checks;
use crate::unit_owners::{Owner, OwnerTarget};

const fn o(
    source: &'static str,
    assertion: &'static str,
    param: &'static str,
    target: Target,
) -> Obligation {
    Obligation {
        source,
        assertion,
        param,
        target,
    }
}

const fn m(case: &'static str, scenario: &'static str) -> Target {
    Target::Manual { case, scenario }
}

const fn exact(case: &'static str, checks: &'static [&'static str]) -> Target {
    Target::Exact { case, checks }
}

/// A unit test of the `stickymd-win` binary; owned once the headless receipt verifies it.
const fn win_test(path: &'static str) -> Target {
    Target::Test(Owner {
        package: "stickymd-win",
        target: OwnerTarget::Bin("stickymd-win"),
        path,
    })
}

const fn gap(planned: &'static str) -> Target {
    Target::Gap { planned }
}

// Obligations that existing automation already exercises are owned through module receipts of
// the executed tests, which land with the exact / automated extensions (C2). Until then they
// are gaps that name the entry, so readiness lists them instead of trusting prose.
const C2_SCROLL_SHELL: &str = "C2 绑定模块收据：phase-05 2026-09-25 滚动条维护节 `cargo test -p stickymd-render -p stickymd-win --locked scrollbar`（app 几何、绘制与 Preview worker 回执）";
const C2_SCROLL_RENDER: &str = "C2 绑定模块收据：phase-03 2026-09-25 滚动条维护节 `cargo test -p stickymd-render -p stickymd-win --locked scrollbar`（render projection）";
const C2_OPACITY_REDUCER: &str = "C2 绑定模块收据：P08-A07 `phase8_` 透明度 reducer 与输入测试";
const C2_ISOLATION_RUNTIME: &str =
    "C2 绑定模块收据：P08-A11 copied-Release `-Runtime`（scenario `WindowShell`）的进程与窗口隔离";
/// The reducer test checks that a Preview intent does not persist, not that dragging produces
/// only Preview intents.
const UNIT_OPACITY_DRAG: &str = "C2b 绑定执行清单：app::controls 的指针判定经偏好 reducer 拖动只产生不持久化的预览、有效松开至多一次提交；实际配置提交与 winit 事件接线待 exact 层";

const G4_DOCK_COMPOSITION: &str = "G4-06: Right/Top dock 下 composition 不收起";
const G4_OPACITY_COMMIT: &str = "G4-02: 透明度松开滑块 / Enter / 失焦三种提交";
const G4_TOPMOST_SYNC: &str = "G4-01: 置顶在托盘与窗口间双向同步，不受临时 sensor topmost 影响";
const G4_QUIT_SAVE_FAILURE: &str = "G4-01: 退出保存失败时取消退出、保持运行、标题状态";
/// G4-01 hides the paper with Close; the tray item is only shown to exist.
const G4_TRAY_HIDE: &str =
    "G4-01: 窗口可见时经托盘菜单 Hide 隐藏（现只经 Close 隐藏，菜单项只核对存在）";
const G4_ESC: &str = "G4-02: 真实键盘 Esc 分支";
/// The Esc decision with each guard alone (asset reconcile, input disabled while quitting) and
/// under every modifier; USER 2026-10-15 approved unit-only coverage of these Esc rows.
const ESC_GUARDS_TEST: &str = "app::escape::tests::each_guard_alone_ignores_esc";
const ESC_MODIFIERS_TEST: &str = "app::escape::tests::modifiers_never_change_esc";
/// The existing scrollbar tests check the app's geometry and painting, the render projection
/// and the Preview worker's receipts; no test drives the app runtime that composes the bars,
/// hovers, drags, pages, ends a drag or admits a Preview receipt.
const UNIT_SCROLL_RUNTIME: &str = "C2b 绑定模块收据：app::scrollbar 与 app::preview_runtime 的手势、栏可用性与回执路由判定；winit 事件接线（hover、拖动、翻页、拖动结束、回执应用）待 exact 层";
const G4_DRAG_OUTSIDE: &str = "G4: 真实鼠标拖出窗口后完成滚动条拖动";
const UNIT_QUIT_STATE: &str = "C2b 绑定模块收据：app::title::tests 与 flow::persistence::tests 的退出已取消状态与载入外部解除";

// Exact owners name the check labels that prove them (`exact_checks`); an obligation without a
// producer, profile or Pin parameter needs every variant.
/// P08-M01 timing: the focused guard, both collapse delays, the sensor reveal and re-expansion on the left edge.
const DOCK_TIMING_LEFT: &[&str] = &[
    "dock-collapse-delay:edge=left,pin=off",
    "dock-focus-guard:edge=left,pin=off",
    "dock-leave-delay:edge=left,pin=off",
    "dock-reexpand:edge=left,pin=off",
    "dock-reveal-delay:edge=left,pin=off",
];
/// P08-M02 timing on the right edge, with Pin off and on.
const DOCK_TIMING_RIGHT: &[&str] = &[
    "dock-collapse-delay:edge=right,pin=off",
    "dock-collapse-delay:edge=right,pin=on",
    "dock-focus-guard:edge=right,pin=off",
    "dock-focus-guard:edge=right,pin=on",
    "dock-leave-delay:edge=right,pin=off",
    "dock-leave-delay:edge=right,pin=on",
    "dock-reexpand:edge=right,pin=off",
    "dock-reexpand:edge=right,pin=on",
    "dock-reveal-delay:edge=right,pin=off",
    "dock-reveal-delay:edge=right,pin=on",
];
/// P08-M03 timing on the top edge.
const DOCK_TIMING_TOP: &[&str] = &[
    "dock-collapse-delay:edge=top,pin=off",
    "dock-focus-guard:edge=top,pin=off",
    "dock-leave-delay:edge=top,pin=off",
    "dock-reexpand:edge=top,pin=off",
    "dock-reveal-delay:edge=top,pin=off",
];
/// A whole dock cycle (snap plus timing) on the left edge.
const DOCK_LEFT: &[&str] = &[
    "dock-collapse-delay:edge=left,pin=off",
    "dock-focus-guard:edge=left,pin=off",
    "dock-leave-delay:edge=left,pin=off",
    "dock-reexpand:edge=left,pin=off",
    "dock-reveal-delay:edge=left,pin=off",
    "dock-snap:edge=left,pin=off",
];
/// A whole dock cycle on the right edge, with Pin off and on.
const DOCK_RIGHT: &[&str] = &[
    "dock-collapse-delay:edge=right,pin=off",
    "dock-collapse-delay:edge=right,pin=on",
    "dock-focus-guard:edge=right,pin=off",
    "dock-focus-guard:edge=right,pin=on",
    "dock-leave-delay:edge=right,pin=off",
    "dock-leave-delay:edge=right,pin=on",
    "dock-reexpand:edge=right,pin=off",
    "dock-reexpand:edge=right,pin=on",
    "dock-reveal-delay:edge=right,pin=off",
    "dock-reveal-delay:edge=right,pin=on",
    "dock-snap:edge=right,pin=off",
    "dock-snap:edge=right,pin=on",
];
/// A whole dock cycle on the top edge.
const DOCK_TOP: &[&str] = &[
    "dock-collapse-delay:edge=top,pin=off",
    "dock-focus-guard:edge=top,pin=off",
    "dock-leave-delay:edge=top,pin=off",
    "dock-reexpand:edge=top,pin=off",
    "dock-reveal-delay:edge=top,pin=off",
    "dock-snap:edge=top,pin=off",
];
/// Real composition keeps a left-docked paper open, for both IMEs.
const COMPOSITION_LEFT: &[&str] = &[
    "ime-composition-no-collapse:edge=left,ime=pinyin",
    "ime-composition-no-collapse:edge=left,ime=wetype",
];
/// An IME commit and Undo at 40% opacity, for both IMEs.
const OPACITY_40_IME: &[&str] = &[
    "ime-commit-undo:ime=pinyin,opacity=40",
    "ime-commit-undo:ime=wetype,opacity=40",
];
/// The whole G4-06 functional matrix of Microsoft Pinyin.
const IME_PINYIN: &[&str] = &[
    "ime-cancel-non-mutating:ime=pinyin",
    "ime-commit-undo:edge=left,ime=pinyin",
    "ime-commit-undo:ime=pinyin,mode=source",
    "ime-commit-undo:ime=pinyin,mode=split",
    "ime-commit-undo:ime=pinyin,opacity=40",
    "ime-composition-no-collapse:edge=left,ime=pinyin",
    "ime-find-no-replace:ime=pinyin",
    "ime-find-query-selects-term:ime=pinyin",
    "ime-focus-loss-keeps-text:ime=pinyin",
    "ime-preedit-not-durable:ime=pinyin",
    "ime-profile-activated:ime=pinyin",
    "ime-refocus-commit-undo:ime=pinyin",
    "ime-replace-commit:ime=pinyin",
    "ime-replace-undo:ime=pinyin",
    "ime-selection-commit-one-undo:ime=pinyin",
];
/// The whole G4-06 functional matrix of WeType.
const IME_WETYPE: &[&str] = &[
    "ime-cancel-non-mutating:ime=wetype",
    "ime-commit-undo:edge=left,ime=wetype",
    "ime-commit-undo:ime=wetype,mode=source",
    "ime-commit-undo:ime=wetype,mode=split",
    "ime-commit-undo:ime=wetype,opacity=40",
    "ime-composition-no-collapse:edge=left,ime=wetype",
    "ime-find-no-replace:ime=wetype",
    "ime-find-query-selects-term:ime=wetype",
    "ime-focus-loss-keeps-text:ime=wetype",
    "ime-preedit-not-durable:ime=wetype",
    "ime-profile-activated:ime=wetype",
    "ime-refocus-commit-undo:ime=wetype",
    "ime-replace-commit:ime=wetype",
    "ime-replace-undo:ime=wetype",
    "ime-selection-commit-one-undo:ime=wetype",
];
/// Show, from the exact tray menu, restores the hidden paper with its retained text.
const TRAY_SHOW: &[&str] = &[
    "tray-menu-items",
    "tray-show-restores-window",
    "tray-show-retains-source",
];
/// Quit, from the exact tray menu, saves the latest text and exits.
const TRAY_QUIT: &[&str] = &[
    "tray-menu-items",
    "tray-quit-exits",
    "tray-quit-saves-latest",
];
/// A second launch through a junction wakes the hidden primary and exits.
const WAKE_HIDDEN: &[&str] = &[
    "close-hides-window",
    "junction-second-instance-exits",
    "junction-wakes-primary",
];

pub(super) const OBLIGATIONS: &[Obligation] = &[
    // P08-M01..M03: dock visual at each DPI, timing on the primary monitor, undock.
    o(
        "P08-M01",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "left-dock"),
    ),
    o(
        "P08-M01",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "left-dock"),
    ),
    o("P08-M01", "timing", "", exact("G4-02", DOCK_TIMING_LEFT)),
    o("P08-M01", "undock-16dip", "", m("P12-M11", "undock-16dip")),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "right-dock"),
    ),
    o(
        "P08-M02",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "right-dock"),
    ),
    o("P08-M02", "timing", "", exact("G4-02", DOCK_TIMING_RIGHT)),
    o("P08-M02", "undock-16dip", "", m("P12-M12", "undock-16dip")),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=100",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=125",
        m("P12-M38", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=150",
        m("P12-M39", "top-dock"),
    ),
    o(
        "P08-M03",
        "dock-visual",
        "dpi=200",
        m("P12-M40", "top-dock"),
    ),
    o("P08-M03", "timing", "", exact("G4-02", DOCK_TIMING_TOP)),
    // P08-M04: IME function is exact; the candidate window is visual.
    o(
        "P08-M04",
        "ime-input",
        "ime=pinyin,mode=source",
        exact("G4-06", &["ime-commit-undo:ime=pinyin,mode=source"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=pinyin,mode=split",
        exact("G4-06", &["ime-commit-undo:ime=pinyin,mode=split"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=wetype,mode=source",
        exact("G4-06", &["ime-commit-undo:ime=wetype,mode=source"]),
    ),
    o(
        "P08-M04",
        "ime-input",
        "ime=wetype,mode=split",
        exact("G4-06", &["ime-commit-undo:ime=wetype,mode=split"]),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=left",
        exact("G4-06", COMPOSITION_LEFT),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=right",
        gap(G4_DOCK_COMPOSITION),
    ),
    o(
        "P08-M04",
        "composition-no-collapse",
        "edge=top",
        gap(G4_DOCK_COMPOSITION),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=40",
        exact("G4-06", OPACITY_40_IME),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=96",
        gap("G4-06: 96% 透明度下 IME 提交"),
    ),
    o(
        "P08-M04",
        "opacity-ime-commit",
        "opacity=100",
        gap("G4-06: 100% 透明度下 IME 提交"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=left",
        m("P12-M01", "left-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=right",
        m("P12-M01", "right-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=pinyin,edge=top",
        m("P12-M01", "top-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=left",
        m("P12-M02", "left-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=right",
        m("P12-M02", "right-dock"),
    ),
    o(
        "P08-M04",
        "candidate-visual",
        "ime=wetype,edge=top",
        m("P12-M02", "top-dock"),
    ),
    // P08-M05: tray lifecycle.
    o(
        "P08-M05",
        "tray-menu",
        "item=show",
        exact("G4-01", TRAY_SHOW),
    ),
    o("P08-M05", "tray-menu", "item=hide", gap(G4_TRAY_HIDE)),
    o(
        "P08-M05",
        "tray-menu",
        "item=quit",
        exact("G4-01", TRAY_QUIT),
    ),
    o("P08-M05", "failed-save-quit", "", gap(G4_QUIT_SAVE_FAILURE)),
    o(
        "P08-M05",
        "failed-save-visible-error",
        "",
        m("P12-M05", "exit-save-failure"),
    ),
    o("P08-M05", "topmost-tray-sync", "", gap(G4_TOPMOST_SYNC)),
    // P08-M06: opacity.
    o(
        "P08-M06",
        "layers-visual",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-M06",
        "layers-visual",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=slider-release",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=enter",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "P08-M06",
        "commit-modes",
        "commit=blur",
        gap(G4_OPACITY_COMMIT),
    ),
    // P08-M07: theme.
    o(
        "P08-M07",
        "theme-visual",
        "theme=light",
        m("P12-M23", "light"),
    ),
    o(
        "P08-M07",
        "theme-visual",
        "theme=dark",
        m("P12-M23", "dark"),
    ),
    o(
        "P08-M07",
        "theme-visual",
        "theme=system",
        m("P12-M23", "system"),
    ),
    o(
        "P08-M07",
        "live-switch",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-M07",
        "live-switch",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    // P08-M08: second launch.
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=hidden",
        m("P12-M05", "second-launch-hidden"),
    ),
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=minimized",
        m("P12-M05", "second-launch-minimized"),
    ),
    o(
        "P08-M08",
        "visible-restore-and-focus",
        "state=docked",
        m("P12-M05", "second-launch-docked"),
    ),
    o(
        "P08-M08",
        "identity-wake",
        "state=hidden",
        exact("G4-05", WAKE_HIDDEN),
    ),
    o(
        "P08-M08",
        "identity-wake",
        "state=docked",
        gap("G4-05: dock 收起状态下第二次启动"),
    ),
    // P08-M09: two portable copies.
    o(
        "P08-M09",
        "process-window-isolation",
        "",
        gap(C2_ISOLATION_RUNTIME),
    ),
    o(
        "P08-M09",
        "note-config-runtime-isolation",
        "",
        gap("P08-A11: 运行时双向 note/config 不串写"),
    ),
    o(
        "P08-M09",
        "quit-isolation",
        "",
        gap("P08-A11: 退出一份不影响另一份"),
    ),
    o(
        "P08-M09",
        "tray-identity",
        "",
        m("P12-M45", "tray-identity"),
    ),
    o(
        "P08-M09",
        "independent-show-hide",
        "",
        m("P12-M45", "independent-show-hide"),
    ),
    // P08-M10: monitor topology.
    o(
        "P08-M10",
        "arrangement",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=right",
        m("P12-M35", "secondary-right"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    o(
        "P08-M10",
        "arrangement",
        "side=below",
        m("P12-M35", "secondary-below"),
    ),
    o(
        "P08-M10",
        "primary-switch",
        "",
        m("P12-M35", "primary-switch"),
    ),
    o(
        "P08-M10",
        "disconnect-reconnect",
        "step=disconnect",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o(
        "P08-M10",
        "disconnect-reconnect",
        "step=reconnect",
        m("P12-M37", "reconnect-monitor"),
    ),
    o("P08-M10", "sleep-resume", "", m("P12-M41", "sleep-resume")),
    o("P08-M10", "rdp", "", m("P12-M42", "rdp-reconnect")),
    // P08-M11: mixed DPI and negative coordinates.
    o(
        "P08-M11",
        "mixed-dpi-dock",
        "edge=left",
        m("P12-M11", "left-dock-mixed-dpi"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-dock",
        "edge=right",
        m("P12-M12", "right-dock-mixed-dpi"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=drag",
        m("P12-M36", "drag"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=dock",
        m("P12-M36", "dock"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=ime",
        m("P12-M36", "ime"),
    ),
    o(
        "P08-M11",
        "mixed-dpi-cross-monitor",
        "what=preview",
        m("P12-M36", "preview"),
    ),
    o(
        "P08-M11",
        "negative-coordinates",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-M11",
        "negative-coordinates",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    // P08-M12: Windows 11 shell; every manual case is observed on both target roles.
    o(
        "P08-M12",
        "supported-builds",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-M12",
        "corners-shadow",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-M12",
        "topmost-button",
        "",
        m("P12-M46", "topmost-button"),
    ),
    o("P08-M12", "control-fade", "", m("P12-M46", "control-fade")),
    o(
        "P08-M12",
        "accessibility",
        "",
        Target::Retired {
            decision: "RETIRE-P08-M12-ACCESSIBILITY",
        },
    ),
    // P08-M13..M15: inherited gates.
    o(
        "P08-M13",
        "ime-functional",
        "",
        exact("G4-06", exact_checks::G4_06),
    ),
    o(
        "P08-M13",
        "ime-visual",
        "ime=pinyin",
        m("P12-M01", "pinyin-100"),
    ),
    o(
        "P08-M13",
        "ime-visual",
        "ime=wetype",
        m("P12-M02", "wetype-100"),
    ),
    o(
        "P08-M14",
        "preview-selection-visual",
        "",
        m("P12-M24", "representative-markdown"),
    ),
    o(
        "P08-M14",
        "math-visual",
        "variant=formulas",
        m("P12-M25", "formulas"),
    ),
    o(
        "P08-M14",
        "math-visual",
        "theme=light",
        m("P12-M23", "light"),
    ),
    o("P08-M14", "math-visual", "theme=dark", m("P12-M23", "dark")),
    o("P08-M14", "math-visual", "dpi=100", m("P12-M47", "preview")),
    o("P08-M14", "math-visual", "dpi=125", m("P12-M38", "preview")),
    o("P08-M14", "math-visual", "dpi=150", m("P12-M39", "preview")),
    o("P08-M14", "math-visual", "dpi=200", m("P12-M40", "preview")),
    o(
        "P08-M15",
        "image-quality",
        "",
        m("P12-M26", "image-formats"),
    ),
    o(
        "P08-M15",
        "clipboard-producers",
        "",
        exact("G3-01", exact_checks::G3_01),
    ),
    o(
        "P08-M15",
        "export-dialog",
        "",
        exact("G3-02", exact_checks::G3_02),
    ),
    // G3-03 accepts the old note at every kill point without confirming the edit or the save
    // began, so a run that never reaches saving passes.
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=note-save",
        gap("G3-03: 确认编辑与持久化阶段后在确定性中断点强杀，并以保存未开始的负对照排除空通过"),
    ),
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=asset-write",
        gap("G3-03: 资产写入完成后的确定性强杀"),
    ),
    o(
        "P08-M15",
        "crash-reconciliation",
        "point=trash-move",
        gap("G3-03: 移入 trash 完成后的确定性强杀"),
    ),
    // P08-Dxxx: the definition-of-done rows each M row receipt covers.
    o(
        "P08-D004",
        "shell-visual",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o(
        "P08-D005",
        "rounded-corner",
        "",
        m("P12-M46", "corners-shadow"),
    ),
    o("P08-D006", "shadow", "", m("P12-M46", "corners-shadow")),
    o("P08-D007", "drag", "edge=left", m("P12-M47", "left-dock")),
    o("P08-D007", "drag", "edge=right", m("P12-M47", "right-dock")),
    o("P08-D007", "drag", "edge=top", m("P12-M47", "top-dock")),
    o("P08-D008", "resize", "", m("P12-M46", "edge-corner-resize")),
    o(
        "P08-D010",
        "mode-controls",
        "",
        m("P12-M46", "controls-in-modes"),
    ),
    o(
        "P08-D012",
        "topmost-button",
        "",
        m("P12-M46", "topmost-button"),
    ),
    o("P08-D013", "topmost-tray-sync", "", gap(G4_TOPMOST_SYNC)),
    o(
        "P08-D016",
        "live-theme-change",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-D016",
        "live-theme-change",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-D020",
        "whole-window",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o("P08-D021", "live-preview", "", m("P12-M22", "opacity-40")),
    o(
        "P08-D022",
        "numeric-input",
        "commit=enter",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "P08-D022",
        "numeric-input",
        "commit=blur",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "P08-D027",
        "tray-icon",
        "",
        exact("G4-01", &["tray-icon-present"]),
    ),
    o(
        "P08-D028",
        "tray-menu-items",
        "",
        exact("G4-01", &["tray-menu-items"]),
    ),
    o(
        "P08-D031",
        "alt-f4",
        "",
        gap("G4-01: 真实 Alt+F4 隐藏到托盘"),
    ),
    o("P08-D039", "left-dock", "", exact("G4-02", DOCK_LEFT)),
    o("P08-D040", "right-dock", "", exact("G4-02", DOCK_RIGHT)),
    o("P08-D041", "top-dock", "", exact("G4-02", DOCK_TOP)),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D051",
        "hover-no-steal",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=left",
        exact("G4-06", COMPOSITION_LEFT),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=right",
        gap(G4_DOCK_COMPOSITION),
    ),
    o(
        "P08-D055",
        "composition-no-collapse",
        "edge=top",
        gap(G4_DOCK_COMPOSITION),
    ),
    // G4-06 composes while focused, and focus alone blocks auto-collapse; the composition
    // guard's own wiring (and blur cancelling preedit) has reducer coverage only (P08-A05).
    o(
        "P08-D055",
        "composition-guard-wiring",
        "",
        gap(
            "C2b 绑定模块收据：app::guards 的守卫投影（含 Search 组字）；失焦取消 preedit 的事件接线待 exact 层",
        ),
    ),
    // G4-02 reveals the sensor with a reinforcing window message and no covering window, so
    // it does not prove a covered strip is reachable by the real mouse.
    o(
        "P08-D068",
        "covered-sensor-hover",
        "",
        gap("G4-02: Pin OFF 时被其他窗口覆盖的感应条经真实鼠标悬停展开"),
    ),
    o(
        "P08-D070",
        "hover-click-focus",
        "",
        m("P12-M05", "restore-sensor"),
    ),
    o(
        "P08-D084",
        "disconnect-fallback",
        "",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o("P08-D089", "dpi-ime-rect", "", m("P12-M36", "ime")),
    o(
        "P08-D117",
        "pinyin-functional",
        "",
        exact("G4-06", IME_PINYIN),
    ),
    o("P08-D117", "pinyin-visual", "", m("P12-M01", "pinyin-100")),
    o(
        "P08-D118",
        "wetype-functional",
        "",
        exact("G4-06", IME_WETYPE),
    ),
    o("P08-D118", "wetype-visual", "", m("P12-M02", "wetype-100")),
    o("P08-D119", "light", "", m("P12-M23", "light")),
    o("P08-D120", "dark", "", m("P12-M23", "dark")),
    o(
        "P08-D121",
        "system-live",
        "to=dark",
        m("P12-M23", "system-to-dark"),
    ),
    o(
        "P08-D121",
        "system-live",
        "to=light",
        m("P12-M23", "system-to-light"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "P08-D122",
        "opacity-visual",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o("P08-D123", "math-visual", "", m("P12-M25", "formulas")),
    o(
        "P08-D124",
        "image-visual",
        "",
        m("P12-M26", "image-formats"),
    ),
    o(
        "P08-D125",
        "mode-visual",
        "",
        m("P12-M46", "controls-in-modes"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=100",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=125",
        m("P12-M38", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=150",
        m("P12-M39", "left-dock"),
    ),
    o(
        "P08-D126",
        "left-dock-visual",
        "dpi=200",
        m("P12-M40", "left-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=100",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=125",
        m("P12-M38", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=150",
        m("P12-M39", "right-dock"),
    ),
    o(
        "P08-D127",
        "right-dock-visual",
        "dpi=200",
        m("P12-M40", "right-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=100",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=125",
        m("P12-M38", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=150",
        m("P12-M39", "top-dock"),
    ),
    o(
        "P08-D128",
        "top-dock-visual",
        "dpi=200",
        m("P12-M40", "top-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D129",
        "sensor-visual",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=left",
        m("P12-M47", "left-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=right",
        m("P12-M47", "right-dock"),
    ),
    o(
        "P08-D130",
        "hover-no-focus-visual",
        "edge=top",
        m("P12-M47", "top-dock"),
    ),
    o(
        "P08-D131",
        "tray-visual-menu",
        "",
        m("P12-M05", "restore-tray"),
    ),
    o(
        "P08-D132",
        "export-dialog",
        "",
        exact("G3-02", exact_checks::G3_02),
    ),
    o("P08-D133", "real-dpi", "dpi=125", m("P12-M38", "input")),
    o("P08-D133", "real-dpi", "dpi=150", m("P12-M39", "input")),
    o("P08-D133", "real-dpi", "dpi=200", m("P12-M40", "input")),
    o("P08-D134", "dual-monitor", "", m("P12-M35", "drag")),
    o("P08-D135", "mixed-dpi", "", m("P12-M36", "drag")),
    o(
        "P08-D136",
        "disconnect",
        "",
        m("P12-M37", "disconnect-current-monitor"),
    ),
    o(
        "P08-D137",
        "negative-geometry",
        "side=left",
        m("P12-M43", "secondary-left"),
    ),
    o(
        "P08-D137",
        "negative-geometry",
        "side=above",
        m("P12-M43", "secondary-above"),
    ),
    o("P08-D138", "sleep-resume", "", m("P12-M41", "sleep-resume")),
    o("P08-D139", "rdp", "", m("P12-M42", "rdp-reconnect")),
    // plan 09: Esc, manual collapse, scrollbars, opacity and quit.
    o("plan09:esc-1", "popup-close-revert", "", gap(G4_ESC)),
    o(
        "plan09:esc-1",
        "slider-drag-esc",
        "",
        gap("G4-02: 滑块拖动中 Esc：关闭、回退、之后不提交"),
    ),
    o(
        "plan09:esc-2",
        "recovery-pending",
        "",
        gap("G3-03: 恢复待处理时 Esc 无变化（确定性 fixture 与正向对照）"),
    ),
    o(
        "plan09:esc-2",
        "transient-guards",
        "state=asset-sync",
        win_test(ESC_GUARDS_TEST),
    ),
    o(
        "plan09:esc-2",
        "transient-guards",
        "state=quitting",
        win_test(ESC_GUARDS_TEST),
    ),
    o(
        "plan09:esc-3",
        "composition-cancel",
        "field=editor",
        gap("G4-06: composition 中 Esc 只取消 preedit"),
    ),
    o(
        "plan09:esc-3",
        "composition-cancel",
        "field=search",
        gap("G4-06: Search 字段 composition 中 Esc 只取消 preedit"),
    ),
    o("plan09:esc-4", "search-close", "", gap(G4_ESC)),
    o(
        "plan09:esc-5",
        "preview-selection",
        "selection=present",
        gap(G4_ESC),
    ),
    o(
        "plan09:esc-5",
        "preview-selection",
        "selection=absent",
        gap(G4_ESC),
    ),
    o("plan09:esc-6", "collapse", "state=docked", gap(G4_ESC)),
    o("plan09:esc-6", "collapse", "state=floating", gap(G4_ESC)),
    o("plan09:esc-6", "collapse", "modifier=shift", gap(G4_ESC)),
    o(
        "plan09:esc-6",
        "collapse",
        "modifier=ctrl",
        win_test(ESC_MODIFIERS_TEST),
    ),
    o(
        "plan09:esc-6",
        "collapse",
        "repeat",
        gap("G4-02: 重复 key-down 逐次独立消费"),
    ),
    o(
        "plan09:manual-collapse",
        "collapse-button",
        "",
        gap("G4-02: 真实点击收起按钮"),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=source",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=preview",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-1",
        "per-pane",
        "view=split",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-1",
        "split-sync-switch",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-2",
        "gutter-geometry",
        "",
        gap(C2_SCROLL_SHELL),
    ),
    o(
        "plan09:scroll-2",
        "short-note",
        "part=hide-thumb",
        gap(C2_SCROLL_SHELL),
    ),
    o(
        "plan09:scroll-2",
        "short-note",
        "part=keep-gutter",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-2",
        "gutter-runtime",
        "aspect=hit-test",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-2",
        "gutter-runtime",
        "aspect=content-zoom",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-3",
        "thumb-geometry",
        "",
        gap(C2_SCROLL_SHELL),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-paint",
        "",
        gap(C2_SCROLL_SHELL),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-trigger",
        "trigger=hover",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-3",
        "emphasis-trigger",
        "trigger=drag",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o("plan09:scroll-3", "drag-mapping", "", gap(C2_SCROLL_SHELL)),
    o(
        "plan09:scroll-3",
        "drag-live-update",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-3",
        "ends-reach-document-ends",
        "view=source",
        gap(C2_SCROLL_RENDER),
    ),
    o(
        "plan09:scroll-3",
        "ends-reach-document-ends",
        "view=preview",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o("plan09:scroll-3", "track-hit", "", gap(C2_SCROLL_SHELL)),
    o(
        "plan09:scroll-3",
        "track-page-scroll",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=text",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=generation",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=dirty",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=caret",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=selection",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "viewport-only",
        "state=preedit",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=release",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=blur",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=view-switch",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "drag-exit",
        "end=resize",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-4",
        "drag-outside-window",
        "",
        gap(G4_DRAG_OUTSIDE),
    ),
    o(
        "plan09:scroll-5",
        "source-mapping",
        "",
        gap(C2_SCROLL_RENDER),
    ),
    o(
        "plan09:scroll-5",
        "preview-document-height",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-6",
        "stale-frame-no-thumb",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-6",
        "stale-receipt-identity",
        "",
        gap(C2_SCROLL_SHELL),
    ),
    o(
        "plan09:scroll-6",
        "stale-receipt-guard",
        "",
        gap(UNIT_SCROLL_RUNTIME),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=100",
        m("P12-M47", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=125",
        m("P12-M38", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=150",
        m("P12-M39", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "dpi=200",
        m("P12-M40", "scrollbars"),
    ),
    o(
        "plan09:scroll-7",
        "grab-no-overlap-visual",
        "theme",
        m("P12-M23", "scrollbars"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=40",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=70",
        m("P12-M22", "opacity-70"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=96",
        m("P12-M22", "opacity-96"),
    ),
    o(
        "plan09:opacity-1",
        "whole-window",
        "opacity=100",
        m("P12-M22", "opacity-100"),
    ),
    o(
        "plan09:opacity-2",
        "range-and-input",
        "",
        gap(C2_OPACITY_REDUCER),
    ),
    o(
        "plan09:opacity-3",
        "live-preview",
        "",
        m("P12-M22", "opacity-40"),
    ),
    o(
        "plan09:opacity-3",
        "no-stepwise-write",
        "",
        gap(UNIT_OPACITY_DRAG),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=slider-release",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=enter",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "plan09:opacity-3",
        "commit-modes",
        "commit=blur",
        gap(G4_OPACITY_COMMIT),
    ),
    o(
        "plan09:quit-1",
        "keeps-running",
        "",
        gap(G4_QUIT_SAVE_FAILURE),
    ),
    o(
        "plan09:quit-1",
        "memory-text-kept",
        "",
        gap(G4_QUIT_SAVE_FAILURE),
    ),
    o(
        "plan09:quit-1",
        "visible-error",
        "",
        m("P12-M05", "exit-save-failure"),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=note-save-failure",
        gap(G4_QUIT_SAVE_FAILURE),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=quit-guard",
        gap(UNIT_QUIT_STATE),
    ),
    o(
        "plan09:quit-2",
        "established-only-by-save-failure",
        "cause=paste-unconverged",
        gap(UNIT_QUIT_STATE),
    ),
    o(
        "plan09:quit-2",
        "cleared-after-event-when-barrier-false",
        "",
        gap(UNIT_QUIT_STATE),
    ),
    o(
        "plan09:quit-2",
        "external-load-releases-rebuild",
        "",
        gap(UNIT_QUIT_STATE),
    ),
    o(
        "plan09:quit-3",
        "title-priority",
        "",
        gap("C2b 绑定模块收据：app::title::tests 的标题优先级与唯一原生写入者源码守卫"),
    ),
];
