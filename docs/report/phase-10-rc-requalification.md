# Phase 10 RC Requalification

## Executive Result

**NOT RC READY.** All ten USER-approved corrections are implemented, all checked-in automated UX,
resource, dependency and package gates pass except warm startup, and a new verified local candidate
exists. Warm startup p95 is 392.263 ms against the unchanged 180 ms gate. Real IME, visual,
taskbar/Alt+Tab, tray, physical DPI/multi-monitor and failure-site rows remain `NOT TESTED`.

## Preconditions and Identity

| Item | Value |
| --- | --- |
| Phase 9 disposition | NOT RC READY |
| USER approval | Phase 10 corrections approved; cold gate may relax to 400 ms if 300 ms remains a bottleneck |
| Starting commit | `3aedb24d` |
| Contract commit | `4a3ef1e` |
| Product implementation commit | `6c372a8` |
| Candidate source commit | `b9f83f13730b89bb59773e701eaa955d3c0acd0d` |
| Runtime dependencies added | None |

## Corrections

| Contract | Result |
| --- | --- |
| Ctrl+Insert / Shift+Delete / Shift+Insert aliases | IMPLEMENTED |
| global content zoom 50–300%, default 100% | IMPLEMENTED |
| keyboard ±10/reset and wheel ±5/notch | IMPLEMENTED |
| 220×120 DIP minimum | IMPLEMENTED |
| focusable Tool Window identity | IMPLEMENTED |
| 24 DIP dock capture | IMPLEMENTED |
| nearest eligible edge | IMPLEMENTED |
| one-DIP tie with Top > Left > Right | IMPLEMENTED |
| dock release remains expanded | IMPLEMENTED |
| opacity 40–100%, default 96% | IMPLEMENTED |

The detailed authority/algorithm review is in `phase-10-ux-corrections.md`.

## Automation

- Rust CLI owns task planning, de-duplication, gate evaluation, copied-Release process control,
  nearest-rank statistics and schema-v1 JSON evidence.
- JSON reports commit, `worktree_dirty`, verified artifact hash, per-task status/detail and structured
  measurements.
- PowerShell Phase wrappers only translate switches and propagate exit status.
- hosted CI calls one `all --ci --json` headless graph; unrequested runtime/manual rows do not make
  that graph fail or become PASS.
- local readiness/package/runtime retains the full automated-row readiness gate, so warm startup
  keeps returning nonzero.

## Startup

| Cohort | Samples | p50 | p95 | max | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| Cold | 30 | 293.867 ms | 321.540 ms | 396.086 ms | PASS <=400 ms; <=300 ms USER WAIVED |
| Warm | 30 | 378.596 ms | 392.263 ms | 394.937 ms | **FAIL** <=180 ms |

See `phase-10-startup-requalification.md` for method, optimization and rejected shortcuts.

## Performance

Unchanged Phase 1–8 Release baselines were re-run by the Rust task graph and passed. Representative
hard-path values remain:

| Path | Final p95 | Gate |
| --- | ---: | --- |
| 1 MiB source full-resync / slowest editor operation | 36.775 ms | PASS <=50 ms |
| 20 KiB Preview | 36.408 ms | PASS <=100 ms |
| 100 KiB Preview | 174.067 ms | PASS <=400 ms |
| 1 MiB Preview | 1.744 s | PASS <=2 s, background |
| 20 formula math document | 11.915 ms | PASS |
| 1 MiB persistence end-to-end | 8.091 ms | PASS |
| zoom relayout 50% | 38.700 ms | PASS <=50 ms |
| zoom relayout 100% | 38.275 ms | PASS <=50 ms |
| zoom relayout 300% | 34.511 ms | PASS <=50 ms |

## Memory and Idle CPU

All resource rows use five independent copied-Release processes. CPU-enabled rows use five
60-second intervals, each split into six diagnostic buckets. Values below are maximum PWS and
nearest-rank CPU p95 from the exact `b9f83f1` resource run.

