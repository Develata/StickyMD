//! The Phase 12 manual cases: tier, session and the scenarios a PASS must cover. The case
//! set equals the `Manual` rows of `docs/acceptance-cases/phase-12.md` (tested), and the
//! recorder, the reduction and the waiver checks all read this one table.

use crate::cli::ManualSession;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(in crate::qualification) enum Tier {
    A,
    B,
    C,
}

impl Tier {
    pub(in crate::qualification) const fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

/// The note a recorder-launched instance starts with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum Fixture {
    /// An empty note, for input-method cases.
    Blank,
    /// The rendering stress note with every image format and an over-limit image.
    Rendering,
}

impl Fixture {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Blank => "blank",
            Self::Rendering => "rendering",
        }
    }
}

/// What the observed StickyMD instance must be.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::qualification) enum InstanceRule {
    /// The recorder extracts the candidate, seeds the fixture and starts the EXE for each
    /// scenario; a PASS needs that process still running as the only StickyMD.
    Launched(Fixture),
    /// As `Launched`, across a display disconnect, sleep or RDP reconnect: the original
    /// process and note must survive, never restarted or re-seeded.
    Preserved(Fixture),
    /// The Clean VM first run happens before the tool is copied in, so the observation is
    /// the observer's declaration (`FIRST_RUN_DECLARATIONS`) with no recorded instance.
    FirstRunDeclared,
    /// Two portable copies in separate program directories, both launched by the recorder;
    /// a PASS needs exactly those two processes running (plan 11, P12-M45 only).
    TwoCopies(Fixture),
}

impl InstanceRule {
    fn canonical(self) -> String {
        match self {
            Self::Launched(fixture) => format!("launched:{}", fixture.as_str()),
            Self::Preserved(fixture) => format!("preserved:{}", fixture.as_str()),
            Self::FirstRunDeclared => "first-run-declared".to_owned(),
            Self::TwoCopies(fixture) => format!("two-copies:{}", fixture.as_str()),
        }
    }
}

/// Declared parameters a `FirstRunDeclared` observation must carry; the last one must be
/// `yes`, stating that nothing changed between the first run and the later capture.
pub(in crate::qualification) const FIRST_RUN_DECLARATIONS: [&str; 3] =
    ["vm-baseline", "package-source", "unchanged-since-first-run"];

#[derive(Debug, Eq, PartialEq)]
pub(in crate::qualification) struct Case {
    pub(in crate::qualification) id: &'static str,
    pub(in crate::qualification) tier: Tier,
    pub(in crate::qualification) session: ManualSession,
    /// The `phase-12.md` action and expected columns, verbatim (tested), so a recorder
    /// without the repository shows the same instructions.
    pub(in crate::qualification) action: &'static str,
    pub(in crate::qualification) expected: &'static str,
    pub(in crate::qualification) instance: InstanceRule,
    /// Scenario IDs, unique within the case; a case passes only when every one passes.
    pub(in crate::qualification) scenarios: &'static [&'static str],
}

use Fixture::{Blank, Rendering};
use InstanceRule::{FirstRunDeclared, Launched, Preserved, TwoCopies};
use ManualSession::{M1, M2, M3, M4, M5};
use Tier::{A, B, C};

