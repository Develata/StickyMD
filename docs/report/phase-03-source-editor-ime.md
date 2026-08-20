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
| 20 KiB | 0.340 / 0.362 / 0.368 ms | 0.822 ms | 0.738 ms |
| 100 KiB | 0.386 / 0.504 / 0.671 ms | 0.621 ms | 0.443 ms |
| 1 MiB | 0.666 / 0.795 / 0.888 ms | 1.123 ms | 1.118 ms |

The 1 MiB worst ordinary insertion position remains 44× below the 50 ms hard gate. Start/middle cases are now
included so the benchmark exercises String movement and later-line offset maintenance rather than
only the cheap append path.

| Size | Backspace p95 | Delete p95 | Selection replace p95 | Newline p95 | Undo p95 | Redo p95 | Full resync p95 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.410 ms | 0.464 ms | 0.421 ms | 0.550 ms | 1.068 ms | 0.403 ms | 7.066 ms |
| 100 KiB | 0.606 ms | 0.556 ms | 0.442 ms | 0.569 ms | 1.112 ms | 0.567 ms | 11.108 ms |
| 1 MiB | 1.235 ms | 1.065 ms | 1.192 ms | 1.534 ms | 1.567 ms | 0.939 ms | 53.227 ms |

Full resync deliberately includes snapshot construction, complete shaping, caret placement and
paint. It is a recovery fallback rather than the ordinary key or newline path; measuring it prevents
that exceptional O(n) work from being hidden by the fast incremental results.

### Corrected Bottleneck

The first full-rebuild implementation measured about 107 ms p95 at 1 MiB, with about 105 ms in
projection reconstruction. This was an implementation defect, not a String model limit. A local
affected-line delta update removed that failure. A later audit then removed the remaining two full
document snapshots from every key. Effects now carry only generation + delta; line splits and
merges rebuild only affected lines, and only true desynchronization requests a canonical snapshot.

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
