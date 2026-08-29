# Phase 14 Acceptance Matrix

本矩阵验证 release policy / qualification tooling，以及 USER 在 candidate freeze 前明确批准的
Split 同步、Source 查找替换和转换控件标识。任何 runtime 变化都会作废旧候选并触发完整重新资格化。自动项由 std-only Rust CLI
持有并由 CI 调用。人工项和 exact-desktop 动态项在 source matrix 中保持 `NOT TESTED`，只有对应的
ignored exact-candidate receipt 可改变 readiness，不能改写此 Markdown 状态。

## Automated qualification

| ID | Requirement | Mode | Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-A01 | Phase 14 stable PowerShell entry、task、report、matrix、guided manual guide 存在 | Automated | `stickymd-smoke phase 14 --ci` | AUTOMATED PASS |
| P14-A02 | Phase 14 加入 deduplicated headless CI graph | Automated | `stickymd-smoke all --ci --json` | AUTOMATED PASS |
| P14-A03 | cold/warm startup 同时报告 180/400/550 ms；正式 warm-cache 固定等待 1000 ms，250 ms rapid-restart 仅作独立诊断；仅 >550 ms hard fail | Automated | qualification/runtime interval + threshold unit tests | AUTOMATED PASS |
| P14-A04 | Performance ordinary failure 不跳过 Resources | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A05 | Resources failure 不抹除 Performance receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A06 | Runtime ordinary failure 仍保留后续独立、安全 receipt | Automated | campaign policy unit tests | AUTOMATED PASS |
| P14-A07 | invalid environment（含无法写入物理 cursor position）、identity mismatch、P0/data-safety failure 全局停止 | Automated | qualification environment capability probe + campaign policy unit tests | AUTOMATED PASS |
| P14-A08 | startup attribution 使用 per-sample milestone intervals 并输出唯一 decision | Automated | attribution parser/classifier tests | AUTOMATED PASS |
| P14-A09 | Tier A `NOT TESTED` 阻断；PASS 或 explicit waiver 才 eligible | Automated | readiness tests | AUTOMATED PASS |
| P14-A10 | Tier B group waiver 必须绑定 version/source；Tier C NT 仅在 automation PASS 时 nonblocking | Automated | readiness tests | AUTOMATED PASS |
| P14-A11 | v0.1.0 waiver 不适用于 v0.1.1 | Automated | readiness version-binding test | AUTOMATED PASS |
| P14-A12 | manual receipt 绑定 exact source/EXE/ZIP/version/Windows build/session/case | Automated | manual receipt tests | AUTOMATED PASS |
| P14-A13 | unsigned package 明确记录且不伪造 Authenticode signed fields | Automated | release/package contract tests | AUTOMATED PASS |
| P14-A14 | GitHub-hosted CI 不执行 absolute 550 ms/resource qualification | Automated | workflow governance trace | AUTOMATED PASS |
| P14-A15 | USER 授权的 candidate defect correction 与资格化加固保持边界内、无新 runtime dependency 且有命名回归 | Automated | git path/dependency audit + source/preview multiline and grapheme selection isolation、source projection/stale-preview/hard-break/theme-math-cache、missing-canonical recovery/note-ack/Keep-Local force-receipt barrier、fixed-temp/recovery/export evidence ownership、native-drag/direct-redock、zoomed toolbar paint/hit alignment、线性 semantic conversion/search、USER rendering-stress Markdown/RaTeX/layout/lazy-image/source-safety regressions | AUTOMATED PASS |
| P14-A16 | exact candidate Release、headless、Runtime、Performance、Resources receipt 独立绑定；runtime smoke 使用有界 reducer、真实 shell/source projection ready gate，并以 fail-closed physical input 驱动快捷键、toolbar view controls、拖动、失焦、Left -> Top -> Left -> Right dock/auto-hide、Right Pin-ON 正交路径、顶角优先级与真实 compact resize；sensor reveal 显式建立 tracked leave，view-mode/image 压力通道分别证明画面投影与 durable config | Automated | Phase 10 targeted Runtime、Phase 14 qualification campaign、`window-stress` collapse/view-mode parser 与 copied-Release runtime contract tests；动态 receipt 不回写本表 | AUTOMATED PASS |
| P14-A17 | CI tests/performance 分片并发时，任务并集严格等于完整 `all --ci` 且失败日志同时保留 test stdout 与 Cargo stderr | Automated | smoke CLI shard-union/output-capture tests + `.github/workflows/ci.yml` | AUTOMATED PASS |
| P14-A18 | 日常 Resources 可按 source-preview/math/images/window/zoom 定向运行；完整候选仍要求全矩阵 | Automated | `phase-14.ps1 -Resources -ResourceModule <module>` + smoke task-plan tests | AUTOMATED PASS |
| P14-A19 | Portable Release 静态链接 MSVC CRT，Rust CLI 同时检查普通与 delay-load PE imports，CI 在打包前拒绝外置 developer runtime | Automated | `.cargo/config.toml` + `qualification native-runtime` + CI/release workflow trace | AUTOMATED PASS |
| P14-A20 | Split 语义同步默认开启、可持久关闭；双向手势单向映射、stale generation guard、无反馈环且保留独立位置 | Automated | render anchor-index + app reducer/config + Rust CLI regression | AUTOMATED PASS |
| P14-A21 | Source 纯文本查找/替换支持大小写开关、wrap、单次/全部替换、generation invalidation、Unicode boundary 与单事务 Undo；关闭后释放匹配投影且不再扫描，外部重载刷新打开会话，搜索焦点不吞掉全局保存/导出；不含正则 | Automated | interaction/flow/core tests + Rust CLI workspace-test shard regression | AUTOMATED PASS |
| P14-A22 | 数学分隔符转换控件以清晰的 `$` 标识，paint/hit geometry 在 50/100/300% 一致 | Automated | toolbar paint/hit contract + headless CI | AUTOMATED PASS |
| P14-A23 | Release 内存按 Source/Preview/Split/cache 分模块归因；优化决策有实测依据且不放宽既有 hard gate | Automated | targeted resource modules + `docs/report/phase-14-memory-attribution.md` | AUTOMATED PASS |
| P14-A24 | 至少 100 个独立 copied-Release 桌面运行且全部失败仅为已分类输入/调度抖动时，成功率 `≥98%` PASS、`<98%` FAIL；内容/数据安全/崩溃/resource hard gate 永不容错 | Automated | repetition boundary tests + window-stress jitter/blocking classifier tests + `docs/plan/11_testing_and_release.md#desktop-repetition-jitter-policy` | AUTOMATED PASS |
| P14-A25 | G3 exact automation 串行使用隔离候选目录；Rust 持有 clipboard/export/kill/recovery/asset 断言，UIA 只适配原生对话框/tray；receipt 对 source/harness/clean tree/EXE/ZIP/五项结果 fail closed | Automated | `qualification g3` parser/receipt tests + `tools/stickymd-smoke/helpers/windows-uia.ps1` boundary audit；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A26 | G4 exact automation 串行使用隔离候选目录；Rust 持有 tray lifecycle、三边 dock/时序、legacy shortcuts、真实数学转换与 junction 单实例断言；tray UIA 对物理右键执行 menu-open acknowledgement、一次有界重试与几何/菜单诊断；receipt 对 source/harness/clean tree/EXE/ZIP/六组结果 fail closed | Automated | `qualification g4` parser/receipt/unit contract tests + UIA adapter governance；UIA 只适配 tray；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A27 | G5 exact automation 串行验证 ToolWindow shell identity、220×120 Source/Preview/Split mechanics、50/100/300% zoom、40 opacity、主题循环以及 Markdown/math/image stress，并把逐候选截图 path/SHA-256 绑定到 receipt；真实 IME、mixed-DPI 与首次视觉判断仍由人工持有 | Automated | `qualification g5` parser/receipt/unit contract tests；UIA 只负责窗口截图，不持有判定；GitHub-hosted CI 只运行无界面子集 | AUTOMATED PASS |
| P14-A28 | Preview 选择保留 Cosmic Text shaping cluster 几何；Times/CJK/Emoji/组合字符/换行/BiDi 的 hit-test、蓝框与 copy range 同源，几何仅缓存当前 viewport，禁止整段比例估算 | Automated | render viewport-cluster geometry unit/integration tests + Phase 14 headless tests shard + Release baseline（5,000 rows；viewport projection p95 19.1 µs；10,000 hits 340.8 µs） | AUTOMATED PASS |
| P14-A29 | 查找/替换使用单一 session；Ctrl+F toggle、Ctrl+H expand、Find-only replacement guard、方向键导航、字段 caret/mouse/IME geometry 与源码 caret 隔离均有回归 | Automated | interaction/render/app unit tests + Phase 14 headless shard | AUTOMATED PASS |
| P14-A30 | exact candidate 使用真实 Microsoft Pinyin 与 WeType profile，以物理键盘验证 Source/Search composition、commit/cancel、selection replace 与一次 Undo；测试结束恢复原 profile | Automated exact candidate | `phase-14.ps1 -G4 -G4Case G4-06`；完整 G4 receipt 必须包含 G4-06 | NOT TESTED |
| P14-A31 | smoke 启动的每个 StickyMD GUI child 都由 RAII owner 持有，普通错误返回或 unwind 会执行 kill + wait；Performance/Resources 在启动前对已遗留的 smoke-owned 测试进程 fail closed，且绝不自动终止用户自己的便签实例 | Automated | `managed_process::tests` + runtime isolation-scenario tests；正式测量前置检查 | AUTOMATED PASS |
| P14-M01 | Microsoft Pinyin / WeType 候选窗位置、遮挡、字体、动画及 DPI 视觉质量 | Guided Manual | exact candidate G1；自动化矩形/截图只能作 companion evidence | NOT TESTED |