pub(in crate::qualification) const CASES: &[Case] = &[
    Case {
        id: "P12-M01",
        tier: A,
        session: M1,
        action: "观察 Microsoft Pinyin 候选窗位置、遮挡、字体、动画及 DPI 视觉质量，含左/右/上 dock",
        expected: "功能矩阵由 Phase 14 G4-06 自动化；本项只判候选窗视觉",
        instance: Launched(Blank),
        scenarios: &[
            "pinyin-100",
            "pinyin-150",
            "pinyin-200",
            "left-dock",
            "right-dock",
            "top-dock",
        ],
    },
    Case {
        id: "P12-M02",
        tier: A,
        session: M1,
        action: "观察 WeChat Input Method / WeType 候选窗位置、遮挡、字体、动画及 DPI 视觉质量，含左/右/上 dock",
        expected: "功能矩阵由 Phase 14 G4-06 自动化；环境缺失必须记录 NOT TESTED",
        instance: Launched(Blank),
        scenarios: &[
            "wetype-100",
            "wetype-150",
            "wetype-200",
            "left-dock",
            "right-dock",
            "top-dock",
        ],
    },
    Case {
        id: "P12-M05",
        tier: A,
        session: M2,
        action: "聚焦 StickyMD 后 Alt+Tab 离开，再点击/托盘/传感区（悬停展开后点击）恢复并输入；从隐藏/最小化/dock 收起状态在 Explorer 中第二次启动；退出时保存失败",
        expected: "切换 away 正常；托盘图标与菜单可辨识；恢复焦点和 IME 正常；第二次启动使原窗口可见恢复并取得焦点；退出保存失败时显示明确错误并保持运行",
        instance: Launched(Rendering),
        scenarios: &[
            "restore-click",
            "restore-tray",
            "restore-sensor",
            "second-launch-hidden",
            "second-launch-minimized",
            "second-launch-docked",
            "exit-save-failure",
        ],
    },
    Case {
        id: "P12-M11",
        tier: A,
        session: M2,
        action: "把窗口拖到左边缘，再拖离超过 16 DIP",
        expected: "Left dock 行为正确，混合 DPI 下感应条仍可用；拖离后为 Floating",
        instance: Launched(Rendering),
        scenarios: &["left-dock-mixed-dpi", "undock-16dip"],
    },
    Case {
        id: "P12-M12",
        tier: A,
        session: M2,
        action: "把窗口拖到右边缘，再拖离超过 16 DIP",
        expected: "Right dock 行为正确，混合 DPI 下感应条仍可用；拖离后为 Floating",
        instance: Launched(Rendering),
        scenarios: &["right-dock-mixed-dpi", "undock-16dip"],
    },
    Case {
        id: "P12-M18",
        tier: A,
        session: M2,
        action: "220×120 Source 模式输入与滚动",
        expected: "内容可用、caret/selection/IME 不被工具栏遮挡",
        instance: Launched(Rendering),
        scenarios: &["compact-source"],
    },
    Case {
        id: "P12-M19",
        tier: A,
        session: M2,
        action: "220×120 Preview 模式滚动/选择/链接",
        expected: "viewport、selection 与控制区可用",
        instance: Launched(Rendering),
        scenarios: &["compact-preview"],
    },
    Case {
        id: "P12-M20",
        tier: A,
        session: M2,
        action: "220×120 Split 模式输入和两栏滚动",
        expected: "两栏均可操作，无不可恢复几何",
        instance: Launched(Rendering),
        scenarios: &["compact-split"],
    },
    Case {
        id: "P12-M21",
        tier: A,
        session: M1,
        action: "实测 50/100/300% zoom，含 Source/Preview/Split",
        expected: "字体、caret、selection、公式、图片缩放正确且交互流畅",
        instance: Launched(Rendering),
        scenarios: &[
            "source-50",
            "source-100",
            "source-300",
            "preview-50",
            "preview-100",
            "preview-300",
            "split-50",
            "split-100",
            "split-300",
            "wheel-and-reset",
        ],
    },
    Case {
        id: "P12-M22",
        tier: A,
        session: M2,
        action: "拖动滑块把 opacity 调到 40/70/96/100（拖动中观察实时预览）并输入/预览/IME",
        expected: "拖动中整窗实时变化；整窗 alpha 正确（背景、文字、公式、图片、控件、阴影）；候选框、caret、焦点与控件可用",
        instance: Launched(Rendering),
        scenarios: &["opacity-40", "opacity-70", "opacity-96", "opacity-100"],
    },
    Case {
        id: "P12-M23",
        tier: A,
        session: M2,
        action: "实测 Light、Dark、System 及运行时系统主题切换，含滚动条",
        expected: "背景、文字、公式、图片、控件立即一致更新；滚动条可抓取且不遮字",
        instance: Launched(Rendering),
        scenarios: &[
            "light",
            "dark",
            "system",
            "system-to-dark",
            "system-to-light",
            "scrollbars",
        ],
    },
    Case {
        id: "P12-M24",
        tier: A,
        session: M1,
        action: "用代表性 Markdown 观察 Preview",
        expected: "标题、列表、表格、引用、代码、链接、selection 视觉正确",
        instance: Launched(Rendering),
        scenarios: &["representative-markdown"],
    },
    Case {
        id: "P12-M25",
        tier: A,
        session: M1,
        action: "用正确和错误公式观察 Preview",
        expected: "RaTeX 视觉正确；错误公式保留原文并显示错误态",
        instance: Launched(Rendering),
        scenarios: &["formulas"],
    },
    Case {
        id: "P12-M26",
        tier: A,
        session: M3,
        action: "用 PNG/JPEG/WebP/GIF 和超限图片观察 Preview",
        expected: "方向、缩放、lazy display、placeholder 与滚动显示正确",
        instance: Launched(Rendering),
        scenarios: &["image-formats"],
    },
    Case {
        id: "P12-M45",
        tier: A,
        session: M2,
        action: "同时运行两份便携副本，分别从各自的托盘图标显示/隐藏",
        expected: "两个托盘图标可辨识；显示/隐藏只作用于各自的窗口",
        instance: TwoCopies(Rendering),
        scenarios: &["tray-identity", "independent-show-hide"],
    },
    Case {
        id: "P12-M46",
        tier: A,
        session: M2,
        action: "观察 Windows 11 外壳：圆角、阴影、置顶按钮、控件淡化、边与角调整大小、各模式控件",
        expected: "外壳视觉与操作符合 plan 09 固定视觉效果与窗口契约",
        instance: Launched(Rendering),
        scenarios: &[
            "corners-shadow",
            "topmost-button",
            "control-fade",
            "edge-corner-resize",
            "controls-in-modes",
        ],
    },
    Case {
        id: "P12-M34",
        tier: B,
        session: M5,
        action: "在 Clean Windows 11 VM 解压并运行 unsigned ZIP",
        expected: "无额外 runtime 安装即可启动；信誉提示与 README 描述一致",
        instance: FirstRunDeclared,
        scenarios: &["clean-vm-first-run"],
    },
    Case {
        id: "P12-M35",
        tier: B,
        session: M4,
        action: "双显示器同 DPI 拖动、dock、tray restore、主显示器切换，副屏在右侧（顶部对齐）与下方（左侧对齐）",
        expected: "monitor identity、位置和 dock edge 稳定",
        instance: Launched(Rendering),
        scenarios: &[
            "drag",
            "dock",
            "tray-restore",
            "primary-switch",
            "secondary-right",
            "secondary-below",
        ],
    },
    Case {
        id: "P12-M36",
        tier: B,
        session: M4,
        action: "双显示器 mixed DPI 拖动、dock、IME、Preview（跨屏移动后检查公式与图片）",
        expected: "DIP/physical conversion、caret candidate 与感应条正确；跨屏后公式与图片按新显示器 scale 重新绘制、清晰",
        instance: Launched(Rendering),
        scenarios: &["drag", "dock", "ime", "preview"],
    },
    Case {
        id: "P12-M37",
        tier: B,
        session: M4,
        action: "运行中断开 StickyMD 所在的非主显示器，再重新连接该显示器",
        expected: "断开后窗口恢复到主显示器且完全可见；重新连接后窗口仍完全可见、不留在不可见坐标",
        instance: Preserved(Rendering),
        scenarios: &["disconnect-current-monitor", "reconnect-monitor"],
    },
    Case {
        id: "P12-M38",
        tier: B,
        session: M4,
        action: "在 125% DPI 完成输入、Preview、左/右/上 dock（拖到边缘、失焦收起，另一窗口在前台时悬停 3 DIP 感应条展开）、滚动条",
        expected: "geometry、caret、selection、公式与图片正确；dock 与 3 DIP 感应条正确，悬停展开不抢前台焦点；滚动条可抓取且不遮字",
        instance: Launched(Rendering),
        scenarios: &[
            "input",
            "preview",
            "left-dock",
            "right-dock",
            "top-dock",
            "scrollbars",
        ],
    },
    Case {
        id: "P12-M39",
        tier: B,
        session: M4,
        action: "在 150% DPI 完成输入、Preview、左/右/上 dock（拖到边缘、失焦收起，另一窗口在前台时悬停 3 DIP 感应条展开）、滚动条",
        expected: "geometry、caret、selection、公式与图片正确；dock 与 3 DIP 感应条正确，悬停展开不抢前台焦点；滚动条可抓取且不遮字",
        instance: Launched(Rendering),
        scenarios: &[
            "input",
            "preview",
            "left-dock",
            "right-dock",
            "top-dock",
            "scrollbars",
        ],
    },
    Case {
        id: "P12-M40",
        tier: B,
        session: M4,
        action: "在 200% DPI 完成输入、Preview、左/右/上 dock（拖到边缘、失焦收起，另一窗口在前台时悬停 3 DIP 感应条展开）、滚动条",
        expected: "geometry、caret、selection、公式与图片正确；dock 与 3 DIP 感应条正确，悬停展开不抢前台焦点；滚动条可抓取且不遮字",
        instance: Launched(Rendering),
        scenarios: &[
            "input",
            "preview",
            "left-dock",
            "right-dock",
            "top-dock",
            "scrollbars",
        ],
    },
    Case {
        id: "P12-M47",
        tier: B,
        session: M4,
        action: "在 100% DPI 完成输入、Preview、左/右/上 dock（拖到边缘、失焦收起，另一窗口在前台时悬停 3 DIP 感应条展开）、滚动条",
        expected: "geometry、caret、selection、公式与图片正确；dock 与 3 DIP 感应条正确，悬停展开不抢前台焦点；滚动条可抓取且不遮字",
        instance: Launched(Rendering),
        scenarios: &[
            "input",
            "preview",
            "left-dock",
            "right-dock",
            "top-dock",
            "scrollbars",
        ],
    },
    Case {
        id: "P12-M41",
        tier: C,
        session: M5,
        action: "Sleep/resume 后继续输入、保存、dock",
        expected: "display re-enumeration、watcher、IME 与窗口状态恢复",
        instance: Preserved(Rendering),
        scenarios: &["sleep-resume"],
    },
    Case {
        id: "P12-M42",
        tier: C,
        session: M5,
        action: "RDP reconnect 后继续输入、保存、dock",
        expected: "display/config change 不丢窗口或文本；无环境则 NOT TESTED",
        instance: Preserved(Rendering),
        scenarios: &["rdp-reconnect"],
    },
    Case {
        id: "P12-M43",
        tier: C,
        session: M4,
        action: "物理 negative-coordinate monitor layout",
        expected: "restore/dock/ratio 保存不假设非负坐标",
        instance: Launched(Rendering),
        scenarios: &["secondary-left", "secondary-above"],
    },
];

