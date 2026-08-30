# Phase 11-B Final Interaction Amendment

## Completion State

Completed

## Goal

在不重开 Phase 11 架构收口的前提下，加入 Comrak 语义数学分隔符批量转换，并证明
Pin/Always-on-top 与 dock auto-hide 状态机正交。

## Inputs

- Phase 11 已冻结的 Document/editor/window authority。
- USER 批准的 delimiter conversion 与 Pin/auto-hide 正交性修订。
- Comrak owned semantic tree 与既有 window reducer。

## Authority

- 数学节点识别：Comrak owned semantic tree。
- canonical mutation：`DocumentState`，整批仅一个 `EditRequest`。
- Pin：durable window preference / z-order projection。
- Auto-hide：`WindowShellCoordinator` visibility reducer；不得读取 configured/effective topmost。

## Scope

- `\\(...\\)` 到 `$...$`、`\\[...\\]` 到 `$$...$$`。
- Source/Split 非空选择只转换 fully-contained 节点；Preview 或空选择转换整篇。
- 单步 Undo/Redo、正常 autosave/preview invalidation、220 DIP compact toolbar。
- Pin 开关下 focus/IME/manual/Esc/sensor/leave/floating 行为等价。
- Phase 11-B Rust smoke、PowerShell 薄入口、逐项验收矩阵和 Release 性能门。

## Out of Scope

- regex/manual Markdown parsing、math body rewrite、delimiter reverse conversion。
- window reducer 重写、新依赖、新产品功能、push/tag/release。

## Deliverables

- Comrak 语义驱动的 delimiter conversion 与单事务 Undo/Redo。
- Pin ON/OFF transition-equivalence 回归和 compact toolbar action。
- Phase 11-B smoke、acceptance matrix 与 Release 性能门。

## Verification

- `tools/smoke/phase-11-b.ps1`
- `tools/smoke/phase-11-b.ps1 -Performance`
- Phase 1–11 既有 smoke/release/package/resource regression。
- 人工项由 `docs/acceptance-cases/phase-11-b.md` 保持 `NOT TESTED`，直到有当前候选 receipt。

## Result

Semantic conversion, Pin/auto-hide orthogonality, exact-candidate automation and packaging are complete.
Phase 11 readiness remains blocked only by the unapproved warm-startup gate and manual `NOT TESTED` rows.
