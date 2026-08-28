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
| P12-A12 | Readiness has no force-ready path and reports missing/failed Release, headless CI, performance, runtime, resources, manual, remote and USER gates | Automated | exact-mode receipt contracts plus `stickymd-smoke qualification readiness --explain` | AUTOMATED PASS |
| P12-A13 | Remote workflow receipt must match exact SHA and successful attempt | Automated | `qualification remote` parser/identity contract | AUTOMATED PASS |
| P12-A14 | Downloaded ZIP must pass checksums, package/runtime smoke and exact hashes | Automated | `qualification downloaded --zip=<path>` | AUTOMATED PASS |
| P12-A15 | Release PE 普通/延迟导入表不含需另装的 C/C++/Rust developer runtime；构建静态链接 MSVC CRT | Automated | `stickymd-smoke qualification native-runtime --exe=target/release/stickymd-win.exe` + Release/package CI | AUTOMATED PASS |
| P12-A16 | G3 exact harness 的命令、隔离、receipt identity 与五项 fail-closed 聚合可由 headless CI 验证 | Automated | `stickymd-smoke` G3 CLI/receipt unit tests；GUI 只在本地交互桌面运行 | AUTOMATED PASS |
| P12-A17 | G4 exact harness 的命令、隔离、receipt identity 与五组 fail-closed 聚合可由 headless CI 验证 | Automated | `stickymd-smoke` G4 CLI/receipt/unit contract tests；GUI 只在本地交互桌面运行 | AUTOMATED PASS |

## Manual Tier A — release blocking unless explicitly waived

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M01 | 用 Microsoft Pinyin 连续中文、中英混输、selection composition、cancel、一次 Undo | Manual | 候选框跟 caret；commit 一次撤销；composition 不污染 canonical/undo | NOT TESTED |
| P12-M02 | 用 WeChat Input Method 重复真实 IME 矩阵 | Manual | 与 Microsoft Pinyin 同等正确；环境缺失必须记录 NOT TESTED | NOT TESTED |
| P12-M03 | 启动并验证 Windows taskbar eligibility | Automated exact candidate | `phase-14.ps1 -G5` / G5-01；真实 HWND 保持 `WS_EX_TOOLWINDOW` 且无 `WS_EX_APPWINDOW` | NOT TESTED |
| P12-M04 | 验证 Alt+Tab eligibility | Automated exact candidate | `phase-14.ps1 -G5` / G5-01；真实 HWND 保持 ToolWindow shell identity，不进入普通 app switch surface | NOT TESTED |
| P12-M05 | 聚焦 StickyMD 后 Alt+Tab 离开，再点击/托盘/传感区恢复并输入 | Manual | 切换 away 正常；恢复焦点和 IME 正常 | NOT TESTED |
| P12-M06 | 检查 exact EXE 的 StickyMD tray 项 | Automated exact candidate | `phase-14.ps1 -G4` / G4-01；菜单恰为显示/隐藏、置顶、退出 | NOT TESTED |
| P12-M07 | 点击 exact EXE 纸张关闭按钮 | Automated exact candidate | `phase-14.ps1 -G4` / G4-01；窗口隐藏，进程与文本保留 | NOT TESTED |
| P12-M08 | 从 tray 对 exact EXE 执行显示 | Automated exact candidate | `phase-14.ps1 -G4` / G4-01；同一 HWND 恢复并可立即输入 | NOT TESTED |
| P12-M09 | 从 tray 对 dirty exact EXE 执行退出 | Automated exact candidate | `phase-14.ps1 -G4` / G4-01；保存、worker join 后进程退出 | NOT TESTED |
| P12-M10 | 把 exact EXE 拖到顶部边缘 | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；仅 Top dock，失焦/hover/传感区时序符合合同 | NOT TESTED |
| P12-M11 | 把窗口拖到左边缘 | Manual | Left dock 行为正确，混合 DPI 下感应条仍可用 | NOT TESTED |
| P12-M12 | 把窗口拖到右边缘 | Manual | Right dock 行为正确，混合 DPI 下感应条仍可用 | NOT TESTED |
| P12-M13 | 把 exact EXE 拖向底边 | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；不进入 Bottom dock | NOT TESTED |
| P12-M14 | 对 exact EXE 实测 24/25 DIP capture threshold | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；24 DIP 吸附，25 DIP 不吸附 | NOT TESTED |
| P12-M15 | 对 exact EXE 实测 nearest-edge 与 top-left/top-right tie | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；选择最近允许边，精确优先级 `Top > Left > Right` | NOT TESTED |
| P12-M16 | 通过 exact EXE 的 3 DIP sensor hover 展开、离开 | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；100 ms reveal、500 ms leave collapse 边界 | NOT TESTED |
| P12-M17 | Exact EXE Docked 时分别 Pin ON/OFF 后失焦并重复 sensor 流程 | Automated exact candidate | `phase-14.ps1 -G4` / G4-02；Pin 与 700/100/500 ms auto-hide 语义正交 | NOT TESTED |
| P12-M18 | 220×120 Source 模式输入与滚动 | Manual | 内容可用、caret/selection/IME 不被工具栏遮挡 | NOT TESTED |
| P12-M19 | 220×120 Preview 模式滚动/选择/链接 | Manual | viewport、selection 与控制区可用 | NOT TESTED |
| P12-M20 | 220×120 Split 模式输入和两栏滚动 | Manual | 两栏均可操作，无不可恢复几何 | NOT TESTED |
| P12-M21 | 实测 50/100/300% zoom，含 Source/Preview/Split | Manual | 字体、caret、selection、公式、图片缩放正确且交互流畅 | NOT TESTED |
| P12-M22 | 调整 opacity 到 40 并输入/预览/IME | Manual | 整窗 alpha 正确；候选框、caret、焦点与控件可用 | NOT TESTED |
| P12-M23 | 实测 Light、Dark、System 及运行时系统主题切换 | Manual | 背景、文字、公式、图片、控件立即一致更新 | NOT TESTED |
| P12-M24 | 用代表性 Markdown 观察 Preview | Manual | 标题、列表、表格、引用、代码、链接、selection 视觉正确 | NOT TESTED |
| P12-M25 | 用正确和错误公式观察 Preview | Manual | RaTeX 视觉正确；错误公式保留原文并显示错误态 | NOT TESTED |
| P12-M26 | 用 PNG/JPEG/WebP/GIF 和超限图片观察 Preview | Manual | 方向、缩放、lazy display、placeholder 与滚动显示正确 | NOT TESTED |
| P12-M27 | 对 exact EXE 用 Shift+Insert、Ctrl+Insert、Shift+Delete 操作文本/图片 | Automated exact candidate | `phase-14.ps1 -G4` / G4-03；传统剪贴板语义、Preview 只读及 Undo/Redo 正确 | NOT TESTED |
| P12-M28 | 以 Explorer/clipboard file drop、Snipping DIB 与 browser PNG+text 标准格式粘贴 exact EXE | Automated exact candidate | `phase-14.ps1 -G3` / G3-01；格式优先级、文件写入、Markdown 插入与 Undo/Redo 资产原子性 | NOT TESTED |
| P12-M29 | 对 exact EXE 执行 Ctrl+Shift+S native Export dialog | Automated exact candidate | `phase-14.ps1 -G3` / G3-02；UIA 只选路径，Rust 验证路径重写、图片复制与 active note 不切换 | NOT TESTED |
| P12-M30 | 在多个保存时点强杀隔离 exact EXE 并重启 | Automated exact candidate | `phase-14.ps1 -G3` / G3-03；note 完整或 tmp 可恢复，无 half UTF-8 | NOT TESTED |
| P12-M31 | 对 exact EXE 中真实 `\\(x\\)` 与多行 `\\[y\\]` 执行转换并一次 Undo | Automated exact candidate | `phase-14.ps1 -G4` / G4-04；源码投影立即刷新；inline code/literal 不变；一次 Undo 全部恢复 | NOT TESTED |
| P12-M32 | 完成 exact EXE user asset edit/undo/redo/GC/export/tray quit/restart 流程 | Automated exact candidate | `phase-14.ps1 -G3` / G3-04；非 managed 用户文件逐边界 hash 不变 | NOT TESTED |
| P12-M33 | 放置 managed-looking fake file 并完成 exact EXE quit/restart | Automated exact candidate | `phase-14.ps1 -G3` / G3-05；无 ownership evidence 的伪 managed 文件路径/hash 不变 | NOT TESTED |

