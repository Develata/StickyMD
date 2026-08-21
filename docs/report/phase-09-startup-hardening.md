# Phase 9 Startup Hardening

## Status

Measured on 2026-08-21. The original cold and warm gates are not met. The USER-approved
400 ms cold ceiling was met by the baseline cohort but was not stable in the post-change cohort;
therefore startup remains a release blocker.

## Environment and Contract

- Artifact: copied standalone `target/release/stickymd-win.exe`, not `cargo run`.
- Fixture: source-only, 20,543-byte Markdown note.
- Readiness: external `CreateProcess` to the first successful presented frame after the canonical
  note is loaded, the native source projection is shaped, the window is visible, and IME input is
  enabled.
- Signal: private named event selected only by `STICKYMD_DIAGNOSTIC_READY_EVENT`.
- Trace: fixed milestone names and monotonic microseconds selected only by
  `STICKYMD_DIAGNOSTIC_STARTUP_TRACE`; no note text or path is recorded.
- Cold cohort: 20 launches, no existing StickyMD process, 10 seconds idle before every launch.
- Warm cohort: 20 launches in the same copied directory, 250 ms between launches.
- Percentile: nearest-rank; samples are not trimmed.
- Debugger: none.
- Defender: enabled; exact scan activity was not independently observable.

## Change Under Test

The source projection's initial full build was changed to use the same logical `BufferLine` model
as incremental edits. Full resync now restores scroll and shapes once instead of shaping before and
after scroll restoration. This removes one duplicate full-resync operation and one separate rich
text construction path. It does not narrow the font database, bundle fonts, weaken fallback, or
change the canonical `DocumentState` authority.

## External Results

| Cohort | Samples | Before p50 | Before p95 | Before max | After p50 | After p95 | After max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Cold | 20 | 270.077 ms | 377.212 ms | 1455.878 ms | 266.528 ms | 441.025 ms | 1522.921 ms |
| Warm | 20 | 258.236 ms | 286.065 ms | 292.004 ms | 282.334 ms | 359.971 ms | 396.725 ms |

The cold median improved by 3.549 ms, but the p95 became worse because the post-change cohort had
two slow launches. The warm cohort also became noisier. This change is retained for cohesion and
duplicate-work removal, not claimed as a startup-gate optimization.

## Internal Milestones

Milestones are cumulative from `main_enter`.

| Milestone | Before cold p50 | Before cold p95 | After cold p50 | After cold p95 |
| --- | ---: | ---: | ---: | ---: |
| program directory | 0.261 ms | 0.308 ms | 0.251 ms | 0.397 ms |
| single instance | 0.313 ms | 0.378 ms | 0.312 ms | 0.465 ms |
| persistence | 7.846 ms | 8.447 ms | 7.871 ms | 8.593 ms |
| document | 8.402 ms | 9.214 ms | 8.546 ms | 9.206 ms |
| event loop | 13.759 ms | 17.595 ms | 13.694 ms | 17.880 ms |
| window created | 28.809 ms | 33.685 ms | 29.080 ms | 38.921 ms |
| surface ready | 29.187 ms | 34.030 ms | 29.777 ms | 39.362 ms |
| FontSystem ready | 86.745 ms | 100.422 ms | 86.123 ms | 111.385 ms |
| source buffer ready | not instrumented | not instrumented | 109.781 ms | 145.570 ms |
| source projection shaped | 181.254 ms | 196.329 ms | 180.215 ms | 240.820 ms |
| monitor ready | 181.489 ms | 196.717 ms | 180.425 ms | 241.072 ms |
| tray ready | 193.303 ms | 211.749 ms | 192.481 ms | 257.648 ms |
| window visible | 231.883 ms | 246.004 ms | 209.540 ms | 306.617 ms |
| editor ready | 240.069 ms | 274.915 ms | 235.060 ms | 314.141 ms |

## Findings

1. `FontSystem::new()` is called once on source startup. Preview and RaTeX resources remain lazy.
2. Font database construction is material at roughly 55–83 ms per ordinary sample, but it is not
   the only bottleneck. Building and shaping the first source viewport is another roughly
   90–130 ms, followed by native shell presentation.
3. Process creation and Windows security/cache activity add approximately 22–58 ms on ordinary
   launches and produced first-sample spikes above 1.4 seconds in both cold experiments.
4. `cosmic-text` has only `std` and `swash` enabled directly; `syntect` and `vi` are not enabled.
5. A narrow font database or temporary renderer could reduce time, but would risk CJK, emoji, and
   other-script fallback. The Phase 9 contract explicitly forbids that unproven tradeoff.

## Gate Result

| Gate | Result |
| --- | --- |
| Cold p95 <= 300 ms | **USER WAIVED** for disposition purposes; measured FAIL in both cohorts |
| USER-approved cold p95 <= 400 ms | **FAIL (not stable)**; 377.212 ms before, 441.025 ms after |
| Warm p95 <= 180 ms | **FAIL**; 286.065 ms before, 359.971 ms after |

No sample was deleted as an outlier. A later final cohort may supersede these measurements only if
the artifact or implementation changes and the complete raw cohort is retained.

