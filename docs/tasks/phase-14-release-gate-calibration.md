# Phase 14 — Release Gate Calibration and Qualification Closure

## Status

Implementation Complete — awaiting exact-candidate evidence and USER-driven manual acceptance.

## Purpose

在产品 runtime 冻结的前提下，校准 v0.1.0 startup release boundary，完成 startup attribution、
风险分层人工验收、独立 evidence channel 与 local readiness 收口。

## Prerequisites

- Phase 13 exact candidate 自动化通道已形成可审计收据。
- USER 已明确批准 v0.1.0、unsigned distribution、550 ms cold/warm hard boundary、
  Tier A/B/C manual policy 与 independent evidence collection。
- push、tag、draft release、publish 均未授权。

## Scope

- 更新 `docs/plan/10` / `docs/plan/11` 发布合同。
- `stickymd-smoke` 增加 Phase 14、三层 startup threshold、startup attribution、G1..G3 guided
  manual sessions、risk-tier readiness 和 independent channel collection。
- 建立 exact candidate 并重新收集 Release、headless CI、Runtime、Performance、Resources、
  Manual、Readiness evidence。

## Out of Scope

- 产品 runtime、产品依赖、Markdown/IME/window/persistence 行为变化。
- 为达到 180/400 ms target 做追逐式优化。
- push、tag、remote workflow、draft release 或 publish。

## Authority and Freeze

唯一产品 authority 不变；Phase 14 tracked delta 仅限 plan/projection/docs 与 verification tooling。
tracked freeze commit 后所有动态收据只写 ignored `dist/evidence/`。任何 tracked source 改变都会
创建新 candidate，并使旧 candidate receipt 失效。

## Deliverables

- `tools/smoke/phase-14.ps1`
- `docs/acceptance-cases/phase-14.md`
- `docs/report/phase-14-release-policy.md`
- `docs/report/phase-14-startup-attribution-plan.md`
- `docs/report/phase-14-final-qualification.md`
- `docs/reference/qualification-execution-model.md`
- `tools/manual/phase-14-guide.md`
- exact Phase 14 evidence bundle and final readiness report

## Verification

- Phase 14 targeted Rust CLI tests and smoke contract trace。
- workspace fmt / strict Clippy / tests / Release build / deny。
- exact copied candidate Release, headless, Runtime, Performance, Resources and Readiness channels。
- product runtime/dependency delta audit。

## Result

source-controlled policy、CLI、CI、guided manual 与 readiness contract 已实现并通过冻结前基线。
动态 evidence 与最终 recommendation 只在 exact source freeze 后产生；人工项目不会因此自动 PASS。