## Manual Tier B — environment-dependent; exact version/source-bound USER group waiver allowed

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M34 | 在 Clean Windows 11 VM 解压并运行 unsigned ZIP | Manual | 无额外 runtime 安装即可启动；信誉提示与 README 描述一致 | NOT TESTED |
| P12-M35 | 双显示器同 DPI 拖动、dock、tray restore | Manual | monitor identity、位置和 dock edge 稳定 | NOT TESTED |
| P12-M36 | 双显示器 mixed DPI 拖动、dock、IME、Preview | Manual | DIP/physical conversion、caret candidate 与感应条正确 | NOT TESTED |
| P12-M37 | 运行中断开当前显示器再恢复窗口 | Manual | 窗口迁移到可见工作区且不留在不可见坐标 | NOT TESTED |
| P12-M38 | 在 125% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |
| P12-M39 | 在 150% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |
| P12-M40 | 在 200% DPI 完成输入、Preview、dock | Manual | geometry、caret、selection、公式与图片正确 | NOT TESTED |

## Manual Tier C — NOT TESTED is non-blocking only while automated contract coverage passes

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P12-M41 | Sleep/resume 后继续输入、保存、dock | Manual | display re-enumeration、watcher、IME 与窗口状态恢复 | NOT TESTED |
| P12-M42 | RDP reconnect 后继续输入、保存、dock | Manual | display/config change 不丢窗口或文本；无环境则 NOT TESTED | NOT TESTED |
| P12-M43 | 物理 negative-coordinate monitor layout | Manual | restore/dock/ratio 保存不假设非负坐标 | NOT TESTED |
| P12-M44 | 从真实 junction 路径启动 exact EXE 第二实例 | Automated exact candidate | `phase-14.ps1 -G4` / G4-05；同一 canonical identity 唤醒原 HWND，第二实例不写 durable file | NOT TESTED |

## Current readiness

所有 source matrix 行仍为 `NOT TESTED`，这不会使 headless CI 失败。G3/G4/G5 自动 exact 行只有对应
receipt 完整 PASS 才解除 readiness blocker；其余 Tier A 仍需人工 PASS 或 USER 明确批准具体
case/group waiver。Tier B 需要 PASS 或
绑定 v0.1.0 + exact source 的明确 USER disposition；Tier C 仅在对应自动化合同通过时允许保持
`NOT TESTED` 而不阻断，任何已观察到的 `MANUAL_FAIL` 仍阻断。
