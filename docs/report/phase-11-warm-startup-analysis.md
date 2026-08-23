# Phase 11 Warm Startup Analysis

## Measurement Contract

- candidate: copied standalone Release executable;
- fixture: 20 KiB Source note in an isolated portable directory;
- ordering: 30 cold samples, each followed by one warm sample, then 20 additional warm samples;
- cold idle: 10 s; warm idle: 250 ms;
- percentile: Rust CLI nearest-rank, no trimming;
- readiness: first usable Source frame has successfully presented, shell guards are installed and IME
  has been enabled; the event was not moved earlier;
- each run uses a PID/nonce-scoped ready object, waits for the prior process to exit and records the
  complete startup milestone trace.

Raw schema-v2 evidence is `docs/report/evidence/phase-11-performance-final.json`.

## Distribution

| Cohort | samples | p50 | p90 | p95 | p99/max | mean | stddev |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| cold | 30 | 283.028 ms | 291.113 ms | 300.692 ms | 436.771 ms | 288.322 ms | 28.247 ms |
| warm | 50 | 291.573 ms | 298.744 ms | 311.353 ms | 349.150 ms | 292.331 ms | 10.921 ms |

本 cohort 中 warm 不再慢于 cold。Phase 10 的 warm p95 392.263 ms 异常没有在本次 50 样本
cohort 重现；因此不能继续把 “warm > cold” 当成当前产品事实。冷启动的长尾主要由 OS 进程
创建/调度与字体/文本布局共同贡献，不能只归因于 “Windows 波动”。

## Startup Cost Breakdown

区间为每个样本内相邻 milestone 的差值；p95 分别对区间样本做 nearest-rank。

| Component | cold p50/p95 | warm p50/p95 | Interpretation |
| --- | ---: | ---: | --- |
| process creation outside Rust trace | 29.264 / 37.183 ms | 29.404 / 42.515 ms | bounded OS/process contributor |
| path/persistence/config/document bootstrap | 7.391 / 8.204 ms | 7.513 / 8.457 ms | not dominant |
| event loop + window + surface | 18.909 / 22.155 ms | 19.306 / 27.094 ms | bounded native setup |
| FontSystem + font selection | 52.684 / 56.452 ms | 53.652 / 58.356 ms | dominant stable dependency cost |
| canonical text to source buffers | 21.954 / 23.295 ms | 22.472 / 25.060 ms | O(document lines/scripts) |
| shaping to source projection | 72.721 / 78.702 ms | 74.144 / 80.447 ms | largest product-controlled interval |
| monitor/tray/show/opacity/topmost | 25.840 / 28.630 ms | 28.557 / 32.136 ms | required recovery/shell path |
| native focus + guards | 47.322 / 50.716 ms | 48.044 / 51.195 ms | synchronous Windows focus cost |
| first present | 6.359 / 6.784 ms | 5.652 / 6.538 ms | not dominant |
| internal total | 254.595 / 265.313 ms | 262.821 / 269.504 ms | first usable frame semantics retained |

Font initialization plus buffer construction and shaping is the principal product-controlled cost. It is
not more than 80% of total startup, so no such claim is made. The external cold long tail is independently
visible rather than folded into font cost.

## Architecture-safe Optimization Ledger

| Change | Before | After | Complexity | Decision |
| --- | --- | --- | --- | --- |
| construct initial Source projection at the real pane geometry; make equal `set_viewport` a no-op | Phase 10 warm p95 392.263 ms and a provable second equal reshape path | Phase 11 warm p95 311.353 ms; unit proves equal viewport does no work | one equality guard, no new state | keep |

The startup before/after rows are sequential machine cohorts, not a controlled binary A/B, so the whole
52.049 ms difference is not attributed to the viewport guard. The retained implementation is justified
independently by removing a deterministic duplicate reshape and by its regression test.

Preview shaping and equal-geometry relayout optimizations are reported separately in
`docs/report/phase-11-performance-final.md`; they do not change Source `EDITOR_READY` semantics.

## Dominant Cost and Stop Rule

The remaining simple hot path is cosmic-text/font-system construction plus initial 20 KiB source shaping.
Further attempts likely require persistent font indexes, a second startup renderer, parallel font authority,
or presenting before a correct input-ready frame. Those options violate the approved complexity/authority
budget. No persistent cache, service, thread pool, second renderer or benchmark-only product path was added.

## Result

- cold p95: **PASS** 300.692 ms <= USER-approved 400 ms;
- warm p95: **FAIL** 311.353 ms > 180 ms;
- methodology and milestone evidence: PASS;
- further simple/high-leverage work: none identified without disproportionate architecture cost.

Proceed to explicit warm-gate reassessment; the Agent does not change that gate.
