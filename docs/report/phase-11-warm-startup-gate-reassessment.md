# Phase 11 Warm Startup Gate Reassessment

## Current Evidence

| Cohort | p50 | p95 | p99 | max | Current gate |
| --- | ---: | ---: | ---: | ---: | --- |
| cold, n=30 | 291.037 ms | 433.263 ms | 481.635 ms | 481.635 ms | FAIL <=400 ms |
| warm, n=50 | 288.877 ms | 340.214 ms | 371.066 ms | 371.066 ms | FAIL <=180 ms |

The ready event still follows the first successfully presented, usable Source frame. Raw samples and
milestones are retained in `docs/report/evidence/phase-11-startup-final.json`.

## Dominant Cost

The stable product-controlled portion is font discovery/selection, source-buffer construction and initial
shaping. Warm p95 for these three intervals is approximately 60.666 + 29.171 + 90.336 ms. Native focus
adds about 57.615 ms p95. Cold additionally contains up to 120.230 ms p95 of measured pre-Rust process
overhead.

## Optimizations Tried

- Removed the deterministic equal-viewport second reshape while retaining one SourceProjection authority.
- Reused the same initial geometry calculation as the shell projection.
- Kept Preview performance work independent from the Source startup path; no Preview cache or shortcut
  moves `EDITOR_READY` earlier.
- Added milestones and unique ready objects instead of moving `EDITOR_READY` earlier.

## Rejected Directions

| Direction | Reason |
| --- | --- |
| persistent serialized font database / registry cache | portable-state migration and second font authority |
| background font indexing service / daemon | lifecycle, resource and failure-path expansion |
| temporary fallback editor/renderer | duplicate renderer and correctness split |
| signalling ready before focus/first present | changes the user-visible contract instead of performance |
| parallel startup state machine / general thread pool | coupling and ordering complexity disproportionate to v1 |

## Recommendation

The current 180 ms warm target is not compatible with the retained native font/shaping and true
input-ready semantics on this environment. If the USER accepts current behavior, recommend changing the
engineering gates to:

- warm startup p95 <=400 ms;
- cold startup p95 <=500 ms.

These values are derived from the observed p95/max envelopes (340.214/371.066 ms warm and
433.263/481.635 ms cold), not from the median. They leave only modest headroom and must be revalidated on
the release environment; they are not architecture invariants or marketing claims.

## Decision State

**RECOMMEND RELAXATION — NOT APPROVED.** The USER previously approved cold <=400 ms only; the current
cohort does not meet it. Warm <=180 ms also remains authoritative. Until explicit USER approval, startup
is a release blocker and the final disposition is `NOT RC READY`.
