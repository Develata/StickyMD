# Phase 14 — Release Gate Calibration and Qualification Closure

## Status

Qualification In Progress — USER-observed candidate defects corrected; exact-candidate requalification required.

## Purpose

在产品 runtime 默认冻结的前提下，校准 v0.1.0 startup release boundary，完成 startup attribution、
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
- 对 exact-candidate 人工验收中 USER 实际观察到的 release-blocking 缺陷作最小、回归绑定的纠正；
  每次 tracked correction 都作废旧候选并重新开始资格化。

## Out of Scope

- 未经 USER 实际缺陷报告授权的产品 runtime、产品依赖、Markdown/IME/window/persistence 行为变化。
- 为达到 180/400 ms target 做追逐式优化。
- push、tag、remote workflow、draft release 或 publish。

## Authority and Freeze

唯一产品 authority 不变；Phase 14 tracked delta 原则上仅限 plan/projection/docs 与 verification tooling；
USER 在 exact-candidate 人工验收中报告的 release-blocking implementation defect 可以按原 plan 作最小纠正。
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
- USER-observed regression 的 named tests，以及扩展后的三边/顶角 copied-Release smoke。
- 远端 CI 故障归因与可诊断输出；隔离 runner CI 分片并发、模块化 Resources 定向入口与
  “分片并集 = 完整任务图”回归。

## Candidate Defect Correction

旧 exact candidate 在人工验收中暴露五项 implementation defect：同一行局部 selection 误涂其它
逻辑行；math delimiter conversion 后 Source projection 未立即进入 layout；Split/Preview 之间切换时
clean preview 未按新 viewport relayout；真实 winit move loop 未提交 Dock，导致三边失焦自动收起不
工作；从旧 Dock 直接换到另一边时错误地先 detach，导致需要第二次拖动。修复不改变 Document
authority、Markdown/math semantics 或 runtime dependency。Phase 8
copied-Release smoke 改用真实指针拖动与真实 shell 失焦，同时补齐 left/top/right、两个顶角和 Pin
ON/OFF 正交路径，并连续执行 Left -> Top -> Left -> Right、禁止以 Floating 中间态掩盖直接换边；
人工 G2 在新候选上重跑前仍为 `NOT TESTED`。详见
`docs/report/phase-14-candidate-defect-remediation.md`。

## Resources Failure Triage

旧 candidate `1d533357ac072605b350b0523f2957597341bc62` 的 Phase 8 hidden-window
resource matrix 在外部 reload 后注入 Enter 时失败。分段、组合与降阶复现没有重现产品状态机
错误；根因归类为 `QUALIFICATION HARNESS DEFECT`：旧 harness 用固定 350 ms sleep 推断 source
projection 已完成，既没有验证 editor projection，也没有记录 foreground/focus/geometry 等 shell
事实。

修正保持产品 runtime 与 runtime dependency delta 为零：Rust smoke CLI 新增有界
`window-stress` reducer、typed shell-state wait 和基于真实 Ctrl+A/C clipboard projection 的 ready
gate。tracked tooling 变化使旧 candidate 失效；必须在新 freeze commit 上重新收集 Release/package、
headless CI、Runtime、Performance 与 Resources。Resources PASS 前不开始正式 manual receipt。

## Result

source-controlled policy、CLI、CI、guided manual 与 readiness contract 已实现并通过冻结前基线。
动态 evidence 与最终 recommendation 只在 exact source freeze 后产生；人工项目不会因此自动 PASS。
