# Phase 9 Final Performance and Memory Report

## Result

All measured memory, idle-CPU, input, Preview, persistence, math and asset hard gates pass on the
Phase 9 Release code. Exact replacement package size remains pending package regeneration. Cold
startup passes its original 300 ms gate. Warm startup fails its unchanged 180 ms gate and remains
the sole automated performance release blocker.

## Environment

| Field | Value |
| --- | --- |
| Source identity | measured Phase 9 convergence tree; exact clean package commit is pending |
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
| Cold | 20 | 252.337 ms | 268.595 ms | 374.945 ms | PASS <=300 ms |
| Warm | 20 | 254.754 ms | 267.094 ms | 272.364 ms | **FAIL** <=180 ms |

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
| Source | 7.754 MiB | 7.758 MiB | 8.551 MiB | 26.742 MiB | 0.002604% | PASS <=40 MiB / <=0.1% |
| Preview | 18.199 MiB | 18.238 MiB | 19.656 MiB | 39.977 MiB | 0.001302% | PASS <=52 MiB / <=0.1% |
| Split | 19.445 MiB | 19.504 MiB | 21.027 MiB | 42.422 MiB | 0.002604% | PASS <=64 MiB / <=0.1% |
| Hidden to tray | 7.207 MiB | 7.578 MiB | 8.656 MiB | 34.543 MiB | 0.001302% | PASS <=36 MiB / <=0.1% |

## Source Editing

The table shows total insertion latency; the accompanying stress row records the slowest editor
operation p95 measured at each size. All operations include canonical mutation, projection update,
caret mapping and paint.

| Size | End typing p50 / p95 / max | Middle IME p95 | Slowest operation p95 | Hard gate |
| --- | --- | ---: | --- | --- |
| 20 KiB | 0.193 / 0.263 / 0.388 ms | 0.262 ms | backspace 2.095 ms | PASS <=16 ms |
| 100 KiB | 0.256 / 0.394 / 0.425 ms | 0.805 ms | full resync 4.057 ms | PASS <=25 ms |
| 1 MiB | 0.813 / 1.011 / 1.017 ms | 1.830 ms | full resync 37.446 ms | PASS <=50 ms |

## Core Text Model

| Size | Middle insert p95 | Middle delete p95 | Snapshot p95 | Undo p95 | Redo p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 20 KiB | 0.6 us | 0.5 us | 0.4 us | 0.4 us | 0.4 us |
| 100 KiB | 1.3 us | 1.1 us | 1.9 us | 1.1 us | 1.1 us |
| 1 MiB | 10.8 us | 12.2 us | 322.8 us | 8.9 us | 8.5 us |

The String-backed model is not disproved at the v1 support limit. A rope would add complexity and
another migration surface without a measured user benefit.

## Persistence

Values are median / p95 / max; total includes snapshot, line-ending encoding, SHA-256, same-dir
temporary write + flush, and atomic replace.

| Size | Snapshot | Encode | Hash | Write + flush | Replace | End-to-end |
| --- | --- | --- | --- | --- | --- | --- |
| 20 KiB | 11 / 28 / 29 us | 22 / 26 / 27 us | 10 / 11 / 11 us | 3.046 / 3.368 / 4.351 ms | 1.634 / 2.336 / 3.466 ms | 4.863 / 5.411 / 7.861 ms |
| 100 KiB | 40 / 52 / 142 us | 99 / 126 / 265 us | 49 / 56 / 78 us | 3.086 / 3.756 / 12.960 ms | 1.638 / 1.994 / 2.042 ms | 4.952 / 5.411 / 15.011 ms |
| 1 MiB | 228 / 264 / 268 us | 944 / 1181 / 1445 us | 515 / 682 / 886 us | 3.845 / 5.797 / 6.605 ms | 1.781 / 2.642 / 3.138 ms | 7.453 / 9.373 / 10.311 ms |

## Native Preview and Math

