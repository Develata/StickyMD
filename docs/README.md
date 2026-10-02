# StickyMD 文档入口

这里按用途组织文档。工程合同、当前源码说明、验收结果和历史报告分别回答不同问题，不能互相替代。

## 当前状态

`v0.1.2` 已从 exact source `e36e61d65536c0cbaa84ee4004f1a289fbcb91dd` 发布。
该版本使用明确的 USER 发布特例，技术 readiness 仍为 `NOT_READY`；完整桌面资格化和
人工验收缺口见 [发布说明](release-notes/0.1.2.md) 与
[发布决定](report/2026-10-02-v0.1.2-release-preparation.md)。后续 `main` 的代码、工具和文档
维护不继承该发布身份或特例。

## 按问题查找

| 想了解什么 | 入口 |
| --- | --- |
| 如何使用、快捷键、产品边界 | [项目 README](../README.md)、[用户行为](features/00_v1_product_behavior.md) |
| 架构和各模块的职责 | [架构概览](overview/architecture.md) |
| 工程约束与术语 | [工程宪法](plan/00_engineering_constitution.md)、[术语表](plan/01_terminology.md) |
| Markdown / 数学应该怎样渲染 | [渲染合同](plan/06_markdown_math_rendering.md) |
| 当前 Preview 实际规则、冲突处理、参数和已知差异 | [Preview 实现详解](report/2026-09-30-preview-rendering-rules.md)，含 [勘误与诊断](report/2026-09-30-preview-rendering-rules.md#review-2026-10-01)及 [2026-10-01 图片、列表、字体修复](report/2026-09-30-preview-rendering-rules.md#fixes-2026-10-01) |
| 图片读取、粘贴与导出的边界 | [图片与导出合同](plan/08_assets_and_export.md) |
| 本地改动要跑什么、如何看耗时 | [贡献指南](../CONTRIBUTING.md#验证改动)、[Rust CLI 说明](../tools/stickymd-smoke/README.md#local-change-based-checks) |
| 自动化覆盖了什么、哪些还没验收 | [覆盖映射](coverage-matrix.md)、[全局验收案例](acceptance-cases/00_v1_acceptance.md)、[Phase 14](acceptance-cases/phase-14.md) |
| 打包、候选身份与发布资格 | [测试与发布合同](plan/11_testing_and_release.md)、[发布工具入口](../tools/stickymd-smoke/README.md#release-tooling) |
| 为什么作出某个决定、当时有哪些证据 | [报告索引](report/README.md)、[ADR 目录说明](adr/AGENTS.md) |

## 阅读与维护顺序

1. `plan/` 是工程合同的唯一权威；实现和其他文档与之冲突时，先记录实现差异。
2. `features/`、`acceptance-cases/`、`overview/` 是投影；分别描述行为、验证与架构概览。
3. `report/` 记录带日期和来源身份的调查结果；后续结论以追加 Resolution 保留历史。
4. `tasks/`、`phases/`、`reference/` 用于实施追踪、提示词归档和外部参考，不另立合同。

普通 CI 成功、本地选测成功、包有效、候选身份正确和完整发布资格是不同结论。
人工项没有对应正式证据时仍为 `NOT TESTED`；历史报告的耗时和截图不能作为当前源码的新验收。
具体目录规则见 [docs/AGENTS.md](AGENTS.md)。
