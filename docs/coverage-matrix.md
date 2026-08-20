# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Code Area ↔ Current Evidence。
> `Current Evidence` 只描述已验证范围；不得用模块存在代替端到端验收。

| Plan | Feature（投影） | Acceptance | Code Area | Current Evidence |
| --- | --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | future lifecycle | contract only |
| `04_runtime_state_model.md` | 内部 authority | AC-009、AC-013 | `stickymd-core/src/{document,edit,selection,generation,snapshot}.rs` | core contract automated PASS；end-to-end pending |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005..008、AC-026、AC-027、AC-030 | future Windows I/O adapter | rebuilt spike: 9 automated tests + Windows smoke PASS；junction/non-ASCII Windows case/ACL/kill NOT TESTED；no production implementation |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片 | AC-013..017 | future `stickymd-render` preview modules | rebuilt spike: 6 semantic/math tests PASS；production RaTeX painter path conditional；no production preview |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo | AC-002、AC-003、AC-004、AC-009、AC-022 | `stickymd-render/src/source/*`; `stickymd-win/{instruction,flow,interaction}` | automated pipeline PASS；AC-003/004 real IME NOT TESTED |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | future asset subsystem | contract only |
| `09_windows_shell.md` | dock、托盘、置顶、透明度、主题、多显示器 | AC-019..029 | future product shell; Phase 3 dev shell | five-run Source shell idle/memory sample；DPI/dock/tray/opacity not implemented |
| `10_performance_reliability.md` | 质量属性 | AC-022、空闲/内存观察 | core benchmark + Phase 1/3 baselines | Source shell WS median 31.57 MiB；1 MiB ordinary command worst p95 1.567 ms，newline 1.534 ms；exceptional full resync p95 53.227 ms；real IME NOT TESTED |
| `11_testing_and_release.md` | 发布形态 | release 验收清单 | CI / future release tools | not implemented |

---

## 未覆盖声明

- `00_engineering_constitution.md` 与 `01_terminology.md` 是治理基座，
  不直接映射验收案例；其约束通过上述所有章节间接验证。
- `03_system_architecture.md` 的层间规则通过 code review、`plan_ref` 审查
  与各案例的实现结构间接验证。

## 维护规则

1. 新增 plan 章节 → 必须补充对应 Feature 段落与 Acceptance 案例（或写明不适用理由）。
2. Acceptance 案例失效 → 标记 Deprecated，编号不复用。
3. Code Area 或验证状态变化 → 同步更新本表；部分实现不得标记完整 AC PASS。
