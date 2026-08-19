# phase-02-core-document-model.md - Phase 2 任务记录（核心文档模型）

## Goal

在 `stickymd-core`（平台无关、`#![forbid(unsafe_code)]`）中实现契约
`docs/plan/04_runtime_state_model.md` 定义的核心文档对象：DocumentState、
DocumentSnapshot、Generation、TextDelta/TextStore、CursorSnapshot、
UndoManager、Hash32、换行风格，使后续 Flow Coordination 与 Execution Domain
层可以围绕一个可测试的运行时权威状态展开。

## Scope

只实现 Object Plane 的文档域对象与其不变量；不触碰文件系统、窗口、Markdown、
Preview、资产。磁盘哈希的计算在 Execution Domain；core 只持有/比较 `Hash32`。

## Inputs

- USER Phase 2 任务 prompt（核心文档模型实现目标与验收）。
- Phase 1 结论：`docs/report/phase-01-technical-spike-report.md`
  （Recommendation A 被 USER 接受，允许进入 Phase 2）。
- `docs/plan/04_runtime_state_model.md`（状态模型与 invariant 1–10）、
  `docs/plan/07_editor_and_ime.md`（undo 分组合同）、
  `docs/plan/05_document_persistence.md`（saved generation / hash 语义）、
  `docs/plan/01_terminology.md`（术语）。

## Deliverables

- Phase 2A：`generation.rs`、`hash.rs`、`line_ending.rs`、`cursor.rs`、
  `text_delta.rs`、`text_store.rs`（StringTextStore）。
- Phase 2B：`undo.rs` —— 差分 UndoEntry + UndoManager（合并/淘汰/redo 清空）。
- Phase 2C：`document.rs` —— DocumentState / DocumentSnapshot / apply_delta /
  undo / redo / acknowledge_persisted / load_external；`error.rs` 增加
  `EditError::InvalidRange` 与 `PersistAckError`。
- Phase 2D：`tests/property.rs`（4 个 property/roundtrip 测试 + perf smoke）；
  fmt / clippy（`-D warnings`）/ tests 全绿。
- Phase 2E：本报告 + `docs/report/phase-02-core-document-model.md`。

## Verification

- `cargo test -p stickymd-core`：38 unit + 5 property 全部通过。
- `cargo fmt --all --check`：通过。
- `cargo clippy -p stickymd-core --all-targets --locked -- -D warnings`：0 警告。
- invariant 覆盖见报告 `phase-02-core-document-model.md`。

## Out of Scope

- 文件读写与 watcher（Phase 3+，Execution Domain）。
- IME preedit 呈现与编辑器后端（Phase 3+）。
- Preview / Markdown / 数学（render crate，后续阶段）。
- 资产、窗口、配置对象。

## Completion State

Completed —— 全部 43 个测试通过，fmt/clippy 干净，perf smoke 达标，
结果已写入 `docs/report/phase-02-core-document-model.md`。
