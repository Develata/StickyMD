# Phase 3 Source Editor and IME Report

- `Date`: 2026-08-20
- `Status`: Automated implementation PASS; manual IME gate OPEN
- `Starting commit`: `3094dbb`

## Executive Result

| Area | Result |
| --- | --- |
| typed intent / coordination boundary | PASS |
| canonical document authority | PASS |
| Unicode grapheme editing | PASS (automated) |
| source projection / paint / scrolling | PASS (automated) |
| incremental input latency | PASS |
| clipboard failure atomicity | PASS |
| synthetic IME state machine | PASS |
| Microsoft Pinyin | **NOT TESTED** |
| WeChat IME | **NOT TESTED** |
| candidate-window visual position | **NOT TESTED** |

Phase 3 code is suitable for USER testing but is not a completed product editor. The executable
title and diagnostic explicitly state `NOT PERSISTED`.

## Boundary Evidence

- Interaction Shell translates `winit` events and paints effects; it cannot obtain
  `&mut DocumentState`.
- `EditorCoordinator` owns `DocumentState` and is the sole mutation gateway.
- Copy/cut read a short-lived immutable canonical view, never shaped projection text.
- `SourceProjection` is generation-tagged and refuses a future/stale overwrite.
- Incremental projection validates generation and old delta content; its owned projection string is
  updated locally. Full snapshots are reserved for initialization and explicit resync.
- IME preedit lives in `EditorSession` and `PreeditVisual`; only commit emits a canonical edit.
- Windows-specific clipboard/framebuffer details remain under app adapters; core/render have no
  Win32 imports or unsafe.

## Automated Tests

| Suite | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `stickymd-core` unit | 30 | 0 | 0 |
| core integration/property | 5 | 0 | 0 |
| `stickymd-render` | 15 | 0 | 0 |
| `stickymd-win` ordinary | 20 | 0 | 1 performance baseline |
| Release performance baseline | 1 | 0 | 0 |

Coverage includes CJK, decomposed combining text, emoji and ZWJ clusters, forward/reverse
selection, grapheme backspace/delete, clipboard failures, stale intents, preedit/commit/cancel,
commit-as-one-undo, post-commit ordinary-key preservation, DPI viewport resize, projection mismatch,
internal preedit-caret positioning, trailing-empty-line resync, mixed/wrapped hit-test roundtrips,
scroll preservation, and a fixed-seed editor/projection synchronization sequence.

## Performance Baseline

Command:
`cargo test -p stickymd-win --release --locked phase3_source_pipeline_release_baseline -- --ignored --nocapture`

| Size | End typing p50 / p95 / max | Start typing p95 | Middle IME p95 |
| --- | --- | ---: | ---: |
| 20 KiB | 0.343 / 0.382 / 0.398 ms | 0.499 ms | 0.789 ms |
| 100 KiB | 0.378 / 0.444 / 0.483 ms | 0.585 ms | 0.460 ms |
| 1 MiB | 0.810 / 0.922 / 0.961 ms | 1.184 ms | 1.076 ms |

The 1 MiB worst ordinary insertion position remains 42× below the 50 ms hard gate. Start/middle cases are now
included so the benchmark exercises String movement and later-line offset maintenance rather than
only the cheap append path.

| Size | Backspace p95 | Delete p95 | Selection replace p95 | Undo p95 | Redo p95 | Full resync p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.387 ms | 0.446 ms | 0.401 ms | 1.121 ms | 0.416 ms | 6.815 ms |
| 100 KiB | 0.747 ms | 0.615 ms | 0.487 ms | 1.113 ms | 0.553 ms | 10.983 ms |
| 1 MiB | 1.665 ms | 1.371 ms | 1.399 ms | 2.028 ms | 1.278 ms | 52.710 ms |

Full resync deliberately includes snapshot construction, complete shaping, caret placement and
paint. It is the recovery/newline fallback rather than the ordinary key path; measuring it prevents
that exceptional O(n) work from being hidden by the fast incremental results.

### Corrected Bottleneck

The first full-rebuild implementation measured about 107 ms p95 at 1 MiB, with about 105 ms in
projection reconstruction. This was an implementation defect, not a String model limit. A local
single-line delta update removed that failure. A later audit then removed the remaining two full
document snapshots from every key: effects now carry only generation + delta, and newline/non-local
edits request one explicit canonical snapshot for safe resync.

## Dependency and Unsafe Result

Dependency reasons and licenses are recorded in `phase-03-dependency-delta.md`. No browser engine,
network client, database, Tokio/general async runtime, or GPU UI framework is present.

- `stickymd-core`: `#![forbid(unsafe_code)]`, zero unsafe.
- `stickymd-render`: `#![forbid(unsafe_code)]`, zero unsafe.
- Phase 3 Windows app uses no handwritten unsafe; clipboard is isolated behind `ClipboardPort`.

## Manual Verification Gap

The following cannot be inferred from unit tests and remains open:

- Microsoft Pinyin and WeChat Input Method real composition/commit/cancel.
- Candidate window location at 100%, 150%, and 200% DPI.
- Selection replacement during composition, composition navigation/backspace, refocus, and window
  move/resize behavior.
- Visual CJK/Latin fallback and preedit/caret quality on the USER machine.

Use `phase-03-manual-ime-checklist.md` and record PASS/FAIL/NOT TESTED literally.

## Runtime Idle / Memory Baseline

Five independent Release launches were each idled for 30 seconds, sampled for memory, then observed
for a further 10-second CPU window. Working Set median/max were 31.570/31.672 MiB; private bytes
median/max were 12.027/12.121 MiB; CPU-time delta median/max were 0/31.25 ms per 10 seconds. The
executable was 2.446 MiB. This is a valid dev-shell idle baseline, but it does not replace manual
typing/IME verification or certify future Preview/Split/cache budgets. Full samples and environment
are in `phase-01-performance-baseline.md`.

## Architecture Drift

No unresolved implementation drift remains in the retained automated slice. Persistence and product
window lifecycle remain deliberately absent; no plan contract was changed to accommodate code.

### Cohesion Review

The former >500-line production sections were split before further growth: `app/input.rs` owns input
translation; `app.rs` owns lifecycle/presentation; `source/geometry.rs`, `source/rendering.rs`, and
`source/projection.rs` separately own geometry, painting, and projection updates. No production file
now crosses the ~500-line hard threshold.

## Recommendation

`STOP — manual IME acceptance required before Phase 4.`

This is a verification stop, not evidence that the approved architecture has failed. RichEdit
fallback is not authorized unless the documented two repair attempts and USER review occur.