| Pipeline | Size | Total median | p95 | max | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Markdown Preview | 20 KiB | 34.740 ms | 37.242 ms | 139.184 ms | PASS p95 <=100 ms |
| Markdown Preview | 100 KiB | 161.160 ms | 263.444 ms | 276.812 ms | PASS p95 <=400 ms |
| Markdown Preview | 1 MiB | 1.638 s | 1.785 s | 1.932 s | PASS p95 <=2 s, background |
| Math document / 20 formulas | 20 KiB | 9.415 ms | 10.478 ms | 10.679 ms | PASS |
| Math document / 100 formulas | 100 KiB | 44.515 ms | 48.739 ms | 48.949 ms | PASS |
| Math document / 500 formulas | 1 MiB | 427.336 ms | 464.251 ms | 470.793 ms | PASS, background |

The cold first Preview build is higher because it initializes fonts (20 KiB 206.587 ms; 1 MiB
1.924 s). User edits remain responsive while math builds: 100 canonical edits had 1.6 us p95 and
329.1 us max.

## Assets, Images and Export

| Operation | Median | p95 | max |
| --- | ---: | ---: | ---: |
| Managed scan, 1 MiB full | 457 us | 570 us | 611 us |
| Managed scan, incremental | 1 us | 3 us | 8 us |
| Decode/resize 1024x768 to 800x600 | 28.011 ms | 37.178 ms | 38.028 ms |
| Decoded-image cache hit | 3 us | 13 us | 18 us |
| Export 20 references / 1 MiB | 15.028 ms | 19.421 ms | 19.446 ms |
| Paste ten PNG files, end-to-end | 91.835 ms | 121.217 ms | 141.600 ms |

The managed scanner uses a one-pass byte search and incremental per-delta updates. There is no
reason to replace it with a full Markdown parse on every edit.

## 4K Image Transient Peak

Five copied-Release runs of a 3840x2160 BMP after the owned-decode lifetime change:

| Metric | Phase 7 max | Phase 9 max | Delta |
| --- | ---: | ---: | ---: |
| Peak working set | 93.93 MiB | 83.46 MiB | -10.47 MiB |
| Peak private bytes | 79.93 MiB | 65.33 MiB | -14.60 MiB |
| Stable working set | 16.78 MiB | 19.35 MiB | +2.57 MiB |
| Stable private bytes | 17.84 MiB | 20.92 MiB | +3.08 MiB |

The optimization drops encoded bytes before scaling, removing a large overlap without unsafe code,
a custom codec, a GPU pipeline or another cache. The transient is bounded and the decoded cache
remains budgeted; the higher stable observation is recorded rather than hidden.

## Window Algorithms and Leak Stress

| Operation | Median | p95 | max |
| --- | ---: | ---: | ---: |
| Control layout + hit-test, 100k | 924.2 us | 1.1045 ms | 1.3273 ms |
| Window reducer, 100k | 2.3885 ms | 2.9474 ms | 3.3585 ms |
| Geometry solver, 100k | 85.9 us | 105.7 us | 146.8 us |

After 1000 dock/reveal/hide animation cycles plus 100 autosave/external reloads, 100 dirty-conflict
resolutions and 100 image-decode/cache-release cycles, private bytes changed from 9,756,672 to
10,428,416 bytes (+0.641 MiB), handles from 325 to 332, GDI objects from 16 to 16 and USER objects
from 21 to 24. Intermediate checkpoints did not grow monotonically. The active animation stress
used one core heavily by design; every post-animation idle mode returned below the 0.1% gate.

## Binary Size

- Exact clean-source EXE and portable ZIP sizes are pending package regeneration after the review
  fixes. The measured working-tree executable remains below the 20 MiB / 30 MiB gates.

## Decision

The implemented algorithms are simple, bounded and measured: deadline coalescing rather than
per-key snapshots, one I/O worker with one in-flight plus one latest pending save, viewport-only
paint, bounded caches/history, one-pass asset scanning and event-driven idle. No complex algorithmic
rewrite is justified by the current data. Warm startup is the only automated performance gate that
does not pass.
