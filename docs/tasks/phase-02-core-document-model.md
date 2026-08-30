# Phase 2 — Core Document Model

## Completion State

Completed

## Goal

Establish `DocumentState` as the sole runtime authority for canonical Markdown text and provide
typed UTF-8 edits, monotonic generations, immutable snapshots, and bounded undo/redo without any
UI, parser, filesystem, or Windows dependency.

## Inputs

- Phase 0 governing contracts are present.
- Phase 1 evidence limitations are recorded in
  `docs/report/phase-01-technical-spike-report.md`（重建后的证据边界）。
- USER authorized replacing implementation that diverged from the supplied Phase 2 contract.

## Scope

- `Generation` with checked, fail-closed advancement.
- Direction-preserving byte-offset `Selection` and `CursorSnapshot`.
- `EditRequest`, canonical `TextDelta`, and typed outcomes/errors.
- Private `StringTextStore`; no mutable text escape hatch.
- `DocumentState` edit/undo/redo/reconciliation/persist-ack gateways.
- Combined undo+redo limits: 256 entries and 4 MiB approximate payload.
- Immutable `DocumentSnapshot` with line-ending metadata.
- Deterministic Unicode roundtrip tests and Release performance baseline.

## Out of Scope

Grapheme navigation, IME preedit, UI, persistence I/O, preview, assets, workers, serialization,
window state, and managed-image side effects.

## Modules

| File | Responsibility |
| --- | --- |
| `document.rs` | sole authority and transactional mutation gateways |
| `edit.rs` | typed edit input, delta, metadata, outcomes |
| `selection.rs` | direction-preserving platform-independent positions |
| `generation.rs` | ordering token and checked advance |
| `snapshot.rs` | immutable worker/projection input |
| `text_store.rs` | private String storage primitive |
| `undo.rs` | deterministic grouping and bounded private history |
| `error.rs` | typed failure model |
| `line_ending.rs`, `hash.rs` | core metadata value types |

## Invariants

1. Only `DocumentState` mutates canonical text.
2. Invalid ranges, cursors, stale generations, and generation exhaustion fail before mutation.
3. Undo/redo failures do not pop history or advance generation.
4. No-op edits preserve generation and redo.
5. Undo/redo advance generation rather than restoring an old token.
6. An oversized edit succeeds canonically but is not stored in history.
7. A persistence acknowledgement cannot acknowledge a future generation.
8. A snapshot is immutable and non-authoritative.

## Deliverables

- Corrected core implementation and tests.
- `docs/report/phase-02-core-document-model.md`.
- Updated coverage matrix.

## Verification

- 稳定入口：`tools/smoke/phase-02.ps1`；`-Performance` 显式运行 String model baseline。
- 当前状态：`docs/acceptance-cases/phase-02.md`。
- 30 core unit tests and 5 integration/property tests pass.
- Release baseline covers append, middle insert/delete, snapshot, undo, and redo at 20 KiB,
  100 KiB, and 1 MiB.
- `stickymd-core` has zero unsafe and no window/render/platform dependency.

## Result

Contract-alignment rebuild complete. The String model remains valid: worst measured 1 MiB common
edit p95 was 12.1 µs, far below the 50 ms exploratory gate. End-to-end Source/IME acceptance is
not claimed here.
