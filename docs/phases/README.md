# docs/phases — USER Prompt Archive

本目录保存 USER 提供的阶段提示词原文，用于需求追溯、实施复核和审计。

这些文件是输入证据快照，不是与 `docs/plan/` 平级的长期架构权威。持续有效的
工程合同必须收敛到 `docs/plan/`；若提示词、历史报告与当前 plan 出现冲突，应按根
`AGENTS.md` 的 authority order 处理，并在需要骨架变更时请求 USER 批准。

## Archived prompts

- [`2026-08-20-v1-master-spec-phase-00-03-prompts.md`](2026-08-20-v1-master-spec-phase-00-03-prompts.md)
  - 内容：StickyMD v1 主规格以及 Phase 0、1、2、3 的完整 Agent 提示词。
  - 来源 SHA-256：`AC8CA0003409FB61E0C2D72A2642CECC207D0E94B21EBDD1DE1173A40F90153B`
  - 导入方式：逐字节归档；未对正文做规范化或改写。
- [`2026-08-20-phase-05-markdown-native-preview.md`](2026-08-20-phase-05-markdown-native-preview.md)
  - 内容：Phase 5 Markdown semantic pipeline、owned AST、native Preview、smoke 与验收任务原文。
  - 来源 SHA-256：`781C97D026427D4AFB7CC90871177F3278D8443AFF6897F20E5FD85CDC663326`
  - 导入方式：逐行原文归档；仅由 `apply_patch` 统一为仓库换行格式，未改写正文。
- [`2026-08-20-phase-06-ratex-native-math.md`](2026-08-20-phase-06-ratex-native-math.md)
  - 内容：Phase 6 RaTeX native math layout、rendering、cache、smoke 与验收任务原文。
  - 来源 SHA-256：`F61ACD5D059A77DE25F865F3EB87F88BB056B5052718B55732A105046A5138F0`
  - 导入方式：逐行原文归档；仅由 `apply_patch` 统一为仓库换行格式，未改写正文。
- [`2026-08-20-phase-07-managed-images-export.md`](2026-08-20-phase-07-managed-images-export.md)
  - 内容：Phase 7 managed images、clipboard、asset transaction/GC、native image Preview、export、smoke 与验收任务原文。
  - 来源 SHA-256：`F75B051E220F4E3C73FE0B202D870B0F745E8D74B336AFF4D58028C9CF9B22AA`
  - 导入方式：逐行原文归档；仅由 `apply_patch` 统一为仓库换行格式，未改写正文。
