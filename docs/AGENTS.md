# docs/AGENTS.md — 文档树职责

本目录是 StickyMD 的文档治理树。各子目录职责固定如下：

```text
docs/plan/
    authoritative engineering contract
    （工程骨架与合同的唯一权威；修改需审批）

docs/features/
    user-visible product behavior projection
    （用户可见行为投影；只描述行为，不定义架构）

docs/acceptance-cases/
    verification contract
    （可验证验收案例；只验证已批准行为，不发明需求）

docs/overview/
    readable architecture projection
    （可读架构投影；供快速理解，细节以 plan 为准）

docs/adr/
    decision history, non-authoritative against current plan
    （决策历史，只解释 why；不凌驾于当前 plan）

docs/report/
    dated analysis/evidence, non-authoritative
    （有时间属性的分析证据；不是长期权威）

docs/tasks/
    phase implementation plans
    （阶段实施计划与完成状态记录）

docs/phases/
    USER prompt archive, non-authoritative against current plan
    （USER 阶段提示词输入证据；用于追溯，不与当前 plan 建立平级权威）

docs/reference/
    external technical references, never overrides plan
    （外部技术参考；永远不得覆盖 plan）
```

```text
docs/coverage-matrix.md
    plan ↔ feature ↔ acceptance ↔ code/evidence 对照表
```

---

## 投影方向（只允许单向）

```text
docs/plan → features / acceptance-cases / overview → code
```

禁止反向：

- **不要在 feature 文档重新定义架构。**
- **不要在 acceptance 文档发明产品需求。**
- **不要在 report 中建立永久权威。**
- 不要在 overview 中引入与 plan 不一致的细节；发现不一致时修 overview，不修 plan。

---

## 文档规则

- 术语必须使用 `docs/plan/01_terminology.md` 的固定名称。
- 相对链接必须可解析；重命名文件时同步更新引用。
- report 文件名包含主题或阶段（如 `phase-00-*.md`、`RISK-*.md`），便于按时间归档。
- 新增 projection 章节时，同步更新 `docs/coverage-matrix.md`。
