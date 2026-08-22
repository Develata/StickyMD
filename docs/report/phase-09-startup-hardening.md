# Phase 9 Startup Hardening

## Status

Final copied-Release cohorts measured on 2026-08-21. Cold startup meets the original 300 ms hard
gate, so the USER-authorized 400 ms fallback is not used. Warm startup remains above its unchanged
180 ms hard gate and is a release blocker.

## Environment and Contract

- Source identity: the measured Phase 9 convergence tree; the exact convergence commit is recorded
  in the final package/readiness report after the tree is committed.
- Artifact: copied standalone Release executable; never `cargo run`.
- Fixture: source-only, 20,480-byte Markdown note generated from
  `tests/fixtures/performance/typical-note-seed.md`.
- Readiness: external process creation to the first successful presented frame after the canonical
  note is loaded, the native source projection is shaped, the window is visible, and IME input is
  enabled.
- Signal: private named event selected only by `STICKYMD_DIAGNOSTIC_READY_EVENT`.
- Exit: private diagnostic flag requests an ordinary event-loop exit after the ready frame, so the
  harness does not use forced termination as a warm-start side effect.
- Trace: fixed milestone names and monotonic microseconds selected only by
  `STICKYMD_DIAGNOSTIC_STARTUP_TRACE`; no note text or path is recorded. The trace uses a unique
  same-directory temporary, `FlushFileBuffers`, and create-new atomic publish, so an environment
  path can neither be truncated nor replaced.
- Cold cohort: 20 launches, no existing StickyMD process, 10 seconds idle before every launch.
- Warm cohort: 20 launches in the same copied directory, 250 ms between launches.
- Percentile: nearest-rank; no trimming or outlier deletion.
- Debugger: none. Defender real-time protection reported disabled on this host.

## Change Under Test

The source projection's initial full build now uses the same logical `BufferLine` model as
incremental edits. It restores scroll and shapes once rather than shaping before and after scroll
restoration. The private diagnostic shutdown also makes the process cohort reproducible without
changing normal startup. Neither change narrows the font database, bundles fonts, weakens fallback,
adds another text authority, or initializes Preview/RaTeX eagerly.

## Final External Results

| Cohort | Samples | p50 | p95 | max | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 20 | 258.771 ms | 277.205 ms | 404.996 ms | PASS: p95 <=300 ms |
| Warm | 20 | 325.975 ms | 342.891 ms | 356.433 ms | FAIL: p95 >180 ms |

The complete cold cohort retains its first-launch 404.996 ms observation. Nearest-rank p95 is the
nineteenth ordered sample, not the maximum; no observation was discarded. The cold result therefore
passes the frozen percentile contract without invoking the 400 ms relaxation.

## Internal Milestones

Milestones are cumulative from `main_enter`.

| Milestone | Cold p50 | Cold p95 | Warm p50 | Warm p95 |
| --- | ---: | ---: | ---: | ---: |
| FontSystem ready | 82.332 ms | 90.934 ms | 114.842 ms | 134.365 ms |
| source projection shaped | 180.880 ms | 191.725 ms | 244.519 ms | 252.025 ms |
| editor ready | 233.815 ms | 249.432 ms | 289.376 ms | 309.050 ms |

The externally observed ready signal trails the internal `editor_ready` milestone by event delivery
and process-observer overhead. The external value is authoritative for the release gate.

## Findings

1. `FontSystem::new()` remains material (cold p95 about 91 ms cumulative and warm p95 about 134 ms
   cumulative), but is not the sole bottleneck.
2. Source buffer construction/shaping plus the native-shell present path dominates the remaining
   warm time. The external warm p95 is 162.891 ms above the gate; the distribution does not support
   a 180 ms claim.
3. `cosmic-text` retains the complete system font fallback behavior; `syntect` and `vi` are not
   enabled.
4. A narrow font database, temporary second editor, false early-ready signal, or proprietary bundled
   fonts would trade correctness/architecture for a metric. None was introduced.
5. The retained optimization is simple and long-lived: one line model and one initial shaping pass,
   with no extra thread, cache, dependency, or lifetime protocol.

## Gate Result

| Gate | Result |
| --- | --- |
| Cold p95 <=300 ms | **PASS**, 277.205 ms |
| USER-authorized fallback cold p95 <=400 ms | Not needed |
| Warm p95 <=180 ms | **FAIL**, 342.891 ms; no waiver granted |
