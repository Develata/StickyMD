# Phase 12 Acceptance Matrix

本矩阵是最终发布资格投影，不修改产品需求。自动项由 Rust CLI/checked-in helper 持有；人工项
只有 exact candidate 上的 interactive receipt 才能改变 readiness，Markdown 仍保持
`NOT TESTED`，不得用旧记录或 Win32 style readback 代替真实观察。

## Automated qualification

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-A01 | Phase 12 governance files and stable entry exist | Automated | `cargo run -p stickymd-smoke --locked -- phase 12 --ci` | AUTOMATED PASS |
| P12-A02 | Phase 12 joins the deduplicated headless CI graph | Automated | `cargo run -p stickymd-smoke --locked -- all --ci --json` | AUTOMATED PASS |
| P12-A03 | Workspace formatting and strict Clippy remain clean | Automated | Phase 12 `-Release` task graph | AUTOMATED PASS |
| P12-A04 | Workspace and crate Release tests remain clean | Automated | Phase 12 baseline commands | AUTOMATED PASS |
| P12-A05 | Dependency/advisory/license policy remains enforced | Automated | `cargo deny check` via Phase 12 `-Release` | AUTOMATED PASS |
| P12-A06 | Exact clean commit creates deterministic local-RC package name | Automated | `tools/release/package.ps1` dirty-tree and SHA contract | AUTOMATED PASS |
| P12-A07 | ZIP allowlist, PE x64, manifest, license, no-user-data and DPI-correct runtime gates run | Automated | package runtime smoke; Per-Monitor V2 coordinate test; bounded cursor parking with actual-position confirmation; focus-transition sensor-topmost regression; copied Release Phase 8 lifecycle at 150% DPI | AUTOMATED PASS |
| P12-A08 | SPDX SBOM and checksums bind exact ZIP and SBOM | Automated | pinned Syft generator plus verifier | AUTOMATED PASS |
| P12-A09 | Candidate receipt binds source, Cargo.lock, EXE, ZIP, SBOM and toolchain | Automated | `stickymd-smoke qualification candidate` plus CLI tests | AUTOMATED PASS |
| P12-A10 | Manual recorder rejects noninteractive inference and accepts explicit statuses only | Automated | `stickymd-smoke` qualification unit tests | AUTOMATED PASS |
| P12-A11 | Stale source/EXE/artifact receipts cannot count | Automated | readiness identity validation and tests | AUTOMATED PASS |
| P12-A12 | Readiness has no force-ready path and reports missing manual/remote/USER gates | Automated | `stickymd-smoke qualification readiness --explain` | AUTOMATED PASS |
| P12-A13 | Remote workflow receipt must match exact SHA and successful attempt | Automated | `qualification remote` parser/identity contract | AUTOMATED PASS |
| P12-A14 | Downloaded ZIP must pass checksums, package/runtime smoke and exact hashes | Automated | `qualification downloaded --zip=<path>` | AUTOMATED PASS |

