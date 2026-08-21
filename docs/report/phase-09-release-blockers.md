# Phase 9 Release Blockers

## Status

Initial inventory. Update every row from current-commit evidence; do not close a row from an older report.

| ID | Priority | Blocker | Evidence / next action | State |
| --- | --- | --- | --- | --- |
| RB-001 | P0 | Startup gates are not stable | Two 20+20 copied-Release cohorts exist. Cold p95 was 377.212 ms then 441.025 ms; warm p95 was 286.065 ms then 359.971 ms. The original 300 ms cold gate is USER-waived, but 400 ms cold and 180 ms warm remain open; see startup reports. | OPEN |
| RB-002 | P0 | Real Microsoft Pinyin and WeChat IME acceptance absent | execute current RC matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-003 | P0 | Native Preview/math/image visual acceptance absent | execute Light/Dark/DPI matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-004 | P0 | Real tray/dock/theme/opacity/multi-monitor acceptance absent | execute physical Windows matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-005 | P0 | Clipboard/export/crash/reparse/ACL/Clean-VM evidence absent | execute environment matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-006 | P0 | Phase 9 release package, checksum, SBOM and verifier not yet implemented | implement one allowlisted package path and verify copied artifact | OPEN |
| RB-007 | P0 | Release workflow and action/tool supply-chain pins not yet implemented | verify current official releases, pin full SHAs/checksums, static review | OPEN |
| RB-008 | P1 | Final current-commit memory/CPU/latency/leak evidence absent | run Phase 9 Release resource/performance suites | OPEN |
| RB-009 | P1 | `ttf-parser 0.25.1` unmaintained advisory is explicitly ignored | re-evaluate severity/upstream status; no blind dependency churn | OPEN |
| RB-010 | P1 | Release-facing README, security, contribution, changelog and checklist incomplete | finalize only implemented behavior; do not claim stable v1 | OPEN |

## Ignored Tests Audit

Twelve ignored Rust tests were found. All are explicit Release-only performance baselines for Phases 3–8; no ignored correctness test was found in the initial scan. Phase 9 must execute every applicable ignored performance route explicitly and record the final count.

## Source Marker Audit

The initial code scan found no unresolved TODO/FIXME/HACK comment requiring implementation. Matches named `ATTEMPT_TIMEOUT` and `TEMP_SEQUENCE` are ordinary identifiers, not postponed work. Documentation status markers remain tracked above.

## Release Rule

Any open P0 blocker yields `NOT RC READY` unless USER explicitly waives that exact blocker. Environment absence remains `NOT TESTED`; it is not a PASS.
