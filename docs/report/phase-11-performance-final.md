# Phase 11 Final Performance Report

## Scope

本报告记录 Phase 11 / 11-B 最终实现的 Release 性能。所有结果来自同一 Windows 11 x64
开发机；机器相关 startup/resource 数据以 checked-in JSON receipt 为准，不外推为跨机器承诺。

## Preview Pipeline

| Workload | median | p95 | max | Hard gate | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| 20 KiB native Preview | 7.817 ms | 9.339 ms | 225.162 ms | 100 ms p95 | PASS |
| 100 KiB native Preview | 20.584 ms | 21.949 ms | 180.043 ms | 400 ms p95 | PASS |
| 1 MiB native Preview | 153.957 ms | 208.042 ms | 332.852 ms | 2 s p95 | PASS |
| 1 MiB / 500 formulas | 146.531 ms | 190.651 ms | 195.145 ms | 2 s p95 | PASS |
| 1 MiB / 1000 delimiter conversions | 25.218 ms | 29.030 ms | 31.758 ms | 50 ms p95 | PASS |

### Before / After

The Phase 5 1 MiB baseline failed at 2.706 s p95 before the final shaping work. The retained implementation
uses three bounded rules:

1. equal width/scale/theme relayouts repaint without rebuilding geometry;
2. large mixed inline runs are split into width-derived batches capped at 48 characters;
3. one document layout reuses identical shaping only after a second occurrence, with at most 1024 cached
   entries and at most 1024 text bytes per key; the index is dropped at the end of that layout.

The final 1 MiB p95 is 208.042 ms, a 92.3% reduction from the 2.706 s failing cohort. Source ranges,
selection ranges and link actions are reprojected for every block and are deliberately excluded from the
shape key; a regression test proves that reused geometry cannot reuse another block's action/source fact.

## Equal-Geometry Zoom

| Zoom | median | p95 | max | Gate |
| --- | ---: | ---: | ---: | --- |
| 50% | 1.756 ms | 2.479 ms | 2.851 ms | PASS <=50 ms |
| 100% | 1.801 ms | 2.365 ms | 3.196 ms | PASS <=50 ms |
| 300% | 1.874 ms | 2.347 ms | 2.524 ms | PASS <=50 ms |

The pre-fast-path 50% cohort repeatedly measured 61–67 ms p95. The fast path is not a benchmark-only
branch: it applies to any repeated relayout request with identical generation, width, normalized scale,
theme and image-source availability. Image-source availability changes and viewport image-band misses still
force a full layout.

## Startup

The final executable measured cold p95 300.692 ms and warm p95 311.353 ms. Cold passes the
USER-approved 400 ms gate; warm still fails the authoritative 180 ms gate. All 30 cold and 50 warm raw
samples, including schema-v2 milestones, are in `docs/report/evidence/phase-11-performance-final.json`.

## Resource Matrix

The complete resource matrix passed. Representative maxima are:

| Scenario | Private working set max | Private bytes max | Idle CPU max |
| --- | ---: | ---: | ---: |
| Source | 13.06 MiB | 15.02 MiB | 0.004% |
| Preview / 20 math | 15.40 MiB | 16.62 MiB | 0.000% |
| Split / 20 math | 21.72 MiB | 24.27 MiB | 0.001% |
| Preview / saturated image cache | 25.22 MiB | 27.19 MiB | 0.000% |
| Split / saturated image cache | 31.77 MiB | 35.04 MiB | 0.004% |
| Hidden to tray | 12.43 MiB | 16.13 MiB | 0.003% |

The 4K-image Preview peaked at 16.24 MiB working set. One hundred 100%-zoom in/out cycles changed
private bytes by -454,656 bytes, so no linear-growth signal was observed. The raw five-process samples
are in `docs/report/evidence/phase-11-resources-final.json`.

The performance/resource receipts were collected at implementation commit `9b6952767105` and bind to
EXE SHA-256 `6dbc31fb...f7c5d`. The final candidate commit `23d2a410a256` changes only release scripts;
its exact release receipt records the same EXE SHA-256, so no product binary changed between cohorts.

## Complexity and Memory Review

- no new runtime dependency, worker, renderer or authority;
- shaping reuse is document-scoped and bounded, not a persistent cache;
- unique text is only admitted after a second occurrence, avoiding a duplicate `Buffer` for one-off text;
- keys larger than 1024 bytes are not cached;
- the cache cardinality is capped at 1024 and dropped before the next document layout;
- image and formula caches retain their independent byte budgets.

## Result

Preview/input/resource gates pass. Startup warm and current-candidate manual acceptance remain release
blockers independent of these improvements.
