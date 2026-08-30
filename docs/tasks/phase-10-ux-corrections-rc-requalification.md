# Phase 10 — UX Corrections, Automation Consolidation and RC Requalification

## Goal

实施 USER 已批准的十项 v1 交互修订，把自动化 gate/evidence 收敛到 Rust CLI，并在最终
代码上重新执行启动、性能、资源、回归与本地候选资格化。

## Scope

- 传统剪贴板快捷键别名。
- 50–300% 全局 Content Zoom。
- 220×120 DIP 最小窗口与紧凑布局。
- 可聚焦 Tool Window 身份。
- 24 DIP 最近合格边缘 dock 判定。
- 40–100% 整窗透明度。
- Rust smoke CLI JSON evidence、薄 PowerShell、CI headless 路由。
- 冷/暖启动方法审计、Phase 9 gate 重跑、Phase 10 本地候选重新生成。

## Inputs

- `docs/phases/2026-08-22-phase-10-user-approved-ux-corrections.md`
- `docs/plan/00..11`
- Phase 8/9 reports、acceptance matrices 与现有产品代码。

## Deliverables

- 权威 plan、feature、AC-031..036、coverage 同步。
- 十项行为实现与回归测试。
- `tools/smoke/phase-10.ps1`、`docs/acceptance-cases/phase-10.md`。
- Phase 10 automation/UX/startup/RC reports。
- 新的 exact-commit local candidate、checksums 与 SBOM。

## Verification

Rust CLI headless/performance/runtime/resource/startup/package routes；workspace fmt/clippy/tests/
Release/deny；30 cold + 30 warm；AC-001..036 自动化回归。真实 IME、视觉、托盘、物理显示器
与故障时序继续依正式人工矩阵，不以自动化替代。

## Out of Scope

新产品功能、WebView/async runtime/network/database、release tag/push/publish、旧 manual 项自动升级。

## Completion State

Completed

Automated UX/resource/package work is complete. Phase 10 当时的 warm startup blocker 与
current-candidate real-environment `NOT TESTED` 状态仍如实保留在
`docs/report/phase-10-rc-requalification.md`；后续 Phase 处理了发布门与 USER disposition。
