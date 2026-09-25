# Phase 02 Acceptance Matrix

> Verification projection for the canonical DocumentState, TextDelta, Generation, Undo/Redo and
> Snapshot slice. End-to-end Source/IME behavior is outside this Phase.

| ID | Plan / AC mapping | Mode | Checked-in evidence | Status |
| --- | --- | --- | --- | --- |
| P02-A01 | AC-009 core edit/undo/redo contract | Automated | core unit/property tests through [`phase-02.ps1`](../../tools/smoke/phase-02.ps1) | AUTOMATED PASS |
| P02-A02 | UTF-8 boundaries, stale edits and failure atomicity | Automated | deterministic core unit/property tests | AUTOMATED PASS |
| P02-A03 | monotonic generation, persistence acknowledgement and bounded history | Automated | core unit/property tests | AUTOMATED PASS |
| P02-A04 | 20 KiB/100 KiB/1 MiB String model baseline | Automated | [`phase-02.ps1 -Performance`](../../tools/smoke/phase-02.ps1) | AUTOMATED PASS |
| P02-A05 | core dependency/unsafe boundary | Automated | Rust governance validator + package tests | AUTOMATED PASS |

Phase 2 has no independent manual acceptance item. Real caret, IME, clipboard and rendering gates are
owned by Phase 3 and are not inferred from this matrix.

## 2026-09-07 maintenance regression coverage

This supplements P02-A01 under AC-009; it does not change a published artifact's qualification.

- Preconditions: adjacent same-kind edits without a selection; separately exercise backward and forward deletion around a newline.
- Action: compare edit gaps of 749/750/751 ms, and undo deletion of text/newline/text.
- Expected: only 749 ms merges; a deleted newline separates undo steps in both directions.
- Failure signals: 750 ms merges, or one undo restores text across the newline boundary.
- Entry: `cargo test -p stickymd-core --locked undo::tests`; the existing Phase 02 Rust task includes these tests. Current maintenance results are recorded in [the dated audit](../report/2026-09-07-runtime-audit.md); this does not refresh unrelated historical rows.
