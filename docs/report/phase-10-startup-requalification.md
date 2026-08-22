# Phase 10 Startup Requalification

## Decision

Cold startup is **PASS against the USER-approved 400 ms hard gate** and **USER WAIVED against the
original 300 ms gate**. Warm startup remains **FAIL** against the unchanged 180 ms gate. Phase 10
therefore remains `NOT RC READY`.

## Environment

| Field | Value |
| --- | --- |
| Windows | Windows 11 Home Chinese, build 26200 |
| CPU | Intel Core i7-12700H, 20 logical processors |
| RAM | 15.8 GiB |
| GPU | NVIDIA GeForce RTX 3060 Laptop GPU; GameViewer virtual display adapter present |
| Filesystem | NTFS, fixed drive |
| Defender real-time | disabled when measured |
| Toolchain | rustc/cargo 1.97.1, Release, no debugger |
| Measured product commit | `6c372a82f21b12e8c16ba5da606e5810aede05c9` |
| Candidate source commit | `9c0e86298545429e4136c80d861918948be8bb2a` |

The final candidate adds only std-only smoke evidence, documentation and release-script reliability
changes after `6c372a8`; the product source and Release EXE are unchanged. Candidate package
identity is recorded in the RC report.

## Method Audit

Phase 9 already used a copied Release executable, a process-private `EDITOR_READY` event after the
first usable present, graceful diagnostic exit, nearest-rank percentiles and no trimming. The Phase
10 audit found no evidence that the warm failure was a stale event or second-instance path. Phase 10
strengthened the cohort as follows:

- 30 cold plus 30 warm samples rather than 20 plus 20;
- cold and warm are interleaved to reduce long-term machine drift;
- each ready object uses a unique sequence/name;
- every prior process is observed fully exited before the next launch;
- the same bootstrapped portable directory is retained for warm steady-state;
- cold waits 10 seconds and warm waits 250 ms before launch;
- every trace must contain the ordered font/source/presentation/editor-ready milestones;
- all observations are retained.

The benchmark therefore measures external process creation to usable `EDITOR_READY`; it does not
signal at process creation, window construction or a cosmetic blank paper.

## Optimization Ledger

Phase 10 initially re-used winit's `skip_taskbar` request and then also established the required
Win32 Tool Window identity. Source/runtime inspection showed the winit 0.30.13 Windows path invokes
`ITaskbarList::DeleteTab` but does not establish `WS_EX_TOOLWINDOW`. Keeping both paths duplicated
taskbar work without contributing identity. Removing the redundant winit call retained the explicit
style adapter and improved cold p95 from 394.881 ms to 343.220 ms in the diagnostic cohort. The
final clean-product cohort improved further to 321.540 ms.

This is a low-complexity deletion, not an early-ready trick: tray recovery is created first, then the
adapter sets Tool Window style and reads it back after visibility transitions.

## Final Cohort

| Cohort | Samples | p50 | p95 | max | Gate result |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 30 | 293.867 ms | 321.540 ms | 396.086 ms | PASS <=400 ms; original <=300 ms USER WAIVED |
| Warm | 30 | 378.596 ms | 392.263 ms | 394.937 ms | **FAIL** <=180 ms |

The 400 ms relaxation is used. It must never be rewritten as a pass against the original 300 ms
contract. No warm waiver has been granted.

## Warm Failure Analysis

The Phase 9 finding remains valid: full system-font discovery, source shaping and native-shell first
presentation dominate the path. Phase 10 Tool Window and zoom changes did not introduce a new
separate authority, thread pool or eager Preview/RaTeX initialization. The following shortcuts stay
rejected:

- signaling before a usable first present;
- narrowing CJK/emoji/user-font fallback merely to improve the metric;
- adding a temporary RichEdit/GDI editor and later handoff;
- adding a permanent startup thread without measured critical-path overlap;
- bundling proprietary Windows fonts.

Further work requires profiler-backed evidence and a USER decision. Until warm p95 is <=180 ms or
the USER explicitly waives that gate, release disposition remains `NOT RC READY`.
