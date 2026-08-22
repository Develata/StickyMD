# Phase 11 Final Performance Report

## Scope

本报告记录 Phase 11 / 11-B 最终实现的 Release 性能。所有结果来自同一 Windows 11 x64
开发机；机器相关 startup/resource 数据以 checked-in JSON receipt 为准，不外推为跨机器承诺。

## Preview Pipeline

| Workload | median | p95 | max | Hard gate | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| 20 KiB native Preview | 6.451 ms | 7.399 ms | 109.415 ms | 100 ms p95 | PASS |
| 100 KiB native Preview | 19.284 ms | 21.558 ms | 119.616 ms | 400 ms p95 | PASS |
| 1 MiB native Preview | 234.258 ms | 253.253 ms | 266.010 ms | 2 s p95 | PASS |
| 1 MiB / 500 formulas | 237.516 ms | 242.320 ms | 246.823 ms | 2 s p95 | PASS |
| 1 MiB / 1000 delimiter conversions | 19.745 ms | 21.904 ms | 22.108 ms | 50 ms p95 | PASS |

### Before / After

The Phase 5 1 MiB baseline failed at 2.706 s p95 before the final shaping work. The retained implementation
uses three bounded rules:

1. equal width/scale/theme relayouts repaint without rebuilding geometry;
2. large mixed inline runs are split into width-derived batches capped at 48 characters;
3. one document layout reuses identical shaping only after a second occurrence, with at most 1024 cached
   entries and at most 1024 text bytes per key; the index is dropped at the end of that layout.

The final 1 MiB p95 is 253.253 ms, a 90.6% reduction from the 2.706 s failing cohort. Source ranges,
selection ranges and link actions are reprojected for every block and are deliberately excluded from the
shape key; a regression test proves that reused geometry cannot reuse another block's action/source fact.

## Equal-Geometry Zoom

| Zoom | median | p95 | max | Gate |
| --- | ---: | ---: | ---: | --- |
| 50% | 1.358 ms | 1.596 ms | 1.691 ms | PASS <=50 ms |
| 100% | 1.326 ms | 1.446 ms | 1.581 ms | PASS <=50 ms |
| 300% | 1.451 ms | 1.600 ms | 1.681 ms | PASS <=50 ms |

The pre-fast-path 50% cohort repeatedly measured 61–67 ms p95. The fast path is not a benchmark-only
branch: it applies to any repeated relayout request with identical generation, width, normalized scale,
theme and image-source availability. Image-source availability changes and viewport image-band misses still
force a full layout.

## Startup

The pre-commit candidate measured cold p95 306.098 ms and warm p95 307.142 ms. Cold passes the
USER-approved 400 ms gate; warm still fails the authoritative 180 ms gate. Exact committed-candidate
cohort evidence will replace `docs/report/evidence/phase-11-startup-final.json` before final handoff.

## Resource Matrix

Exact committed-candidate memory, idle CPU, 4K image peak, repeated lifecycle and zoom-resource evidence
is pending the final `phase-11.ps1 -Resources` run. No resource row is inferred from unit or timing tests.

## Complexity and Memory Review

- no new runtime dependency, worker, renderer or authority;
- shaping reuse is document-scoped and bounded, not a persistent cache;
- unique text is only admitted after a second occurrence, avoiding a duplicate `Buffer` for one-off text;
- keys larger than 1024 bytes are not cached;
- the cache cardinality is capped at 1024 and dropped before the next document layout;
- image and formula caches retain their independent byte budgets.

## Result

Headless performance gates pass. Startup warm and current-candidate manual acceptance remain release
blockers independent of these improvements.
