# Phase 2 Core Document Report

- `Date`: 2026-08-20
- `Status`: Completed after contract-alignment rebuild; awaiting USER review
- `Starting commit`: `3094dbb`
- `Authority`: `docs/plan/04_runtime_state_model.md`, `05_document_persistence.md`,
  `07_editor_and_ime.md`

## Executive Result

| Contract | Result |
| --- | --- |
| Document authority | PASS |
| String TextStore model | PASS |
| TextDelta / stale protection | PASS |
| Checked monotonic generation | PASS |
| Undo/Redo correctness and bounds | PASS |
| 1 MiB String model | PASS |

The previous implementation at `4018a83` did not satisfy the supplied Phase 2 contract. It used
saturating generation, rejected reverse selections, changed state for no-op edits, lacked stale
edit protection, retained oversized undo entries, popped history before complete validation, and
restored the wrong cursor after grouped backspace. Those paths were removed rather than patched
around.

## Final Module Map

| File | Responsibility |
| --- | --- |
| `document.rs` | canonical state, validation, transactional mutation, reconciliation, persist ack |
| `edit.rs` | `EditRequest`, `EditMeta`, `EditKind`, `TextDelta`, typed outcomes |
| `selection.rs` | `TextPosition`, direction-preserving `Selection`, `CursorSnapshot` |
| `generation.rs` | checked monotonic ordering token |
| `snapshot.rs` | immutable `Arc<str>` snapshot with generation/line ending |
| `text_store.rs` | private `StringTextStore` and UTF-8 range validation |
| `undo.rs` | private bounded history and conservative grouping |
| `error.rs` | typed failure model |

## Public Contract

- `DocumentState::{loaded, empty, edit, undo, redo, acknowledge_persisted,
  replace_from_reconciliation, snapshot}`
- read-only accessors for text/generation/saved generation/line ending/hash/history capability
- `EditRequest` carries expected generation; deleted text is always read from canonical state
- `TextDelta` exposes immutable ranges/content/cursor snapshots, not storage internals
- no `text_mut`, public undo stack, global state, or runtime serialization

## Invariants Proven

| Invariant | Evidence |
| --- | --- |
| single mutation authority | private store; all mutations route through `DocumentState` |
| UTF-8 boundaries | range and cursor validation before replacement |
| monotonic generation | `checked_next`; edit/undo/redo/reconciliation advance; no-op/ack do not |
| failure atomicity | validation and next-generation computation precede text/history mutation |
| redo invalidation | successful new canonical edit clears redo; no-op preserves it |
| bounded history | undo+redo combined payload ≤4 MiB and entry count ≤256 |
| stale edit protection | expected generation mismatch returns `StaleEdit` |
| persistence receipt safety | future generation rejected; stale receipt cannot mark current text clean |

## Undo Grouping

| Kind | Merge | Rule |
| --- | --- | --- |
| Typing | yes | adjacent, cursor-continuous, same kind, ≤750 ms |
| Backspace | yes | contiguous reverse deletion; original deleted order preserved |
| DeleteForward | yes | contiguous forward deletion; original deleted order preserved |
| Paste | no | standalone |
| ImeCommit | no | one commit = one entry |
| Newline | no | standalone |
| SelectionReplace / Other | no | standalone |

Payload accounting is deterministic: deleted bytes + inserted bytes + 128 bytes metadata estimate.
Undo and redo own one combined budget. An entry larger than 4 MiB clears old history and is not
recorded, while the canonical edit still succeeds.

## Unicode Verification

ASCII, CJK, composed/decomposed Latin (`é`, `e\u{301}`), emoji, family ZWJ emoji, newlines,
and mathematical Unicode fixtures pass deterministic edit→undo-all→redo-all roundtrips. Core
operates on valid UTF-8 byte ranges and deliberately does not implement grapheme navigation.

## Release Performance

Command: `cargo bench -p stickymd-core --bench release_baseline --locked`.
Fixture setup and warm-up are excluded from samples.

| Size | Append p95 | Middle insert p95 | Middle delete p95 | Snapshot p95 | Undo p95 | Redo p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.3 µs | 0.4 µs | 0.3 µs | 0.5 µs | 0.3 µs | 0.2 µs |
| 100 KiB | 0.3 µs | 1.2 µs | 1.1 µs | 55.0 µs | 1.0 µs | 1.0 µs |
| 1 MiB | 0.3 µs | 12.1 µs | 11.1 µs | 341.1 µs | 9.7 µs | 10.2 µs |

The 1 MiB common-edit p95 gate (<50 ms) passes with substantial margin. Snapshot is an explicit
O(n) copy and occurs only at worker/projection boundaries.

## Dependencies and Unsafe

- Runtime dependency: `thiserror` only.
- No `winit`, `cosmic-text`, `comrak`, `ratex`, `windows`, `softbuffer`, or `tiny-skia` dependency.
- `stickymd-core` retains `#![forbid(unsafe_code)]`; runtime unsafe count is zero.

## Deferred Responsibilities

- grapheme navigation and IME preedit → Phase 3 editor session
- persistence I/O and conflict coordination → later persistence phase
- managed asset effects → asset phase; private `UndoEntry` can be extended without changing the
  public document API
- preview consumption → future generation-tagged preview projection

## Architecture Drift

None after rebuild. The public mutation model matches the approved runtime authority contract.

## Recommendation

`APPROVE next phase WITH CONDITIONS`: Phase 3 automated implementation may proceed, but real
Microsoft Pinyin and WeChat IME acceptance remains a blocking manual gate.
