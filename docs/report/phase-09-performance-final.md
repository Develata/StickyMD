# Phase 9 Final Performance and Memory Report

## Result

All measured memory, idle-CPU, input, Preview, persistence, math, asset and package-size hard gates
pass on the Phase 9 Release code. Cold startup passes its original 300 ms gate. Warm startup fails
its unchanged 180 ms gate and remains the sole automated performance release blocker.

## Environment

| Field | Value |
| --- | --- |
| Source commit | `eb687b2441a5816111c116ce30a01bb5b0fba8c6` |
| Windows | Windows 11 Home, build 26200 |
| CPU | Intel Core i7-12700H, 14 cores / 20 logical processors |
| RAM | 16,962,281,472 bytes reported |
| GPU | NVIDIA GeForce RTX 3060 Laptop GPU |
| Filesystem | NTFS on fixed NVMe SSD |
| Defender real-time | disabled when measured |
| Toolchain | rustc/cargo 1.97.1, `x86_64-pc-windows-msvc`, LLVM 22.1.6 |
| Debugger | none |

The common source seed is 393 bytes, SHA-256
`24005dd7f13977c075dcd929095001eb8d358a982cd61c8750e0c720da6fcb2d`. Benchmarks repeat that
seed deterministically to their reported size. Runtime source/preview/split fixtures are 20 KiB;
Preview includes 20 formulas. Percentiles are nearest-rank and no samples are trimmed.

## Startup

| Cohort | Samples | p50 | p95 | max | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 20 | 258.771 ms | 277.205 ms | 404.996 ms | PASS <=300 ms |
| Warm | 20 | 325.975 ms | 342.891 ms | 356.433 ms | **FAIL** <=180 ms |

The 400 ms cold fallback authorized by the USER was not used. Full method and milestones are in
`phase-09-startup-hardening.md`.

## Runtime Memory and Idle CPU

Five copied-Release runs were used. Memory is recorded after warm-up and stable presentation. Each
mode has five independent 60-second CPU samples, each split into six 10-second diagnostic buckets;
the table reports nearest-rank p95. The harness parks the physical cursor inside the work area but
outside StickyMD and records window geometry in every bucket, avoiding taskbar/edge-sensor or paper
interaction being mislabeled as process idle. Private working set is the release-gate metric;
private bytes and process peak working set are retained as leak/transient diagnostics.

| Mode | PWS median | PWS max | Private max | Peak WS max | Idle CPU p95 | Hard gate |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Source | 7.754 MiB | 7.785 MiB | 8.586 MiB | 26.773 MiB | 0.002604% | PASS <=40 MiB / <=0.1% |
| Preview | 18.160 MiB | 18.215 MiB | 19.758 MiB | 40.051 MiB | 0.001302% | PASS <=52 MiB / <=0.1% |
| Split | 19.531 MiB | 19.578 MiB | 21.133 MiB | 42.484 MiB | 0.005208% | PASS <=64 MiB / <=0.1% |
| Hidden to tray | 7.137 MiB | 7.195 MiB | 8.562 MiB | 34.246 MiB | 0.002604% | PASS <=36 MiB / <=0.1% |

## Source Editing

The table shows total insertion latency; the accompanying stress row records the slowest editor
operation p95 measured at each size. All operations include canonical mutation, projection update,
caret mapping and paint.

| Size | End typing p50 / p95 / max | Middle IME p95 | Slowest operation p95 | Hard gate |
| --- | --- | ---: | --- | --- |
| 20 KiB | 0.196 / 0.490 / 0.657 ms | 0.454 ms | backspace 2.291 ms | PASS <=16 ms |
| 100 KiB | 0.241 / 0.317 / 0.355 ms | 1.035 ms | full resync 3.942 ms | PASS <=25 ms |
| 1 MiB | 0.825 / 0.996 / 1.050 ms | 1.407 ms | full resync 36.775 ms | PASS <=50 ms |

## Core Text Model

| Size | Middle insert p95 | Middle delete p95 | Snapshot p95 | Undo p95 | Redo p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.7 us | 0.6 us | 0.5 us | 0.4 us | 0.4 us |
| 100 KiB | 1.3 us | 1.2 us | 2.1 us | 1.3 us | 1.2 us |
| 1 MiB | 12.0 us | 10.8 us | 347.6 us | 11.0 us | 10.1 us |

The String-backed model is not disproved at the v1 support limit. A rope would add complexity and
another migration surface without a measured user benefit.

## Persistence

Values are median / p95 / max; total includes snapshot, line-ending encoding, SHA-256, same-dir
temporary write + flush, and atomic replace.