## Manual Tier A — release blocking unless explicitly waived

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M01 | 用 Microsoft Pinyin 连续中文、中英混输、selection composition、cancel、一次 Undo | Manual | 候选框跟 caret；commit 一次撤销；composition 不污染 canonical/undo | NOT TESTED |
| P12-M02 | 用 WeChat Input Method 重复真实 IME 矩阵 | Manual | 与 Microsoft Pinyin 同等正确；环境缺失必须记录 NOT TESTED | NOT TESTED |
| P12-M03 | 启动并观察 Windows taskbar | Manual | StickyMD 不出现在任务栏 | NOT TESTED |
| P12-M04 | 打开 Alt+Tab switcher | Manual | StickyMD 不出现在 Alt+Tab 列表 | NOT TESTED |
| P12-M05 | 聚焦 StickyMD 后 Alt+Tab 离开，再点击/托盘/传感区恢复并输入 | Manual | 切换 away 正常；恢复焦点和 IME 正常 | NOT TESTED |
| P12-M06 | 打开通知区并检查 StickyMD tray 项 | Manual | 图标与只含显示/隐藏、置顶、退出的菜单可用 | NOT TESTED |
| P12-M07 | 点击纸张关闭按钮 | Manual | 窗口隐藏到 tray，进程与文本保留 | NOT TESTED |
| P12-M08 | 从 tray 执行显示 | Manual | 同一窗口恢复并可立即输入 | NOT TESTED |
| P12-M09 | 从 tray 执行退出 | Manual | 完成保存、worker join 后进程退出 | NOT TESTED |
| P12-M10 | 把窗口拖到顶部边缘 | Manual | 仅 Top dock，失焦/hover/传感区时序符合合同 | NOT TESTED |
| P12-M11 | 把窗口拖到左边缘 | Manual | Left dock 行为正确，混合 DPI 下感应条仍可用 | NOT TESTED |
| P12-M12 | 把窗口拖到右边缘 | Manual | Right dock 行为正确，混合 DPI 下感应条仍可用 | NOT TESTED |
| P12-M13 | 把窗口拖向底边 | Manual | 不进入 Bottom dock | NOT TESTED |
| P12-M14 | 在边缘内外实测 24 DIP capture threshold | Manual | 阈值内吸附，阈值外不吸附 | NOT TESTED |
| P12-M15 | 在角落/等距位置实测 nearest-edge 与 tie | Manual | 选择最近允许边；tie 结果与 reducer 规则一致且稳定 | NOT TESTED |
| P12-M16 | 通过 3 DIP sensor hover 展开、离开 | Manual | 约 100 ms reveal；未聚焦离开约 500 ms collapse | NOT TESTED |
| P12-M17 | Docked 时分别 Pin ON/OFF 后失焦，并重复 sensor 流程 | Manual | Pin 不改变约 700 ms collapse / 100 ms reveal / 500 ms leave 语义 | NOT TESTED |
| P12-M18 | 220×120 Source 模式输入与滚动 | Manual | 内容可用、caret/selection/IME 不被工具栏遮挡 | NOT TESTED |
| P12-M19 | 220×120 Preview 模式滚动/选择/链接 | Manual | viewport、selection 与控制区可用 | NOT TESTED |
| P12-M20 | 220×120 Split 模式输入和两栏滚动 | Manual | 两栏均可操作，无不可恢复几何 | NOT TESTED |
| P12-M21 | 实测 50/100/300% zoom，含 Source/Preview/Split | Manual | 字体、caret、selection、公式、图片缩放正确且交互流畅 | NOT TESTED |
| P12-M22 | 调整 opacity 到 40 并输入/预览/IME | Manual | 整窗 alpha 正确；候选框、caret、焦点与控件可用 | NOT TESTED |
| P12-M23 | 实测 Light、Dark、System 及运行时系统主题切换 | Manual | 背景、文字、公式、图片、控件立即一致更新 | NOT TESTED |
| P12-M24 | 用代表性 Markdown 观察 Preview | Manual | 标题、列表、表格、引用、代码、链接、selection 视觉正确 | NOT TESTED |
| P12-M25 | 用正确和错误公式观察 Preview | Manual | RaTeX 视觉正确；错误公式保留原文并显示错误态 | NOT TESTED |
| P12-M26 | 用 PNG/JPEG/WebP/GIF 和超限图片观察 Preview | Manual | 方向、缩放、lazy display、placeholder 与滚动显示正确 | NOT TESTED |
| P12-M27 | 用 Shift+Insert、Ctrl+Insert、Shift+Delete 操作文本 | Manual | 与传统剪贴板语义一致，Undo/Redo 正确 | NOT TESTED |
| P12-M28 | 从 Explorer、Snipping Tool 与 browser 粘贴真实图片 | Manual | 格式优先级、文件写入、Markdown 插入与 Undo 原子性正确 | NOT TESTED |
| P12-M29 | 执行 Ctrl+Shift+S native Export dialog | Manual | 原生对话框、路径重写、引用图片复制与 active note 不切换 | NOT TESTED |
| P12-M30 | 在保存窗口附近强杀复制目录中的 Release EXE 并重启 | Manual | note 完整或 tmp 可恢复；无 half UTF-8；证据未被静默删除 | NOT TESTED |
| P12-M31 | 对真实 `\\(x\\)` 与多行 `\\[y\\]` 执行转换并一次 Undo | Manual | 转为 `$x$` / `$$...$$`；inline code/literal 不变；一次 Undo 全部恢复 | NOT TESTED |
| P12-M32 | 完成真实 user asset edit/undo/redo/GC/export/quit/restart 流程 | Manual | 非 managed 用户文件从未被自动移动或删除 | NOT TESTED |
| P12-M33 | 放置 managed-looking fake file 并完成 GC/restart | Manual | 无 ownership evidence 的伪 managed 文件不被删除 | NOT TESTED |
| P12-M34 | 在 Clean Windows 11 VM 解压并运行 unsigned ZIP | Manual | 无额外 runtime 安装即可启动；信誉提示与 README 描述一致 | NOT TESTED |

## Manual Tier B

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M35 | 双显示器同 DPI 拖动、dock、tray restore | Manual | monitor identity、位置和 dock edge 稳定 | NOT TESTED |
| P12-M36 | 双显示器 mixed DPI 拖动、dock、IME、Preview | Manual | DIP/physical conversion、caret candidate 与感应条正确 | NOT TESTED |
| P12-M37 | 运行中断开当前显示器再恢复窗口 | Manual | 窗口迁移到可见工作区且不留在不可见坐标 | NOT TESTED |
| P12-M38 | 在 125% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |
| P12-M39 | 在 150% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |
| P12-M40 | 在 200% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |

## Manual Tier C

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M41 | Sleep/resume 后继续输入、保存、dock | Manual | display re-enumeration、watcher、IME 与窗口状态恢复 | NOT TESTED |
| P12-M42 | RDP reconnect 后继续输入、保存、dock | Manual | display/config change 不丢窗口或文本；无环境则 NOT TESTED | NOT TESTED |
| P12-M43 | 物理 negative-coordinate monitor layout | Manual | restore/dock/ratio 保存不假设非负坐标 | NOT TESTED |
| P12-M44 | 从真实 junction/symlink 路径启动两个副本 | Manual | canonical identity 合并为同目录单实例，且第二实例不写盘 | NOT TESTED |

## Current readiness

所有 `P12-M*` 当前均为 `NOT TESTED`。这不会使 headless CI 失败，但会使 release readiness
保持 `NOT_READY`，除非 exact-artifact manual receipt 全部 PASS 或 USER 明确批准具体 waiver。
