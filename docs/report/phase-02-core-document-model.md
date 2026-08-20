# phase-02-core-document-model.md - Phase 2 核心文档模型实现报告

- `Date`: 2026-08-19
- `Type`: 阶段实现结果报告
- `Status`: Completed

plan_ref: docs/plan/04_runtime_state_model.md#核心-invariant ; docs/plan/07_editor_and_ime.md#undo-分组 ;
docs/plan/05_document_persistence.md ; docs/plan/01_terminology.md

前置门：Phase 1 spike 总报告 Recommendation A 已获 USER 接受
（`phase-01-technical-spike-report.md`）。

任务记录：`docs/tasks/phase-02-core-document-model.md`。

---

## 1. 交付内容

全部位于 `crates/stickymd-core`（平台无关，`#![forbid(unsafe_code)]`，
唯一外部依赖 `thiserror`）。

| 模块 | 行数 | 职责 |
| --- | --- | --- |
| `generation.rs` | 53 | `Generation` 单调版本号（saturating） |
| `hash.rs` | 76 | `Hash32` 磁盘内容指纹（core 只持有/比较，摘要在 Execution Domain 计算） |
| `line_ending.rs` | 106 | 换行风格检测 / 内部 `\n` 归一 / 落盘还原 |
| `cursor.rs` | 91 | `CursorSnapshot`（byte offset + selection + generation，可校验有效性） |
| `text_delta.rs` | 158 | `TextDelta` 校验（InvalidRange/OutOfBounds/NotCharBoundary）+ `InputKind` |
| `text_store.rs` | 103 | `TextStore` trait + `StringTextStore`（失败不破坏存储） |
| `undo.rs` | 359 | 差分 `UndoEntry` + `UndoManager`（合并窗口 / 256 条 / 4 MiB 双限） |
| `document.rs` | 370 | `DocumentState` / `DocumentSnapshot` / apply / undo / redo / ack / 外部载入闸门 |
| `error.rs` | 56 | `EditError`（+InvalidRange）、`PersistError`、`PersistAckError` |
| `tests/property.rs` | 222 | property/roundtrip fuzz + perf smoke |

`document.rs`(370) 与 `undo.rs`(359) 超过 ~250 行 soft 参考线、未达 ~500 行
hard 线；职责单一（状态推进 / undo 合同），暂不拆分，后续阶段若继续增长
将按职责边界拆分。

## 2. 契约实现要点

- **DocumentState 是唯一运行时权威**：所有文本修改经 `apply_delta` 单一入口；
  delta 与前后 cursor 均先校验，任一失败整体回滚，存储不被部分修改。
- **Generation 语义**：每次成功修改（含 undo/redo）+1；`snapshot()` 产出
  `Arc<str>` + generation 的只读快照，快照不可写回。
- **saved generation 只认实际落盘**：`acknowledge_persisted(persisted, hash)`
  拒绝 `persisted > generation`（`PersistAckError::AheadOfDocument`）；
  落后于 saved_generation 的过期回执为 no-op。
- **外部内容闸门**：`load_external` 是外部磁盘内容进入 DocumentState 的唯一
  入口，执行即清空 undo（合同：外部 reload 不进 undo）并回到 clean。
- **Undo 合同（plan 07）**：仅内存、256 条或 4 MiB 先到者淘汰最老条目
  （最后一条永不淘汰）；合并条件 = 相邻位置 + 同类输入（Typing/Backspace/
  Delete）+ <750 ms；IME commit / 粘贴 / Newline / DeleteSelection 恒独立
  （`InputKind` 非 mergeable 类型）。

## 3. Invariant 覆盖（plan 04）

| Invariant | 本阶段状态 | 证据 |
| --- | --- | --- |
| #2 外部变化须经 reconciliation | 已实现并测试：外部文本只能经 `load_external` 进入 | `document::tests::load_external_clears_undo_and_marks_clean` |
| #4 过期 preview generation 不提交 | 不适用（本阶段无 Preview）；generation 机制已就绪 | — |
| #7 saved_generation ≤ 实际落盘 generation | 已实现并测试：ahead 拒绝、stale no-op | `document::tests::persist_ack_never_exceeds_generation`、`stale_persist_ack_is_noop`、`persist_ack_advances_saved_generation` |
| #9 后台任务不得持有可变引用 | 类型级保证：后台任务只能拿到 `DocumentSnapshot`（`Arc<str>`，只读） | `snapshot()` API 形状 |
| #6 preedit 非规范文本 | 结构性保证：文本只能经 committed `TextDelta` 进入；preedit 呈现对象留待编辑器阶段 | — |

