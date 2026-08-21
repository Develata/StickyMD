# Phase 9 Startup Gate Review

## Decision State

`STOP FOR USER DISPOSITION` for the startup gate. Other Phase 9 convergence work may continue, but
an RC-ready recommendation cannot be issued while the relaxed 400 ms cold gate and the 180 ms warm
gate remain unresolved.

## Remaining Bottleneck

The cost is distributed rather than attributable to one accidental duplicate:

- system font discovery and font database construction: commonly 55–83 ms;
- source buffer construction and visible-line shaping: commonly about 90–130 ms combined;
- native monitor/tray/window visibility and first present: commonly about 40–70 ms;
- external process/security/cache overhead: commonly 22–58 ms, with first-sample spikes above
  1.4 seconds.

## Best Observed Values

- cold p50: 266.528 ms;
- cold p95 cohorts: 377.212 ms and 441.025 ms;
- warm p50 cohorts: 258.236 ms and 282.334 ms;
- warm p95 cohorts: 286.065 ms and 359.971 ms;
- internal editor-ready p50 after cleanup: 235.060 ms.

These results do not support a stable 400 ms cold claim and are far from the 180 ms warm hard gate.

## Further Optimization Cost and Risk

| Candidate | Likely benefit | Cost / risk | Disposition |
| --- | --- | --- | --- |
| Remove duplicate resync shaping and unify line construction | small; improves non-startup resync too | low | implemented and retained |
| Start font discovery on a one-shot startup thread | overlaps part of window/bootstrap work | adds another startup thread, cross-thread resource handoff, and more variable disk contention; still unlikely to reach 180 ms alone | do not add solely to chase the number |
| Narrow the system font database | potentially material | can break CJK/emoji/other-script fallback and user-installed fonts | forbidden without a separate risk decision and proof |
| Show a temporary GDI/DirectWrite/RichEdit editor | potentially material perceived speed | creates a second renderer/projection authority and visible handoff | forbidden |
| Signal before first usable source projection/present | makes the metric smaller only | violates the frozen `EDITOR_READY` contract | forbidden |
| Bundle proprietary Windows fonts | avoids discovery uncertainty | license and package-size violation | forbidden |

## Correctness, Memory, Dependency, and Maintenance Effects

- The retained cleanup preserves canonical text ownership and all font fallback behavior.
- It introduces no runtime dependency, proprietary font, additional cache, or persistent thread.
- It removes a duplicate full-resync shaping pass and uses one projection line model, reducing future
  maintenance surface.
- More aggressive candidates either add concurrency/lifetime complexity or change user-visible
  Unicode behavior. Their expected gain is insufficiently certain to justify those costs.

## Recommended USER Disposition

1. Keep 300 ms recorded as `USER WAIVED`, per the USER's explicit decision.
2. Do **not** yet declare the 400 ms relaxed cold gate passed; require a final current-artifact cohort
   after all Phase 9 release changes.
3. Decide whether the unchanged warm p95 <=180 ms gate may also be waived or relaxed. No such waiver
   has been granted yet.
4. Do not approve a narrow font database or temporary renderer merely to manufacture a passing
   startup number.

