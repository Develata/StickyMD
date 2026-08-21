# Phase 9 Inherited Conditions

## Status

Open inventory captured from current Phase 0–8 reports, risk reports and acceptance matrices on 2026-08-21.

## Release-Critical Manual Evidence

| ID | Inherited condition | Source | Current state |
| --- | --- | --- | --- |
| IC-01 | Microsoft Pinyin end-to-end IME matrix | Phase 1/3/5/6/7/8 matrices | NOT TESTED |
| IC-02 | WeChat Input Method end-to-end IME matrix | Phase 1/3/8 matrices | NOT TESTED |
| IC-03 | Native Preview selection/link/Split visual behavior | Phase 5/8 matrices | NOT TESTED |
| IC-04 | RaTeX visual fidelity, baseline, error UI and real-DPI rendering | Phase 6/8 matrices | NOT TESTED |
| IC-05 | Local-image visual quality and cache behavior | Phase 7/8 matrices | NOT TESTED |
| IC-06 | Real clipboard producers and paste/undo/restart behavior | Phase 7 matrix | NOT TESTED |
| IC-07 | Native Export dialog and Explorer-opened result | Phase 7/8 matrices | NOT TESTED |
| IC-08 | Real left/right/top dock, hover/no-focus, tray, opacity and theme | Phase 8 matrix | NOT TESTED |
| IC-09 | Physical multi-monitor, mixed DPI, disconnect, sleep/resume and RDP | Phase 8 matrix | NOT TESTED |
| IC-10 | Real Notepad reconciliation, ACL/read-only and long-path behavior | Phase 4 matrix | NOT TESTED |
| IC-11 | Forced-process recovery and live ReplaceFileW rare states | Phase 4/7 matrices | NOT TESTED |
| IC-12 | Real junction/symlink/reparse ownership boundary | Phase 1/7 matrices | NOT TESTED |
| IC-13 | Clean Windows 11 VM portable run | Phase 9 requirement | NOT TESTED |
| IC-14 | Governance/architecture semantic review receipts | Phase 0 matrix | NOT TESTED |

## Automated Conditions Requiring Fresh Phase 9 Evidence

- Cold and warm startup must be remeasured with an actual editor-ready signal and at least 20 samples each.
- Source/Preview/Split/Hidden memory, idle CPU, input/preview latency and leak stress must be rerun on the final Phase 9 commit.
- Persistence, OCC, recovery, managed-asset safety, export snapshot isolation, raw HTML and zero-network invariants must be rerun.
- Package allowlist, PE resources, licenses, checksums, SBOM and release workflow safety do not yet exist as Phase 9 evidence.

## User Decision

The original cold hard gate remains 300 ms during optimization. If correct, low-complexity optimization cannot meet it, USER has authorized a relaxed 400 ms hard gate. That disposition is conditional on fresh measurements and must never be reported as a 300 ms PASS.