pub(in crate::qualification) fn find(id: &str) -> Option<&'static Case> {
    CASES.iter().find(|case| case.id == id)
}

/// Canonical text of the table; its digest binds an observation context to this registry.
pub(in crate::qualification) fn canonical() -> String {
    let mut text = String::from("stickymd-manual-registry-v3\n");
    for case in CASES {
        for field in [
            case.id,
            case.tier.as_str(),
            case.session.as_str(),
            case.action,
            case.expected,
            &case.instance.canonical(),
        ] {
            text.push_str(field);
            text.push('|');
        }
        text.push_str("scenarios");
        for scenario in case.scenarios {
            text.push('|');
            text.push_str(scenario);
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{CASES, Fixture, InstanceRule, Tier};

    /// SHA-256 of `docs/acceptance-cases/phase-12.md` (lines joined by `\n`) as last reviewed.
    const PHASE12_SHA256: &str = "e29bb75f523c3035e044440c37b8e73b2525ee8dcc1d3654ee9cd6f15ee68783";

    #[test]
    fn registry_equals_the_phase12_manual_rows_with_unique_scenarios() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let text = std::fs::read_to_string(root.join("docs/acceptance-cases/phase-12.md")).unwrap();
        // The whole document is bound, so no change to its lines (a reformatted table, a
        // decorated row) can add a manual case the comparison below misses; LF and CRLF, and a
        // final newline, are equivalent. After reviewing a change, update the constant.
        let normalized = text.lines().collect::<Vec<_>>().join("\n");
        assert_eq!(
            crate::integrity::sha256_bytes(normalized.as_bytes()).unwrap(),
            PHASE12_SHA256,
            "phase-12.md changed: confirm every manual row is compared below, then update PHASE12_SHA256"
        );
        // Read exactly as governance reads matrix rows, so a row is compared here or refused.
        let manual =
            crate::governance::manual_rows(&text, std::path::Path::new("phase-12.md"), "P12-M")
                .unwrap();
        let registered = CASES
            .iter()
            .map(|case| {
                (
                    case.id.to_owned(),
                    case.action.to_owned(),
                    case.expected.to_owned(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(registered, manual);
        for case in CASES {
            for text in [case.action, case.expected] {
                assert!(!text.contains(['|', '\n']), "{}", case.id);
            }
            assert!(!case.scenarios.is_empty(), "{}", case.id);
            let mut ids = case.scenarios.to_vec();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), case.scenarios.len(), "{}", case.id);
            for scenario in case.scenarios {
                assert!(
                    scenario.bytes().all(|byte| byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'-'),
                    "{}/{scenario}",
                    case.id
                );
            }
        }
        let count = |tier| CASES.iter().filter(|case| case.tier == tier).count();
        assert_eq!((count(Tier::A), count(Tier::B), count(Tier::C)), (16, 8, 3));
    }

    /// Plan 11: only M34 is observed without a recorder-launched instance, and the
    /// display-disconnect, sleep and RDP cases keep the original process.
    #[test]
    fn instance_rules_follow_plan_11() {
        let with = |wanted: fn(InstanceRule) -> bool| {
            CASES
                .iter()
                .filter(|case| wanted(case.instance))
                .map(|case| case.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            with(|rule| rule == InstanceRule::FirstRunDeclared),
            ["P12-M34"]
        );
        assert_eq!(
            with(|rule| matches!(rule, InstanceRule::Preserved(_))),
            ["P12-M37", "P12-M41", "P12-M42"]
        );
        assert_eq!(
            with(|rule| rule == InstanceRule::Launched(Fixture::Blank)),
            ["P12-M01", "P12-M02"]
        );
        assert_eq!(
            with(|rule| matches!(rule, InstanceRule::TwoCopies(_))),
            ["P12-M45"]
        );
    }
}
