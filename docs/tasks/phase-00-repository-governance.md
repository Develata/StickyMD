# phase-00-repository-governance.md - Phase 0 任务记录

## Goal

在任何正式 Rust 功能代码出现之前，建立 StickyMD 的仓库治理骨架、Agent 执行规则、
工程宪法、术语体系、产品边界、架构蓝图、状态/数据责任、验证入口与文档投影关系。

## Scope

纯 contract-first：仅治理与架构文档、目录结构、模板与仓库基础配置文件。
不实现任何运行时功能。

## Inputs

- USER Phase 0 任务 prompt（含已冻结产品事实与工程宪法原文）。
- StickyMD v1 主规格（USER 已批准）。
- 仓库现状：clean 工作树，仅 `.gitignore` 与 MIT `LICENSE`。

## Deliverables

- 根级：`AGENTS.md`、`README.md`、`.gitattributes`、`.gitignore`（合并更新）、
  `.github/pull_request_template.md`。
- `docs/AGENTS.md`、`docs/coverage-matrix.md`。
- `docs/plan/`：`AGENTS.md` + `00_engineering_constitution.md`（USER 宪法全文）
  + `01_terminology.md` + `02`–`11` 各契约章节。
- `docs/features/`：`AGENTS.md` + `00_v1_product_behavior.md`。
- `docs/acceptance-cases/`：`AGENTS.md` + `00_v1_acceptance.md`（AC-001..AC-030）。
- `docs/overview/`：`AGENTS.md` + `architecture.md`。
- `docs/adr/`：`AGENTS.md` + `README.md` + `0000-template.md`。
- `docs/report/`：`AGENTS.md` + `README.md` + Phase 0 现状检查记录。
- `docs/tasks/`：`AGENTS.md` + 本文件。
- `docs/reference/`：`AGENTS.md` + `README.md`。

## Verification

- `git diff --check` 无空白错误。
- 相对链接人工检查（全部可解析）。
- 宪法与 USER 原文逐节比对，无删节。
- Phase 0 自检清单（术语一致、无 Rust 代码、无依赖、无禁止方向、
  单一架构真相源）通过。
- 架构 review 15 问通过（见本文 Completion State）。

## Out of Scope

- Cargo workspace、任何 crate、`src/main.rs`、运行时代码。
- 技术 spike、UI 原型、benchmark 实现。
- GitHub Actions workflow、release 流程实现。
- crate 版本锁定（待技术验证阶段核实）。

## Completion State

`Completed`（2026-08-19）

说明：本阶段仅完成治理与契约文档。
**任何运行时能力均未实现，也不得据本文声称已具备。**
