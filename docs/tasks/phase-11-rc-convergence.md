# Phase 11 — RC Convergence

## Completion State

In Progress

## Purpose

在不增加产品功能的前提下，校准启动门、收口最终自动化证据、保持人工门真实状态，并生成新的
本地 RC 候选。Phase 11 不以牺牲边界、authority 或维护成本换取性能数字。

## Prerequisites

- Phase 10 实现已完成，但 warm startup 与真实环境人工验收仍未关闭。
- Starting commit: `3b4ec804209dcfbce8108643ca539ab6b0a257be`。
- USER 未批准 warm startup 新门槛、人工 waiver、push、tag 或 GitHub Release。

## Inherited Conditions

- Warm startup p95 hard gate 仍为 180 ms；未获 USER waiver。
- Microsoft Pinyin、WeChat 输入法、Tool Window、tray、三边 docking、物理多屏、真实恢复与
  Clean VM 等人工项目沿用 `NOT TESTED`，直到有当前候选的可复核 receipt。
- Phase 10 本地 artifact 在 Phase 11 新候选生成前仍是最新 artifact，之后必须标记 superseded。

## Scope

- 修正性能治理语言，区分不可放宽 invariant 与可由 USER 校准的量化门。
- 审核 `EDITOR_READY`、进程退出、唯一 ready event、cold/warm cohort 与启动里程碑。
- 至少 50 个 warm、30 个 cold 样本；输出原始样本和完整统计。
- 最多保留 2–3 个简单、局部、高收益、低耦合优化；每个均需 before/after。
- 运行最终 headless regression、resource/stress/security 检查并生成本地 RC artifact。
- 维护 Rust CLI smoke、Phase 11 PowerShell 薄入口和完整 acceptance matrix。

## Out of Scope

- 新产品功能、第二渲染器、第二字体 authority、持久字体数据库、后台服务。
- 仅为 benchmark 分支的产品路径、通用 async runtime、线程池、数据库、网络 client。
- 自动批准 gate、人工 waiver、push、tag 或 GitHub Release。

## Architecture Invariants

- `DocumentState` 仍是 canonical text 唯一 authority。
- `SourceProjection` / Preview / disk / worker snapshot 均不是平级 authority。
- 原子保存、OCC、managed-asset ownership、generation stale-drop 与 IME composition atomicity
  不可因性能门放宽。
- 诊断仅在显式环境变量开启时收集；普通启动不持久化 benchmark 状态。

## Startup Qualification

- Fixture: copied standalone Release EXE + 20 KiB Source note。
- Cold: 至少 30 个样本，记录冷却条件。
- Warm: 至少 50 个样本，同目录连续启动。
- 每个样本确认上一进程退出，并使用 PID/nonce 唯一 ready object。
- `EDITOR_READY` 仍表示首个可用 frame 已成功 present；不得前移。

## Optimization Budget

1. 审核并消除可证明的首次 viewport 重复布局（待 before/after 决定是否保留）。
2. 仅在证据显示另一局部 dominant cost 时考虑第二项。
3. 低收益或复杂度不成比例的改动必须撤销。

## Manual Acceptance

人工状态投影到 `docs/acceptance-cases/phase-11.md` 与
`docs/report/phase-11-manual-acceptance.md`。当前环境无法形成真实 IME、物理多屏或 Clean VM
receipt 的项目继续保持 `NOT TESTED`。

## Deliverables

- `tools/smoke/phase-11.ps1`
- `docs/acceptance-cases/phase-11.md`
- `docs/report/phase-11-blocker-classification.md`
- `docs/report/phase-11-warm-startup-analysis.md`
- `docs/report/phase-11-warm-startup-gate-reassessment.md`（若 180 ms 未通过）
- `docs/report/phase-11-manual-acceptance.md`
- `docs/report/phase-11-performance-final.md`
- `docs/report/phase-11-rc-readiness.md`

## Verification

- Rust CLI governance/headless/runtime/resource/performance/release/package modes。
- `cargo fmt --check`、workspace clippy/tests/Release、core/render release tests、`cargo deny check`。
- dependency/unsafe/forbidden-architecture/file-authority scans、`git diff --check`。
- 本地 artifact hash、SBOM 与 package verification；不 push/tag/release。

## Result

Pending measurement and final evidence.
