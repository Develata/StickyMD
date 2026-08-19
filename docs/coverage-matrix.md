# coverage-matrix.md - 契约覆盖矩阵

> Plan Contract ↔ Feature Projection ↔ Acceptance Case ↔ Future Code Area。
> Future Code 为规划名称，不代表已存在。新增章节/案例时必须同步更新本表。

| Plan | Feature（投影） | Acceptance | Future Code |
| --- | --- | --- | --- |
| `02_positioning_and_scope.md` | 产品定位、便签模型 | AC-001、AC-026、AC-027 | — |
| `04_runtime_state_model.md` | （内部模型，无直接用户投影） | AC-009、AC-013 | `stickymd-core`（state/document） |
| `05_document_persistence.md` | 自动保存、外部修改冲突、崩溃恢复 | AC-001、AC-005、AC-006、AC-007、AC-008、AC-026、AC-027、AC-030 | `stickymd-core`、Windows I/O adapter |
| `06_markdown_math_rendering.md` | 预览、数学、raw HTML、remote 图片 | AC-013、AC-014、AC-015、AC-016、AC-017 | `stickymd-render` |
| `07_editor_and_ime.md` | 源码输入、中文输入法、Undo/Redo | AC-002、AC-003、AC-004、AC-009、AC-022 | editor backend（`stickymd-win/editor`） |
| `08_assets_and_export.md` | 图片粘贴、managed GC、导出 | AC-010、AC-011、AC-012、AC-017、AC-018 | asset 子系统（core + I/O adapter） |
| `09_windows_shell.md` | dock、托盘、置顶、透明度、主题、多显示器 | AC-019..AC-029 | `stickymd-win`（shell + platform/windows） |
| `10_performance_reliability.md` | （质量属性，无直接行为投影） | AC-022（typing guard）、空闲/内存观察 | 全仓库 + benchmark |
| `11_testing_and_release.md` | 发布形态 | （release 验收清单） | CI / release 工具链 |

---

## 未覆盖声明

- `00_engineering_constitution.md` 与 `01_terminology.md` 是治理基座，
  不直接映射验收案例；其约束通过上述所有章节间接验证。
- `03_system_architecture.md` 的层间规则通过 code review、`plan_ref` 审查
  与各案例的实现结构间接验证。

## 维护规则

1. 新增 plan 章节 → 必须补充对应 Feature 段落与 Acceptance 案例（或写明不适用理由）。
2. Acceptance 案例失效 → 标记 Deprecated，编号不复用。
3. Future Code 名称变化 → 同步更新本表。
