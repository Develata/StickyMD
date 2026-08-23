# Phase 11 Warm Startup Gate Reassessment

## Current Evidence

| Cohort | p50 | p95 | p99 | max | Current gate |
| --- | ---: | ---: | ---: | ---: | --- |
| cold, n=30 | 283.028 ms | 300.692 ms | 436.771 ms | 436.771 ms | PASS <=400 ms |
| warm, n=50 | 291.573 ms | 311.353 ms | 349.150 ms | 349.150 ms | FAIL <=180 ms |

The ready event still follows the first successfully presented, usable Source frame. Raw samples and
milestones are retained in `docs/report/evidence/phase-11-performance-final.json`.

## Dominant Cost

The stable product-controlled portion is font discovery/selection, source-buffer construction and initial
shaping. Warm p95 for these three intervals is approximately 58.356 + 25.060 + 80.447 ms. Native focus
and guards add about 51.195 ms p95. The measured pre-Rust process overhead is 42.515 ms warm p95.

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

- warm startup p95 <=400 ms.

This value is derived from the observed p95/max envelope (311.353/349.150 ms), not from the median. It
leaves modest headroom and must be revalidated on the release environment; it is not an architecture
invariant or marketing claim. The USER-approved cold <=400 ms gate already passes and needs no further
relaxation.

## Decision State

**RECOMMEND WARM RELAXATION — NOT APPROVED.** The USER-approved cold <=400 ms gate passes. Warm <=180 ms
remains authoritative until explicit USER approval, so startup is still a release blocker and the final
disposition is `NOT RC READY`.
