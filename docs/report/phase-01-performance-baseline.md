# Phase 1 Performance Baseline — Rebuilt

- `Date`: 2026-08-20
- `Environment`: Windows 11 build 26200; i7-12700H (20 logical processors); 15.8 GiB RAM;
  Rust 1.97.1 MSVC
- `Status`: Local engineering evidence, not a public claim

## Current Source Development Shell

Release build, five independent launches. Each launch idled 30 seconds before memory sampling, then
CPU time was sampled over another 10 seconds. The exact process started by the measurement was stopped
after each run.

| Run | Working Set MiB | Private MiB | CPU delta over 10 s |
| ---: | ---: | ---: | ---: |
| 1 | 31.670 | 12.030 | 0 ms |
| 2 | 31.470 | 11.980 | 0 ms |
| 3 | 31.630 | 11.960 | 0 ms |
| 4 | 31.480 | 12.060 | 31.25 ms |
| 5 | 31.570 | 12.120 | 31.25 ms |
| median | **31.570** | **12.027** | **0 ms** |
| max | **31.672** | **12.121** | **31.25 ms** |

The maximum 31.25 ms over 10 seconds is about 0.0156% of total 20-logical-processor capacity. The
current executable is 2.446 MiB. This validates no obvious permanent redraw loop in the current dev
shell; it does not validate product Preview/Split/cache budgets.

## Comrak + Owned Projection

20 Release samples after three warm-ups:

| Size | median | p95 | max |
| --- | ---: | ---: | ---: |
| 20 KiB | 3.665 ms | 3.944 ms | 4.147 ms |
| 100 KiB | 16.980 ms | 17.727 ms | 17.744 ms |
| 1 MiB | 175.445 ms | 184.886 ms | 184.977 ms |

The 1 MiB result supports the approved background full-parse + 1000 ms debounce + generation stale-drop
model. It is not a reason to introduce an incremental Markdown parser.

## RaTeX Spike-only PNG Pipeline

Representative formula parse/layout/display-list/PNG, 20 Release samples:

| median | p95 | max | PNG bytes |
| ---: | ---: | ---: | ---: |
| 0.558 ms | 0.920 ms | 1.824 ms | 11,164 |

PNG encoding is only technical evidence and is forbidden as the final production hot path.

## Unmeasured

- cold/hot startup p95;
- true Private Working Set/Commit Size via dedicated Windows counters (the table uses process working
  set and private bytes exposed by PowerShell);
- real IME memory/latency under Microsoft Pinyin and WeChat IME;
- 100/150/200% DPI, multi-monitor, product Preview/Split, images and hidden-cache purge.
