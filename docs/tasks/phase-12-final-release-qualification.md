# Phase 12 — Final Release Qualification

Status: In Progress

## Goal

把冻结的 v0.1.0 源码、portable artifact、自动化结果、人工结果与远端 workflow 绑定到同一
exact identity；在任何必要证据或 USER 授权缺失时 fail closed。

## Scope

- 收敛 Phase 12 治理、验收矩阵、决策账本与发布交接文档。
- 将 warm startup v0.1.0 hard boundary 校准为 USER 批准的 400 ms，同时保留 180 ms
  preferred target 与历史测量。
- 扩展 std-only `stickymd-smoke`，生成并校验 candidate、automated、manual、remote、
  downloaded-artifact 与 readiness receipts。
- 生成 clean exact local candidate；完成本地自动资格化。

## Inputs

- Phase 11/11-B 最终源码、报告、自动化与人工 `NOT TESTED` 清单。
- USER 于 2026-08-23 明确批准 warm startup v0.1.0 hard release boundary 为 400 ms。
- Phase 12 USER prompt。

## Deliverables

- `tools/smoke/phase-12.ps1` 与 `docs/acceptance-cases/phase-12.md`。
- `stickymd-smoke qualification ...`、exact-candidate decision projection 和
  `stickymd-smoke acceptance manual`。
- `docs/report/phase-12-release-decisions.md`、最终资格化报告与 handoff。
- ignored `dist/evidence/*.json` exact-artifact receipts。

## Verification

- Phase 12 CLI 单元测试与治理 contract。
- `all --ci` 无界面合并任务图。
- fmt、strict Clippy、workspace/core/render/win Release tests、deny、release build。
- exact package、SBOM、checksums、portable runtime smoke。
- readiness 在 manual/remote/USER decision 缺失时必须返回非零且解释 blockers。

## Out of Scope

- 新产品功能、架构重写或非阻塞性能微调。
- 未经 USER 授权的 push、tag、draft release、publish。
- 把真实 IME、视觉、物理多屏或 Clean VM 自动伪造为 PASS。

## Completion State

In Progress

当前本地治理与工具收敛可以完成；mandatory manual acceptance、release version、unsigned
policy、push/tag/draft/publish 与 remote/downloaded artifact evidence 仍需后续 USER gate。