| Scenario | PWS max | Idle CPU p95 | Gate |
| --- | ---: | ---: | --- |
| Source | 13.04 MiB | 0.003906% | PASS <=40 MiB / <=0.1% |
| Preview | 18.34 MiB | 0.001302% | PASS <=52 MiB / <=0.1% |
| Split | 24.62 MiB | 0.002604% | PASS <=64 MiB / <=0.1% |
| Preview, 200 unique formulae | 21.69 MiB | not separately sampled | PASS memory |
| Split, saturated image cache | 34.02 MiB | 0.002604% | PASS <=64 MiB / <=0.1% |
| Source after Preview cache release | 13.21 MiB | 0.002604% | PASS |
| Docked collapsed | 13.82 MiB | 0.005208% | PASS <=0.1% |
| Hidden to tray | 12.44 MiB | 0.001302% | PASS <=36 MiB / <=0.1% |
| Split zoom 50% | 24.22 MiB | covered by Split idle matrix | PASS <=64 MiB |
| Split zoom 100% | 24.52 MiB | covered by Split idle matrix | PASS <=64 MiB |
| Split zoom 300% | 26.68 MiB | covered by Split idle matrix | PASS <=64 MiB |

After 100 zoom-in/out cycles, private bytes changed by -245,760 bytes; no linear zoom growth was
observed. The highest idle-CPU p95 across the full matrix was 0.015623% in Source-with-image-links,
still below 0.1%.

The absolute Phase 9 and Phase 10 report rows were collected in separate long-running machine
cohorts and are not treated as a controlled product delta. A supplementary same-machine A/B
diagnostic launched the packaged Phase 9 and Phase 10 executables five times each with the same
20 KiB Source fixture and sampled each after 30 seconds. Median Working Set changed by -8,192 bytes
and median Private Bytes by +20,480 bytes; maxima changed by -61,440 and +1,015,808 bytes. This
hidden-launch diagnostic does not replace the formal copied-runtime gate, but it rejects the claim
that Phase 10 introduced a stable approximately-five-MiB product regression. The formal Phase 10
matrix above remains the current-candidate authority.

## Package

| Artifact | Value |
| --- | --- |
| source commit | `b9f83f13730b89bb59773e701eaa955d3c0acd0d` |
| EXE SHA-256 | `4c6b3470cc3e28a7c3fcdde4ee7c79c01a41a96c49756b445f4f33fac12faecf` |
| ZIP | `StickyMD-0.1.0-local-rc-b9f83f13730b-windows-x64-portable.zip` |
| ZIP bytes | 3,883,540 |
| ZIP SHA-256 | `277ab831171001453adb68925c42d1a34da74d6269fcfb73d3808d088571a053` |
| SBOM SHA-256 | `8bc55ec3a26ee384f704107d51883caefd69242371aa9fdedf990ea47ea9ab0a` |
| package verification | PASS, including copied runtime |

The prior Phase 9 ZIP is obsolete and was not reused. It was moved out of `dist/` to a recoverable
system-temporary backup before the Phase 10 package was generated.

## Safety and Architecture

- canonical text owner: `DocumentState`;
- durable note: external/durable fact only;
- Source/Preview: disposable generation-tagged projections;
- zoom/opacity authority: `ConfigCoordinator`;
- Tool Window identity: non-configurable Windows platform invariant;
- dock choice: pure O(1) comparison over Top/Left/Right;
- core unsafe blocks: 0;
- render unsafe blocks: 0;
- Windows-platform unsafe blocks: 67 total, all under the approved adapter boundary;
- forbidden WebView/Tauri/Tokio/async runtime/network/database imports: none;
- direct filesystem I/O in app/interaction/instruction shell modules: none.

## Acceptance

P10-A01..A30 and P10-A32..A36 are `AUTOMATED PASS`. P10-A31 is `BLOCKED` by warm startup. All
Phase 10 real-environment UX rows and inherited Phase 9 manual rows remain `NOT TESTED` unless a
current-candidate receipt is checked in.

## Recommendation

**STOP — NOT RC READY.** Do not tag, push or publish. Resolve or explicitly waive warm startup and
complete the current-candidate manual matrix first.
