# docs/acceptance-cases/AGENTS.md

`docs/acceptance-cases/` 是**验证合同**的投影文档树。

规则：

- 每个案例写成可验证形式：Preconditions / Action / Expected / Failure Signals。
- 案例只验证 `docs/plan/` 与 `docs/features/` 中已批准的行为；**不得发明产品需求**。
- 案例 ID 一旦发布不得复用；废弃时标记 `Deprecated` 并保留编号。
- 新增契约章节时必须同步补充案例并更新 `docs/coverage-matrix.md`。
- 案例描述的是“如何判定通过”，不是实现步骤；实现细节属于 plan。
- 每个 Phase 必须提交 `phase-XX.md`，逐项映射 plan/全局 AC、验证模式、仓库内入口、
  当前状态与剩余证据。
- 状态只允许 `AUTOMATED PASS`、`MANUAL PASS`、`NOT TESTED`、`BLOCKED`；不得使用
  无证据的裸 `PASS` 或 `CONDITIONAL`。
- 人工项没有当前提交的正式矩阵收据时必须保持 `NOT TESTED`；一次性终端记录、旧报告、
  未提交脚本或自动化替代动作均不能算人工验收。