其余 invariant（#1/#3/#5/#8/#10）属于 Preview/资产/窗口/持久化写入层，
在对应阶段落实。

## 4. 验证结果

| 项 | 结果 |
| --- | --- |
| `cargo test -p stickymd-core` | **38 unit + 5 property 全通过（43/43）** |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy -p stickymd-core --all-targets --locked -- -D warnings` | 0 警告 |
| perf smoke（debug profile，1 MiB 文档） | append ≈ 2.2 µs/op；中段插入 ≈ 11.6 µs/op |

Property 测试覆盖：任意 Unicode delta（ASCII/CJK/emoji/combining mark）
不破坏 UTF-8；无合并与合并分组两种节奏下的完整 undo→原文、redo→终态
roundtrip；非 boundary delta 恒被拒绝且状态不变。

注：perf smoke 为 debug 构建，且未对照 plan 10 的正式延迟 gate
（该 gate 属于生产按键路径，后续阶段测量）。

## 5. NOT TESTED（如实声明）

- 真实输入法（微软拼音/微信输入法）commit 行为：本阶段无 UI，无法人工验证。
- 真实文件系统交互（原子替换、watcher、hash 比对）：属 Execution Domain，
  Phase 3+；Phase 1 persistence spike 已单独验证 API 可行性。
- 多线程/并发场景：core 当前为单线程模型，快照只读共享由类型保证，
  但未做并发压测。
- Release profile 性能：perf smoke 数字来自 debug 构建。

## 6. 结论

Phase 2 目标全部完成：核心文档模型已按 plan 04/05/07 契约实现并通过
43 个测试与 fmt/clippy 门禁。建议下一步进入 Phase 3（Execution Domain
持久化接入 / Flow Coordination 骨架，由 USER 指定）。

---

## Phase 3 Preflight Release Baseline

Measured during Phase 3 preflight（`cargo bench -p stickymd-core
--bench release_baseline`，release/bench profile，Phase 2 commit `4018a83`
之上；确定性 fixture，warm-up 与 setup 不计入，edit 类 n=1000、
snapshot/undo/redo n=200，机器：本机 Windows 11 x64）。

| 操作 | 20 KiB (23 779 B) median / p95 / max | 100 KiB (105 698 B) | 1 MiB (1 051 873 B) |
| --- | --- | --- | --- |
| append | 100 ns / 200 ns / 18.8 µs | 100 ns / 200 ns / 2.2 µs | 100 ns / 200 ns / 30.9 µs |
| middle insert | 300 ns / 300 ns / 400 ns | 1.2 µs / 1.3 µs / 8.2 µs | 10.5 µs / 12.8 µs / 161.4 µs |
| middle delete | 200 ns / 300 ns / 10.4 µs | 1.1 µs / 1.2 µs / 8.2 µs | 11.0 µs / 11.7 µs / 30.7 µs |
| snapshot | 500 ns / 500 ns / 10.8 µs | 2.7 µs / 2.9 µs / 52.6 µs | 441.6 µs / 591.3 µs / 795.7 µs |
| undo | 300 ns / 400 ns / 2.5 µs | 1.1 µs / 1.2 µs / 10.2 µs | 12.4 µs / 15.8 µs / 39.9 µs |
| redo | 200 ns / 300 ns / 300 ns | 1.0 µs / 1.1 µs / 9.3 µs | 11.9 µs / 12.8 µs / 46.5 µs |

### StringTextStore Gate

要求：1 MiB common edit（append / middle insert / middle delete）p95 < 50 ms。
实测最差 p95 = 12.8 µs，约 4 个数量级的裕量。

```text
StringTextStore Phase 3 Gate: PASS
```

说明：snapshot 为全文 `Arc<str>` 复制（1 MiB p95 ≈ 0.59 ms），属预期成本；
本节仅补 Release 基线，不改变上文 debug 数字与既有结论。
