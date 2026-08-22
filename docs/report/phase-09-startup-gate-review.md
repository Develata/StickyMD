# Phase 9 Startup Gate Review

## Decision State

Cold startup is closed as **PASS** against the original 300 ms hard gate. The USER-authorized
400 ms fallback is unnecessary. Warm startup remains **FAIL** against 180 ms and prevents an
RC-ready recommendation.

## Final Evidence

- copied Release source identity: exact clean source commit
  `eb687b2441a5816111c116ce30a01bb5b0fba8c6`;
- cold, 20 samples: p50 258.771 ms, p95 277.205 ms, max 404.996 ms;
- warm, 20 samples: p50 325.975 ms, p95 342.891 ms, max 356.433 ms;
- cold internal editor-ready: p50 233.815 ms, p95 249.432 ms;
- warm internal editor-ready: p50 289.376 ms, p95 309.050 ms;
- method: private ready event after first usable present, graceful diagnostic exit, nearest-rank,
  no trimming.

## Remaining Bottleneck

The warm cost is distributed across system-font initialization, source shaping and native-shell
presentation. There is no remaining measured duplicate comparable to the removed second initial
shape pass.

| Candidate | Likely benefit | Cost / risk | Disposition |
| --- | --- | --- | --- |
| Single projection line model and one initial shape | small, also reduces maintenance | low | implemented |
| Start font discovery on another startup thread | uncertain overlap | new synchronization/lifetime protocol and extra stack | rejected without stronger evidence |
| Narrow the system font database | potentially material | breaks CJK/emoji/user-font fallback | rejected |
| Temporary GDI/RichEdit editor | perceived startup gain | second renderer/projection authority and handoff bugs | rejected |
| Signal before usable first present | metric-only gain | violates `EDITOR_READY` contract | rejected |
| Bundle proprietary fonts | avoids discovery uncertainty | license/package violation | rejected |

## Long-Term Decision

The retained cleanup improves cohesion and removes work without adding compatibility debt. Further
warm-start optimization should begin with a profiler-backed source-layout/presentation study, not a
second text authority, a permanent startup thread, or reduced Unicode correctness. Until the warm
gate is met or the USER explicitly changes it, the correct release disposition is `NOT RC READY`.
