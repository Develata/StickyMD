# docs/features/AGENTS.md

`docs/features/` 是**用户可见行为**的投影文档树。

规则：

- 只从用户视角描述“看得见、摸得着”的行为。
- 禁止放内部架构、线程模型、状态机、库名与实现细节。
- 不得重新定义架构；架构真相在 `docs/plan/`。
- 行为变更必须先修改 `docs/plan/` 契约，再同步更新这里的投影。
- 新增行为描述必须能在 `docs/acceptance-cases/` 找到对应验收案例。