## Guided manual sessions

下列 session 是交互记录入口，不替代 `phase-12.md` 的 P12-M01..M44 authority。

| ID | Scope | Mode | Underlying cases / Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-G1 | Editor / IME / rendering | Guided Manual | P12-M01,M02,M21,M22,M24..M26 | NOT TESTED |
| P14-G2 | focus recovery / mixed-DPI dock / compact visual / theme | Guided Manual | P12-M05,M11,M12,M18..M20,M23；可复核 G5 截图以减少重复操作 | NOT TESTED |

## Exact-candidate automated desktop session

| ID | Scope | Mode | Underlying cases / Evidence | Status |
| --- | --- | --- | --- | --- |
| P14-G3 | clipboard / native export / process-kill recovery / asset safety | Automated exact candidate | `phase-14.ps1 -G3`; P12-M28..M30,M32,M33；独立 `g3-exact-qualification.json` | NOT TESTED |
| P14-G4 | tray lifecycle / dock timing / legacy shortcuts / math conversion / junction identity / real IME functional matrix | Automated exact candidate | `phase-14.ps1 -G4`; P12-M06..M10,M13..M17,M27,M31,M44 + P14-A30；独立 `g4-exact-qualification.json` | NOT TESTED |
| P14-G5 | shell identity / compact / presentation / rendering mechanics | Automated exact candidate | `phase-14.ps1 -G5`; P12-M03,M04，并为 M05/M18..M26 提供候选绑定的机械与截图 companion evidence；独立 `g5-exact-qualification.json` | NOT TESTED |

## Readiness interpretation

- Tier A manual facts require exact PASS or explicit case/group waiver。
- Tier B requires exact PASS or version/source-bound USER disposition。
- Tier C `NOT TESTED` is nonblocking only while corresponding automated coverage is PASS；FAIL blocks。
- PUSH、TAG、DRAFT-RELEASE、PUBLISH 均不由本矩阵授权。