| Size | Snapshot | Encode | Hash | Write + flush | Replace | End-to-end |
| --- | --- | --- | --- | --- | --- | --- |
| 20 KiB | 14 / 17 / 18 us | 24 / 33 / 41 us | 10 / 11 / 12 us | 2.995 / 3.348 / 4.206 ms | 1.731 / 2.471 / 3.715 ms | 4.814 / 5.555 / 7.980 ms |
| 100 KiB | 30 / 34 / 36 us | 111 / 127 / 128 us | 49 / 67 / 68 us | 3.054 / 3.350 / 3.972 ms | 1.596 / 2.097 / 2.271 ms | 4.925 / 5.530 / 5.953 ms |
| 1 MiB | 227 / 315 / 361 us | 1071 / 1493 / 1627 us | 514 / 628 / 643 us | 3.784 / 4.230 / 6.766 ms | 1.693 / 2.264 / 2.275 ms | 7.552 / 8.091 / 11.092 ms |

## Native Preview and Math

| Pipeline | Size | Total median | p95 | max | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Markdown Preview | 20 KiB | 34.852 ms | 36.408 ms | 134.650 ms | PASS p95 <=100 ms |
| Markdown Preview | 100 KiB | 162.917 ms | 174.067 ms | 251.797 ms | PASS p95 <=400 ms |
| Markdown Preview | 1 MiB | 1.655 s | 1.744 s | 1.762 s | PASS p95 <=2 s, background |
| Math document / 20 formulas | 20 KiB | 10.229 ms | 11.915 ms | 12.762 ms | PASS |
| Math document / 100 formulas | 100 KiB | 44.657 ms | 49.793 ms | 50.784 ms | PASS |
| Math document / 500 formulas | 1 MiB | 420.250 ms | 461.034 ms | 465.953 ms | PASS, background |

The cold first Preview build is higher because it initializes fonts (20 KiB 212.070 ms; 1 MiB
1.969 s). User edits remain responsive while math builds: 100 canonical edits had 2.0 us p95 and
274.9 us max.

## Assets, Images and Export

| Operation | Median | p95 | max |
| --- | ---: | ---: | ---: |
| Managed scan, 1 MiB full | 418 us | 557 us | 558 us |
| Managed scan, incremental | 1 us | 5 us | 6 us |
| Decode/resize 1024x768 to 800x600 | 25.365 ms | 35.520 ms | 35.577 ms |
| Decoded-image cache hit | 3 us | 4 us | 11 us |
| Export 20 references / 1 MiB | 14.687 ms | 21.338 ms | 35.375 ms |
| Paste ten PNG files, end-to-end | 82.434 ms | 100.631 ms | 111.156 ms |

The managed scanner uses a one-pass byte search and incremental per-delta updates. There is no
reason to replace it with a full Markdown parse on every edit.

## 4K Image Transient Peak

Five copied-Release runs of a 3840x2160 BMP after the owned-decode lifetime change:

| Metric | Phase 7 max | Phase 9 max | Delta |
| --- | ---: | ---: | ---: |
| Peak working set | 93.93 MiB | 83.438 MiB | -10.49 MiB |
| Peak private bytes | 79.93 MiB | 65.293 MiB | -14.64 MiB |
| Stable working set | 16.78 MiB | 19.316 MiB | +2.54 MiB |
| Stable private bytes | 17.84 MiB | 20.863 MiB | +3.02 MiB |

The optimization drops encoded bytes before scaling, removing a large overlap without unsafe code,
a custom codec, a GPU pipeline or another cache. The transient is bounded and the decoded cache
remains budgeted; the higher stable observation is recorded rather than hidden.

## Window Algorithms and Leak Stress

| Operation | Median | p95 | max |
| --- | ---: | ---: | ---: |
| Control layout + hit-test, 100k | 828.4 us | 899.0 us | 973.9 us |
| Window reducer, 100k | 2.1712 ms | 2.7372 ms | 3.1757 ms |
| Geometry solver, 100k | 74.9 us | 95.2 us | 194.5 us |

After 1000 dock/reveal/hide animation cycles plus 100 autosave/external reloads, 100 dirty-conflict
resolutions and 100 image-decode/cache-release cycles, private bytes changed from 9,773,056 to
10,326,016 bytes (+0.527 MiB), handles from 320 to 327, GDI objects from 16 to 16 and USER objects
from 21 to 24. Intermediate checkpoints did not grow monotonically. The active animation stress
used one core heavily by design; every post-animation idle mode returned below the 0.1% gate.

## Binary Size

- `StickyMD.exe`: 8,287,744 bytes (7.904 MiB), SHA-256
  `84057a4322c965dbf48646274f2686464f060059a70aeebe1e72264d260c7831`, PASS <=20 MiB.
- portable ZIP: 3,878,842 bytes (3.699 MiB), PASS <=30 MiB.

## Decision

The implemented algorithms are simple, bounded and measured: deadline coalescing rather than
per-key snapshots, one I/O worker with one in-flight plus one latest pending save, viewport-only
paint, bounded caches/history, one-pass asset scanning and event-driven idle. No complex algorithmic
rewrite is justified by the current data. Warm startup is the only automated performance gate that
does not pass.
