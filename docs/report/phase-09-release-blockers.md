# Phase 9 Release Blockers

## Status

Automated inventory for the measured Phase 9 convergence tree. Exact clean-source package evidence
is recorded after the implementation tree is committed. Manual rows remain open unless a checked-in
current-artifact receipt exists.

| ID | Priority | Blocker | Evidence / next action | State |
| --- | --- | --- | --- | --- |
| RB-001 | P0 | Warm startup exceeds its hard gate | Final graceful-exit cohort: cold p95 268.595 ms PASS at the original 300 ms gate; warm p95 267.094 ms FAIL at 180 ms. No warm waiver exists. | OPEN |
| RB-002 | P0 | Real Microsoft Pinyin and WeChat IME acceptance absent | execute current RC matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-003 | P0 | Native Preview/math/image visual acceptance absent | execute Light/Dark/DPI matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-004 | P0 | Real tray/dock/theme/opacity/multi-monitor acceptance absent | execute physical Windows matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-005 | P0 | Clipboard/export/crash/reparse/ACL/Clean-VM evidence absent | execute environment matrix or retain NOT TESTED / obtain USER waiver | OPEN |
| RB-006 | P0 | Full user-asset and managed-looking-fake safety chains absent | execute the current-RC restart/edit/undo/redo/GC/export/quit receipts; low-level tests do not close this gate | OPEN |
| RB-007 | P0 | Phase 9 package/SBOM/license pipeline | runtime-graph notice generation and stricter verifier implemented; exact clean-source package must be regenerated | OPEN |
| RB-008 | P0 | Release workflow and supply-chain pins | cargo-deny action download replaced by Cargo checksum-verified installation; static and remote evidence must be rerun | OPEN |
| RB-009 | P1 | Final memory/CPU/latency/leak evidence | five 60-second samples per mode and the full 1000-window/100-persistence/100-conflict/100-image stress contract pass | CLOSED |
| RB-010 | P1 | `ttf-parser 0.25.1` unmaintained advisory is explicitly ignored | no vulnerability is reported and no compatible safe convergence exists; tracked in dedicated risk report | MONITORED |
| RB-011 | P1 | Release-facing documentation | README, Chinese README, security, contribution, changelog and release checklist completed without v1-ready claims | CLOSED |

## Ignored Tests Audit

Twelve ignored Rust tests were found. All are explicit Release-only performance baselines for Phases
3--8; no ignored correctness test exists. Every applicable ignored route was explicitly executed in
Release mode and recorded in `phase-09-performance-final.md`.

## Source Marker Audit

The initial code scan found no unresolved TODO/FIXME/HACK comment requiring implementation. Matches named `ATTEMPT_TIMEOUT` and `TEMP_SEQUENCE` are ordinary identifiers, not postponed work. Documentation status markers remain tracked above.

## Release Rule

Any open P0 blocker yields `NOT RC READY` unless USER explicitly waives that exact blocker. Environment absence remains `NOT TESTED`; it is not a PASS.
