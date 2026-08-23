# Phase 13 — Exact Candidate Qualification Campaign

## Status

In Progress — source preparation precedes the exact candidate freeze.

## Purpose

在不修改产品 runtime 的前提下，建立可快速拒绝锁屏/非交互环境的资格化门，冻结唯一候选
commit，并按固定顺序收集 exact-artifact 自动、人工与远端证据。

## Prerequisites

- Phase 12 local qualification infrastructure complete。
- USER 已批准 v0.1.0 cold/warm startup hard gate 均为 400 ms；warm 180 ms 仅 preferred。
- 当前 Phase 12 旧收据只作历史诊断，不可用于 Phase 13 PASS。

## Scope

- std-only Rust environment preflight 与 machine-readable evidence。
- Runtime / Performance / Resources 前置门；资源分阶段 partial receipt。
- Phase 13 exact receipt suite 与五通道 readiness。
- M1..M5 人工 session helper，保留 P12-M01..M44 逐项状态。
- exact candidate freeze、local evidence、manual handoff 与 remote handoff。

## Out of Scope

- 产品功能、重构、性能优化、依赖升级。
- 自动批准 USER decision 或人工 waiver。
- 未经 USER 授权的 push、tag、GitHub workflow、draft Release 或 publish。

## Authority and Freeze

产品 authority 不变。新增代码只属于 `tools/stickymd-smoke` verification plane。完成本任务的
tracked tooling/docs 后提交一次 freeze commit；此后所有动态收据只进入 ignored
`dist/evidence/`。任何 source/tooling 变动都会形成新候选并使旧收据失效。

## Ordered Campaign

1. Qualification Environment。
2. Exact Release/package baseline。
3. `all --ci` headless baseline。
4. Runtime；失败或 blocked 时停止。
5. Performance。
6. Resources；场景间重检环境并持续写 `INCOMPLETE` partial receipt。
7. Readiness（人工前预检）。
8. USER 驱动 M1..M5 人工记录。
9. Local readiness consolidation。
10. 等待 USER 授权 push/remote；不 tag、不发布。

## Verification

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --locked`
- `cargo build --workspace --release --locked`
- core/render/win Release tests
- `cargo deny check`
- `cargo run -p stickymd-smoke --locked -- phase 13 --ci --json`
- `cargo run -p stickymd-smoke --locked -- all --ci --json`
- `git diff --check`

## Stop Conditions

- 环境门为 `ENVIRONMENT_BLOCKED` / `UNSUPPORTED` / `ERROR`。
- Runtime product failure。
- startup hard p95 超过 400 ms，排除环境失真后仍可复现。
- 任何 P0/P1、资源 hard gate、exact identity 或 package integrity 失败。
- 需要修改 frozen product/tooling source。

## Result

尚未形成。freeze 后的真实结果只写入 `dist/evidence/` 与最终聊天报告，不修改本文件。
