# Phase 0 Governance Revalidation

- `Date`: 2026-08-20
- `Type`: implementation-alignment and architecture-contract review
- `Status`: PASS after targeted corrections

## Executive Result

Phase 0 的主体没有必要整体删除。工程宪法、权威顺序、术语表、四层 + Object Plane、
30 个验收案例和文档投影关系均有实质内容。问题集中在契约可执行性，而不是产品本体错误。

## Requirement-to-Evidence Ledger

| Requirement | Evidence | Result |
| --- | --- | --- |
| Engineering Constitution 完整落盘 | 归一化后 USER archive 正文 378 行与 plan 正文 378 行逐行相同 | PASS |
| Authority order | 根 `AGENTS.md` 与 `docs/AGENTS.md` | PASS |
| 术语四字段 | `01_terminology.md` 的 Definition/Authority/Not equivalent/Lifetime | PASS |
| 四层 + Object Plane | `03_system_architecture.md` + 五条完整调用链 | PASS |
| runtime authority | `04_runtime_state_model.md` | PASS after correction |
| persistence failure model | `05_document_persistence.md` | PASS after correction |
| feature projection | `docs/features/00_v1_product_behavior.md` | PASS |
| acceptance cases | AC-001..AC-030；每例均有 Preconditions/Action/Expected/Failure Signals | PASS |
| coverage matrix | `docs/coverage-matrix.md` | PASS，继续按实现阶段更新 |
| ADR / PR governance | ADR template 与 PR checklist | PASS |

## Defects Found and Corrected

1. `docs/plan/AGENTS.md` 要求 operational chapter 使用完整 contract headings，但 02–11
   多章缺少 Inputs/Outputs/State Changes 等同名标题。现已补齐；00/01 的受控例外明确写出。
2. 旧 `plan_ref` 依赖中文/标点自动 slug，且若干实验引用不存在的 plan 文件。现在 governing
   plan 使用显式 ASCII anchor；production module refs 已改到这些 anchor。
3. 原“输入文字”调用链让 UI 提供 `deleted`，破坏 DocumentState authority。现在 UI 只提交
   expected generation、range、inserted 与 cursor/meta；deleted 由 canonical text 派生。
4. Generation 原规则未明确 checked overflow、undo/redo/recovery 递增与 preedit/caret 不递增。
   现已补齐 fail-closed 语义。
5. 原子保存对 `ReplaceFileW` fallback 的“安全条件”过于空泛。现在区分首次创建、已存在
   替换、竞争与未知错误；未知错误不得无条件降级覆盖。

## Architecture Review

- 系统本体仍是一张 portable Markdown 草稿纸；没有多文档、知识管理或插件方向。
- Interaction Shell 仍只有转译 + 呈现；DocumentState mutation 只能经 typed intent 和
  coordinator/domain gateway。
- Preview 严格是 snapshot projection；磁盘变化必须 reconcile。
- managed asset ownership 与 user asset 不删除边界未改变。
- Windows-specific 能力仍隔离在 adapter；RichEdit 仍只是受审批 contingency。

## Scope

本次没有删除 USER 工程宪法、冻结规格或已有 acceptance 编号；只修复被 USER 授权的
Phase 0 contract drift。没有加入运行时功能或依赖。
